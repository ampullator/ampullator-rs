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
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = ampullator::build_markdown_index(&cli.input, &cli.output) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
