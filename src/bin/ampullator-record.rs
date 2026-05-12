use std::collections::HashSet;
use std::io::BufWriter;
use std::path::PathBuf;

use ampullator::{
    GenGraph, Recorder, WavFormat, graph_from_chain_expression,
    graph_from_json_definition,
};
use clap::Parser;

const DEFAULT_BUFFER_SIZE: usize = 128;

#[derive(Parser, Debug)]
#[command(
    name = "ampullator-record",
    about = "Record outputs from an Ampullator graph to a multichannel WAV file"
)]
struct Cli {
    /// Chain DSL expression or path to a text/json graph definition file
    input: String,

    /// Node name to record from (defaults to final node in graph execution order)
    #[arg(long)]
    node: Option<String>,

    /// Output names to record (comma-separated); defaults to all outputs on selected node
    #[arg(long, value_delimiter = ',')]
    outputs: Vec<String>,

    /// WAV bit depth (16, 24, or 32)
    #[arg(long, default_value_t = 16)]
    bit_depth: u16,

    /// Sampling rate in Hz
    #[arg(long, default_value_t = 44_100.0)]
    sample_rate: f32,

    /// Duration to record, in seconds
    #[arg(long)]
    duration: f32,

    /// Output WAV file path; omit to stream WAV to stdout
    #[arg(short = 'o', long)]
    output: Option<PathBuf>,
}

fn build_graph_from_input(
    input: &str,
    sample_rate: f32,
    buffer_size: usize,
) -> Result<GenGraph, String> {
    if input.ends_with(".json") {
        let content = std::fs::read_to_string(input)
            .map_err(|e| format!("Failed to read input file '{input}': {e}"))?;
        graph_from_json_definition(&content, sample_rate, buffer_size)
    } else if input.ends_with(".txt") || input.ends_with(".chain") {
        let content = std::fs::read_to_string(input)
            .map_err(|e| format!("Failed to read input file '{input}': {e}"))?;
        graph_from_chain_expression(content.trim(), sample_rate, buffer_size)
    } else {
        graph_from_chain_expression(input, sample_rate, buffer_size)
    }
}

fn resolve_output_labels(
    graph: &mut GenGraph,
    node: Option<&str>,
    outputs: &[String],
) -> Result<Vec<String>, String> {
    // Fast path: no node or outputs specified — use last node's outputs.
    if node.is_none() && outputs.is_empty() {
        return graph
            .get_last_node_output_names()
            .ok_or_else(|| "Graph has no nodes to record".to_string());
    }

    // Determine the node to record, either provided or the last
    let node_name = match node {
        Some(name) => name.to_string(),
        None => {
            let names = graph.get_execution_names();
            names
                .last()
                .cloned()
                .ok_or_else(|| "Graph has no nodes to record".to_string())?
        }
    };

    let node_labels = graph
        .get_output_names_for(&node_name)
        .ok_or_else(|| format!("Unknown node '{node_name}'"))?;
    if node_labels.is_empty() {
        return Err(format!("Node '{node_name}' has no outputs"));
    }

    // if none provided, record all
    if outputs.is_empty() {
        return Ok(node_labels);
    }

    let label_set: HashSet<&str> = node_labels.iter().map(|s| s.as_str()).collect();
    let mut selected = Vec::with_capacity(outputs.len());
    for output in outputs {
        let label = if output.contains('.') {
            output.clone()
        } else {
            format!("{node_name}.{output}")
        };
        if !label_set.contains(label.as_str()) {
            return Err(format!(
                "Output '{output}' is not available on node '{node_name}'"
            ));
        }
        if !selected.contains(&label) {
            selected.push(label);
        }
    }
    Ok(selected)
}

fn run(cli: Cli) -> Result<(), String> {
    if cli.duration <= 0.0 {
        return Err("duration must be > 0".to_string());
    }
    if cli.sample_rate <= 0.0 {
        return Err("sample-rate must be > 0".to_string());
    }
    let mut graph =
        build_graph_from_input(&cli.input, cli.sample_rate, DEFAULT_BUFFER_SIZE)?;
    let labels = resolve_output_labels(&mut graph, cli.node.as_deref(), &cli.outputs)?;

    let recorder = Recorder::from_duration(graph, Some(labels), cli.duration);
    let format = WavFormat::try_from(cli.bit_depth).expect("invalid bit depth");
    match cli.output {
        None => {
            let stdout = std::io::stdout();
            recorder
                .to_wav_write(BufWriter::new(stdout.lock()), format)
                .map_err(|e| format!("Failed to write WAV to stdout: {e}"))?;
        }
        Some(ref path) => {
            recorder
                .to_wav(path, format)
                .map_err(|e| format!("Failed to write WAV '{}': {e}", path.display()))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::Builder;

    #[test]
    fn test_build_graph_from_json_file() {
        let tmp = Builder::new().suffix(".json").tempfile().unwrap();
        std::fs::write(
            tmp.path(),
            r#"{"chain":"Clock(rate=1, mode=Samples) + Clock(rate=2, mode=Samples)"}"#,
        )
        .unwrap();

        let g =
            build_graph_from_input(tmp.path().to_str().unwrap(), 44_100.0, 128).unwrap();
        assert!(!g.is_empty());
    }

    #[test]
    fn test_resolve_output_labels_defaults_to_last_node_outputs() {
        let mut g = build_graph_from_input(
            "Clock(rate=1, mode=Samples) + Clock(rate=2, mode=Samples)",
            44_100.0,
            128,
        )
        .unwrap();
        let labels = resolve_output_labels(&mut g, None, &[]).unwrap();
        assert_eq!(labels.len(), 1);
        assert!(labels[0].ends_with(".out"));
    }

    #[test]
    fn test_resolve_output_labels_for_specific_node_and_output() {
        let mut g = build_graph_from_input(
            "Clock(rate=1, mode=Samples) => c | c -> Round() => r",
            44_100.0,
            128,
        )
        .unwrap();
        let labels =
            resolve_output_labels(&mut g, Some("c"), &["out".to_string()]).unwrap();
        assert_eq!(labels, vec!["c.out".to_string()]);
    }
}
