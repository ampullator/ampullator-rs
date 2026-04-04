use std::path::PathBuf;

use clap::Parser;

#[derive(Parser)]
#[command(
    name = "ampullator-doc",
    about = "Build Ampullator documentation and examples"
)]
struct Cli {
    /// Input directory containing example files
    #[arg(short, long, default_value = "doc/example")]
    input: PathBuf,

    /// Output directory for generated documentation
    #[arg(short, long, default_value = "doc/out")]
    output: PathBuf,

    /// Sample rate in Hz
    #[arg(long, default_value_t = 100.0)]
    sample_rate: f32,

    /// Buffer size in samples
    #[arg(long, default_value_t = 8)]
    buffer_size: usize,

    /// Total samples to render
    #[arg(long, default_value_t = 100)]
    total_samples: usize,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = ampullator::build_markdown_index(
        &cli.input,
        &cli.output,
        cli.sample_rate,
        cli.buffer_size,
        cli.total_samples,
    ) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
