use std::time::Instant;

use ampullator::{GenGraph, UGLowPass, UGSine, UGWhite};
use clap::{Parser, ValueEnum};

const DEFAULT_BUFFER_SIZE: usize = 128;
const DEFAULT_SAMPLE_RATE: f32 = 44_800.0;
const DEFAULT_DURATION: f64 = 4.0;
/// Stop doubling when node count would exceed this
const MAX_NODES: usize = 4096;

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

#[derive(ValueEnum, Clone, Debug, PartialEq, Copy)]
enum GraphType {
    /// Chain of sine oscillators where each modulates the next
    SineChain,
    /// White-noise source passed through a series of low-pass filters
    FilteredNoise,
    /// Run all three graph types in sequence
    All,
}

#[derive(Parser, Debug)]
#[command(
    name = "ampullator-profile",
    about = "Estimate real-time performance constraints by benchmarking signal \
             graphs of increasing node-count complexity"
)]
struct Cli {
    /// Sampling rate in Hz
    #[arg(long, default_value_t = DEFAULT_SAMPLE_RATE)]
    sample_rate: f32,

    /// Buffer size in samples (must be a non-zero multiple of 8)
    #[arg(long, default_value_t = DEFAULT_BUFFER_SIZE)]
    buffer_size: usize,

    /// Target duration to simulate in seconds
    #[arg(long, default_value_t = DEFAULT_DURATION)]
    duration: f64,

    /// Graph topology to benchmark; defaults to running all topologies
    #[arg(long, value_enum, default_value_t = GraphType::All)]
    graph_type: GraphType,

    /// Disable parallel execution (run graph types sequentially)
    #[arg(long, default_value_t = false)]
    no_parallel: bool,
}

// ---------------------------------------------------------------------------
// Graph builders
// ---------------------------------------------------------------------------

/// Build a chain of `n` sine oscillators where each feeds the frequency
/// input of the next one.  Minimum `n` is 1 (a single free-running sine).
fn build_sine_chain(n: usize, sample_rate: f32, buffer_size: usize) -> GenGraph {
    assert!(n >= 1);
    let mut graph = GenGraph::new(sample_rate, buffer_size);
    for i in 0..n {
        graph.add_node(format!("osc{i}"), Box::new(UGSine::new()));
        if i > 0 {
            graph.connect(&format!("osc{}.wave", i - 1), &format!("osc{i}.freq"));
        }
    }
    graph
}

/// Build a graph with one white-noise source followed by `n` low-pass
/// filters in series.  Minimum `n` is 1 (noise + one LPF).
/// Total node count = n + 1.
fn build_filtered_noise(n: usize, sample_rate: f32, buffer_size: usize) -> GenGraph {
    assert!(n >= 1);
    let mut graph = GenGraph::new(sample_rate, buffer_size);
    graph.add_node("noise", Box::new(UGWhite::new(None)));
    for i in 0..n {
        graph.add_node(format!("lpf{i}"), Box::new(UGLowPass::new(12.0)));
        let src = if i == 0 {
            "noise.out".to_string()
        } else {
            format!("lpf{}.out", i - 1)
        };
        graph.connect(&src, &format!("lpf{i}.in"));
    }
    graph
}

// ---------------------------------------------------------------------------
// Benchmarking
// ---------------------------------------------------------------------------

struct BenchRow {
    graph_type: &'static str,
    node_count: usize,
    sample_rate: f32,
    buffer_size: usize,
    target_duration: f64,
    performed_duration: f64,
}

const BENCH_RUNS: usize = 2;

/// Process enough buffers to cover `target_secs` worth of audio, repeated
/// `BENCH_RUNS` times, and return the average wall-clock time.
fn bench_graph(
    mut graph: GenGraph,
    sample_rate: f32,
    buffer_size: usize,
    target_secs: f64,
) -> f64 {
    let total_samples = (sample_rate as f64 * target_secs).ceil() as usize;
    let num_buffers = total_samples.div_ceil(buffer_size);

    let mut total = 0.0;
    for _ in 0..BENCH_RUNS {
        let start = Instant::now();
        for _ in 0..num_buffers {
            graph.process();
        }
        total += start.elapsed().as_secs_f64();
    }
    total / BENCH_RUNS as f64
}

const LEVEL_START: usize = 128;
const LEVEL_INCREMENT: usize = 128;

/// Levels to benchmark: 128, 256, 384, … incrementing until exceeding MAX_NODES.
fn level_sequence() -> impl Iterator<Item = usize> {
    (0..)
        .map(|k| LEVEL_START + k * LEVEL_INCREMENT)
        .take_while(|&n| n <= MAX_NODES)
}

/// Bench a single level, returning a `BenchRow`.
fn bench_level(
    label: &'static str,
    builder: fn(usize, f32, usize) -> GenGraph,
    level: usize,
    sample_rate: f32,
    buffer_size: usize,
    duration: f64,
) -> BenchRow {
    let graph = builder(level, sample_rate, buffer_size);
    let node_count = graph.len();
    let performed = bench_graph(graph, sample_rate, buffer_size, duration);
    BenchRow {
        graph_type: label,
        node_count,
        sample_rate,
        buffer_size,
        target_duration: duration,
        performed_duration: performed,
    }
}

/// Run the benchmark for a graph type sequentially.
fn run_benchmark_sequential(
    label: &'static str,
    builder: fn(usize, f32, usize) -> GenGraph,
    sample_rate: f32,
    buffer_size: usize,
    duration: f64,
) -> Vec<BenchRow> {
    let mut rows = Vec::new();
    for level in level_sequence() {
        let row = bench_level(label, builder, level, sample_rate, buffer_size, duration);
        let exceeded = row.performed_duration > duration;
        rows.push(row);
        if exceeded {
            break;
        }
    }
    rows
}

/// Run the benchmark for a graph type with levels processed in parallel
/// chunks of `max_threads` size.
fn run_benchmark_parallel(
    label: &'static str,
    builder: fn(usize, f32, usize) -> GenGraph,
    sample_rate: f32,
    buffer_size: usize,
    duration: f64,
    max_threads: usize,
) -> Vec<BenchRow> {
    let levels: Vec<usize> = level_sequence().collect();
    let mut rows = Vec::new();
    for chunk in levels.chunks(max_threads) {
        let chunk_rows: Vec<BenchRow> = std::thread::scope(|s| {
            let handles: Vec<_> = chunk
                .iter()
                .map(|&level| {
                    s.spawn(move || {
                        bench_level(
                            label,
                            builder,
                            level,
                            sample_rate,
                            buffer_size,
                            duration,
                        )
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let exceeded = chunk_rows.iter().any(|r| r.performed_duration > duration);
        rows.extend(chunk_rows);
        if exceeded {
            break;
        }
    }
    rows
}

// ---------------------------------------------------------------------------
// Output formatting
// ---------------------------------------------------------------------------

fn print_header() {
    println!(
        "   {:<18} {:<6} {:<11} {:<8} {:<12} {:<14} {:<7}",
        "Graph", "Nodes", "SampleRate", "Buffer", "Target", "Performed", "Ratio"
    );
}

fn print_row(row: &BenchRow) {
    let ratio = row.performed_duration / row.target_duration;
    let status = if ratio >= 1.0 { "⚠️  " } else { "✅ " };
    println!(
        "{status}{:<18} {:<6} {:<11} {:<7} {:<7.1} {:<14.4} {:<7.2}",
        row.graph_type,
        row.node_count,
        row.sample_rate,
        row.buffer_size,
        row.target_duration,
        row.performed_duration,
        ratio,
    );
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

fn run(cli: Cli) -> Result<(), String> {
    if cli.duration <= 0.0 {
        return Err("--duration must be > 0".to_string());
    }
    if cli.sample_rate <= 0.0 {
        return Err("--sample-rate must be > 0".to_string());
    }
    if !cli.buffer_size.is_multiple_of(8) {
        return Err("--buffer-size must be a non-zero multiple of 8".to_string());
    }

    let sr = cli.sample_rate;
    let buf = cli.buffer_size;
    let dur = cli.duration;

    type BuildFn = fn(usize, f32, usize) -> GenGraph;
    let mut types_to_run: Vec<(&'static str, BuildFn)> = Vec::new();
    if matches!(cli.graph_type, GraphType::SineChain | GraphType::All) {
        types_to_run.push(("sine-chain", build_sine_chain));
    }
    if matches!(cli.graph_type, GraphType::FilteredNoise | GraphType::All) {
        types_to_run.push(("filtered-noise", build_filtered_noise));
    }
    let max_threads = std::thread::available_parallelism()
        .map(|n| n.get() / 2)
        .unwrap_or(1)
        .max(1);

    print_header();
    for (label, builder) in &types_to_run {
        let rows = if cli.no_parallel {
            run_benchmark_sequential(label, *builder, sr, buf, dur)
        } else {
            run_benchmark_parallel(label, *builder, sr, buf, dur, max_threads)
        };
        for row in &rows {
            print_row(row);
        }
    }

    Ok(())
}

fn main() {
    if let Err(e) = run(Cli::parse()) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sine_chain_node_count() {
        for n in [1, 2, 4, 8] {
            let g = build_sine_chain(n, 44_100.0, 128);
            assert_eq!(g.len(), n, "sine-chain level {n}");
        }
    }

    #[test]
    fn test_filtered_noise_node_count() {
        for n in [1, 2, 4, 8] {
            let g = build_filtered_noise(n, 44_100.0, 128);
            // 1 noise + n LPFs
            assert_eq!(g.len(), n + 1, "filtered-noise level {n}");
        }
    }

    #[test]
    fn test_bench_graph_produces_output() {
        // Just ensure bench_graph runs without panicking and returns a non-negative time.
        let g = build_sine_chain(2, 44_100.0, 128);
        let t = bench_graph(g, 44_100.0, 128, 0.01);
        assert!(t >= 0.0);
    }

    #[test]
    fn test_level_sequence() {
        let levels: Vec<usize> = level_sequence().take(5).collect();
        assert_eq!(levels, vec![128, 256, 384, 512, 640]);
    }
}
