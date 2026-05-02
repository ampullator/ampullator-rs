use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use ampullator::{GenGraph, graph_from_chain_expression, graph_from_json_definition};
use clap::Parser;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{BufferSize, SampleFormat, SampleRate, StreamConfig};

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
    /// Output labels to feed into the device (1 or 2 entries)
    labels: Vec<String>,
    /// Ring buffer of pre-interleaved f32 samples ready for the cpal callback
    queue: VecDeque<f32>,
    /// Number of output channels reported by the device
    out_channels: usize,
}

impl AudioState {
    /// Process one block from the graph and push interleaved samples onto the queue.
    fn fill_queue(&mut self) {
        self.graph.process();

        // Collect a snapshot of each source channel's buffer to avoid borrow issues.
        let buffers: Vec<Vec<f32>> = self
            .labels
            .iter()
            .map(|l| self.graph.get_output_by_label(l).to_vec())
            .collect();

        let n_frames = buffers[0].len();
        let n_src = buffers.len(); // 1 or 2

        for i in 0..n_frames {
            for ch in 0..self.out_channels {
                let s = if n_src == 1 {
                    // Mono source → broadcast to all output channels
                    buffers[0][i]
                } else if ch < n_src {
                    buffers[ch][i]
                } else {
                    0.0
                };
                self.queue.push_back(s);
            }
        }
    }
}

// ─── helpers ─────────────────────────────────────────────────────────────────

fn build_graph(input: &str, sample_rate: f32) -> Result<GenGraph, String> {
    if input.ends_with(".json") {
        let content = std::fs::read_to_string(input)
            .map_err(|e| format!("Failed to read '{input}': {e}"))?;
        graph_from_json_definition(&content, sample_rate, DEFAULT_BUFFER_SIZE)
    } else if input.ends_with(".txt") || input.ends_with(".chain") {
        let content = std::fs::read_to_string(input)
            .map_err(|e| format!("Failed to read '{input}': {e}"))?;
        graph_from_chain_expression(content.trim(), sample_rate, DEFAULT_BUFFER_SIZE)
    } else {
        graph_from_chain_expression(input, sample_rate, DEFAULT_BUFFER_SIZE)
    }
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
                for sample in output.iter_mut() {
                    if st.queue.is_empty() {
                        st.fill_queue();
                    }
                    *sample = T::from_sample(st.queue.pop_front().unwrap_or(0.0));
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
            .and_then(|d| d.name().ok())
            .unwrap_or_default();
        println!("Available output devices:");
        for device in host
            .output_devices()
            .map_err(|e| format!("Cannot enumerate devices: {e}"))?
        {
            let name = device.name().unwrap_or_else(|_| "<unknown>".to_string());
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
                d.name()
                    .map(|n| n.to_lowercase().contains(&name.to_lowercase()))
                    .unwrap_or(false)
            })
            .ok_or_else(|| format!("No output device matching '{name}'"))?,
        None => host
            .default_output_device()
            .ok_or_else(|| "No default output device found".to_string())?,
    };

    let device_name = device.name().unwrap_or_else(|_| "<unknown>".to_string());
    println!("Output device: {device_name}");

    // ── device configuration ───────────────────────────────────────────────
    let supported = device
        .default_output_config()
        .map_err(|e| format!("Cannot get default output config: {e}"))?;

    let sample_rate = cli.sample_rate.unwrap_or_else(|| supported.sample_rate().0);

    let out_channels = supported.channels() as usize;
    let sample_format = supported.sample_format();

    let config = StreamConfig {
        channels: supported.channels(),
        sample_rate: SampleRate(sample_rate),
        buffer_size: BufferSize::Default,
    };

    // ── build graph ────────────────────────────────────────────────────────
    let mut graph = build_graph(input, sample_rate as f32)?;
    let labels = resolve_labels(&mut graph, &cli.outputs)?;

    println!("Playing outputs: {}", labels.join(", "));
    println!(
        "Sample rate: {} Hz  |  Channels: {}  |  Format: {:?}",
        sample_rate, out_channels, sample_format
    );
    println!("Press Enter to stop.");

    let state = Arc::new(Mutex::new(AudioState {
        graph,
        labels,
        queue: VecDeque::new(),
        out_channels,
    }));

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
