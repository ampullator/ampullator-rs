use std::sync::{Arc, Mutex};

use ampullator::{
    ChannelRouting, GenGraph, graph_from_chain_expression, graph_from_json_definition,
};
use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat, StreamConfig, SupportedBufferSize};

const DEFAULT_BUFFER_SIZE: usize = 128;

// ─── CLI ─────────────────────────────────────────────────────────────────────

#[derive(Parser, Debug)]
#[command(
    name = "ampullator-play",
    about = "Play an Ampullator graph in real time via audio output"
)]
struct Cli {
    /// Chain DSL expression or path to a .txt/.chain/.json graph definition.
    /// Not required when --list-devices is given.
    #[arg(required_unless_present = "list_devices")]
    input: Option<String>,

    /// List available output devices and exit
    #[arg(long)]
    list_devices: bool,

    /// Output device name (substring match; defaults to system default)
    #[arg(long)]
    device: Option<String>,

    /// Output labels to play, e.g. "osc.wave" or "osc.wave,lfo.wave".
    /// Defaults to the last node's first two outputs (mapped to stereo L/R).
    #[arg(long, value_delimiter = ',')]
    outputs: Vec<String>,

    /// Sampling rate in Hz; defaults to the device's preferred rate
    #[arg(long)]
    sample_rate: Option<u32>,
}

// ─── audio state ─────────────────────────────────────────────────────────────

struct AudioState {
    graph: GenGraph,
    /// Resolved per-device-channel routing; immutable for the stream's life.
    routing: ChannelRouting,
    /// One block of pre-interleaved f32 samples. Length is
    /// `graph.buffer_size() * routing.channel_count()`. Drained from `head`
    /// outward; refilled when `head` reaches `block.len()`.
    block: Vec<f32>,
    /// Read position in `block`; `block.len()` means "drained, refill needed".
    head: usize,
}

impl AudioState {
    fn new(graph: GenGraph, routing: ChannelRouting) -> Self {
        let block_len = graph.buffer_size() * routing.channel_count();
        let block = vec![0.0; block_len];
        Self {
            graph,
            routing,
            block,
            // Start drained so the first callback triggers a refill.
            head: block_len,
        }
    }

    /// Render one block from the graph and write interleaved samples into
    /// `self.block`. Resets the read head.
    fn refill_block(&mut self) {
        self.graph.process();
        self.routing.interleave_into(&self.graph, &mut self.block);
        self.head = 0;
    }
}

// ─── helpers ─────────────────────────────────────────────────────────────────

fn build_graph(
    input: &str,
    sample_rate: f32,
    buffer_size: usize,
) -> Result<GenGraph, String> {
    if input.ends_with(".json") {
        let content = std::fs::read_to_string(input)
            .map_err(|e| format!("Failed to read '{input}': {e}"))?;
        graph_from_json_definition(&content, sample_rate, buffer_size)
    } else if input.ends_with(".txt") || input.ends_with(".chain") {
        let content = std::fs::read_to_string(input)
            .map_err(|e| format!("Failed to read '{input}': {e}"))?;
        graph_from_chain_expression(content.trim(), sample_rate, buffer_size)
    } else {
        graph_from_chain_expression(input, sample_rate, buffer_size)
    }
}

/// Pick a graph block size: prefer `preferred`, clamped into the device's
/// supported range when known, then rounded to a multiple of 8 (required by
/// the SIMD path in `GenGraph::process`).
fn pick_block_size(preferred: usize, supported: &SupportedBufferSize) -> usize {
    let raw = match supported {
        SupportedBufferSize::Range { min, max } => {
            preferred.clamp(*min as usize, *max as usize)
        }
        SupportedBufferSize::Unknown => preferred,
    };
    // Round down to a multiple of 8, with a floor of 8.
    (raw & !7).max(8)
}

/// Return the labels to feed to the device. If the user specified explicit
/// labels, those are used as-is. Otherwise, the last node's first two outputs
/// are chosen (one for mono chains, two for stereo).
fn resolve_labels(
    graph: &mut GenGraph,
    requested: &[String],
) -> Result<Vec<String>, String> {
    if !requested.is_empty() {
        return Ok(requested.to_vec());
    }
    let all = graph
        .get_last_node_output_names()
        .ok_or_else(|| "Graph has no nodes".to_string())?;
    Ok(all.into_iter().take(2).collect())
}

/// Build a typed output stream for any `SizedSample + FromSample<f32>` type.
fn build_typed_stream<T>(
    device: &cpal::Device,
    config: &StreamConfig,
    state: Arc<Mutex<AudioState>>,
) -> Result<cpal::Stream, String>
where
    T: cpal::SizedSample + cpal::FromSample<f32>,
{
    let err_fn = |e: cpal::StreamError| eprintln!("Audio stream error: {e}");

    device
        .build_output_stream(
            config,
            move |output: &mut [T], _: &cpal::OutputCallbackInfo| {
                let mut st = state.lock().unwrap();
                let mut written = 0;
                while written < output.len() {
                    if st.head == st.block.len() {
                        st.refill_block();
                    }
                    let take = (output.len() - written).min(st.block.len() - st.head);
                    let src = &st.block[st.head..st.head + take];
                    for (dst, &s) in output[written..written + take].iter_mut().zip(src) {
                        *dst = T::from_sample(s);
                    }
                    st.head += take;
                    written += take;
                }
            },
            err_fn,
            None,
        )
        .map_err(|e| format!("Failed to build output stream: {e}"))
}

// ─── main ────────────────────────────────────────────────────────────────────

fn run(cli: Cli) -> Result<(), String> {
    let host = cpal::default_host();

    if cli.list_devices {
        let default_name = host
            .default_output_device()
            .and_then(|d| d.description().ok())
            .map(|d| d.name().to_string())
            .unwrap_or_default();
        println!("Available output devices:");
        for device in host
            .output_devices()
            .map_err(|e| format!("Cannot enumerate devices: {e}"))?
        {
            let name = device
                .description()
                .map(|d| d.name().to_string())
                .unwrap_or_else(|_| "<unknown>".to_string());
            let marker = if name == default_name {
                " (default)"
            } else {
                ""
            };
            println!("  {name}{marker}");
        }
        return Ok(());
    }

    // `input` is guaranteed by clap when `list_devices` is false.
    let input = cli.input.as_deref().unwrap();

    // ── select device ──────────────────────────────────────────────────────
    let device = match &cli.device {
        Some(name) => host
            .output_devices()
            .map_err(|e| format!("Cannot enumerate devices: {e}"))?
            .find(|d| {
                d.description()
                    .map(|n| n.name().to_lowercase().contains(&name.to_lowercase()))
                    .unwrap_or(false)
            })
            .ok_or_else(|| format!("No output device matching '{name}'"))?,
        None => host
            .default_output_device()
            .ok_or_else(|| "No default output device found".to_string())?,
    };

    let device_name = device
        .description()
        .map(|d| d.name().to_string())
        .unwrap_or_else(|_| "<unknown>".to_string());
    println!("Output device: {device_name}");

    // ── device configuration ───────────────────────────────────────────────
    let supported = device
        .default_output_config()
        .map_err(|e| format!("Cannot get default output config: {e}"))?;

    let sample_rate = cli.sample_rate.unwrap_or_else(|| supported.sample_rate());

    let out_channels = supported.channels() as usize;
    let sample_format = supported.sample_format();

    // Pick a graph block size that lands inside the device's supported range
    // (and stays a multiple of 8 for the SIMD path in graph.process).
    let block_size = pick_block_size(DEFAULT_BUFFER_SIZE, supported.buffer_size());

    let config = StreamConfig {
        channels: supported.channels(),
        sample_rate,
        buffer_size: BufferSize::Fixed(block_size as u32),
    };

    // ── build graph ────────────────────────────────────────────────────────
    let mut graph = build_graph(input, sample_rate as f32, block_size)?;
    let labels = resolve_labels(&mut graph, &cli.outputs)?;

    // Resolve per-device-channel routing once. For a single-source chain,
    // broadcast the mono signal to every device channel. Otherwise map
    // 1:1 by index and pad with silence.
    let channel_refs: Vec<Option<&str>> = (0..out_channels)
        .map(|ch| {
            if labels.len() == 1 {
                Some(labels[0].as_str())
            } else if ch < labels.len() {
                Some(labels[ch].as_str())
            } else {
                None
            }
        })
        .collect();
    let routing = ChannelRouting::new(&graph, &channel_refs);

    println!("Playing outputs: {}", labels.join(", "));
    println!(
        "Sample rate: {} Hz  |  Channels: {}  |  Format: {:?}  |  Block: {}",
        sample_rate, out_channels, sample_format, block_size
    );
    println!("Press Enter to stop.");

    let state = Arc::new(Mutex::new(AudioState::new(graph, routing)));

    // ── build typed stream ─────────────────────────────────────────────────
    let stream = match sample_format {
        SampleFormat::F32 => {
            build_typed_stream::<f32>(&device, &config, Arc::clone(&state))
        }
        SampleFormat::I16 => {
            build_typed_stream::<i16>(&device, &config, Arc::clone(&state))
        }
        SampleFormat::U16 => {
            build_typed_stream::<u16>(&device, &config, Arc::clone(&state))
        }
        fmt => return Err(format!("Unsupported sample format: {fmt:?}")),
    }?;

    stream
        .play()
        .map_err(|e| format!("Failed to start stream: {e}"))?;

    // ── wait for Enter ─────────────────────────────────────────────────────
    let mut line = String::new();
    std::io::stdin()
        .read_line(&mut line)
        .map_err(|e| format!("stdin error: {e}"))?;

    drop(stream);
    Ok(())
}

fn main() {
    if let Err(e) = run(Cli::parse()) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
