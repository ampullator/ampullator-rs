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

    /// Output directory for generated figures
    #[arg(short, long, default_value = "doc/out")]
    output: PathBuf,

    /// Path to usage.md source file
    #[arg(short, long, default_value = "doc/usage.md")]
    usage: PathBuf,

    /// Path to clis.md source file
    #[arg(short, long, default_value = "doc/clis.md")]
    clis: PathBuf,

    /// Path to write the combined README.md
    #[arg(short, long, default_value = "README.md")]
    readme: PathBuf,

    /// Prefix image paths with the GitHub repository base URL
    #[arg(long, default_value_t = true)]
    abs_paths: bool,
}

fn main() {
    let cli = Cli::parse();
    if let Err(e) = ampullator::build_markdown_index(
        &cli.input,
        &cli.output,
        &cli.usage,
        &cli.clis,
        &cli.readme,
        cli.abs_paths,
    ) {
        eprintln!("Error: {e}");
        std::process::exit(1);
    }
}
