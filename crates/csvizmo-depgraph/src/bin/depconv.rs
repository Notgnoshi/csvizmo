use std::io::{IsTerminal, Read};
use std::path::PathBuf;

use clap::Parser;
use csvizmo_depgraph::{InputFormat, OutputFormat};
use csvizmo_utils::stdio::{get_input_reader, get_output_writer};

/// Dependency graph format converter.
///
/// Formats are auto-detected from file extensions or content when --from/--to are not specified.
#[derive(Debug, Parser)]
#[clap(version, verbatim_doc_comment)]
struct Args {
    #[clap(short, long, default_value_t = tracing::Level::INFO)]
    log_level: tracing::Level,

    /// Input format (auto-detected from extension or content if omitted)
    #[clap(short, long)]
    from: Option<InputFormat>,

    /// Output format (auto-detected from output extension if omitted, defaults to DOT)
    #[clap(short, long)]
    to: Option<OutputFormat>,

    /// Print the detected input format and exit
    #[clap(long)]
    detect: bool,

    /// Path to the input. stdin if '-' or omitted
    #[clap(short, long)]
    input: Option<PathBuf>,

    /// Path to the output. stdout if '-' or omitted
    #[clap(short, long)]
    output: Option<PathBuf>,
}

fn main() -> eyre::Result<()> {
    let use_color = std::io::stderr().is_terminal();
    if use_color {
        color_eyre::install()?;
    }

    let args = Args::parse();

    let filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(args.log_level.into())
        .with_env_var("CSV_LOG")
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(use_color)
        .with_writer(std::io::stderr)
        .init();

    let mut input = get_input_reader(&args.input)?;
    let mut input_text = String::new();
    input.read_to_string(&mut input_text)?;

    if args.detect {
        // TODO: implement format auto-detection
        eyre::bail!("--detect not yet implemented");
    }

    let from = args
        .from
        .ok_or_else(|| eyre::eyre!("--from is required (auto-detection not yet implemented)"))?;
    let to = args.to.unwrap_or(OutputFormat::Dot);

    let graph = csvizmo_depgraph::parse::parse(from, &input_text)?;

    let mut output = get_output_writer(&args.output)?;
    csvizmo_depgraph::emit::emit(to, &graph, &mut output)?;

    Ok(())
}
