use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};

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

    // Normalize `-` to None -- it means stdio, not a file path.
    let is_stdio = |p: &PathBuf| p.as_os_str() == "-";
    let input_path = args.input.filter(|p| !is_stdio(p));
    let output_path = args.output.filter(|p| !is_stdio(p));

    let mut input = get_input_reader(&input_path)?;
    let mut input_text = String::new();
    input.read_to_string(&mut input_text)?;

    let from = resolve_input_format(args.from, input_path.as_deref(), &input_text)?;

    if args.detect {
        println!("{from}");
        return Ok(());
    }

    let to = resolve_output_format(args.to, output_path.as_deref())?;

    let graph = csvizmo_depgraph::parse::parse(from, &input_text)?;
    tracing::info!(
        "Parsed graph with {} nodes, {} edges, and {} subgraphs",
        graph.all_nodes().len(),
        graph.all_edges().len(),
        graph.subgraphs.len()
    );

    let mut output = get_output_writer(&output_path)?;
    csvizmo_depgraph::emit::emit(to, &graph, &mut output)?;

    Ok(())
}

/// Resolve input format: explicit flag > file extension > content detection.
fn resolve_input_format(
    flag: Option<InputFormat>,
    path: Option<&Path>,
    input: &str,
) -> eyre::Result<InputFormat> {
    if let Some(f) = flag {
        return Ok(f);
    }
    let ext_err = match path.map(InputFormat::try_from) {
        Some(Ok(f)) => {
            tracing::info!("Detected input format: {f:?} from file extension");
            return Ok(f);
        }
        // There was an error with the extension detection, but we'll try content detection before bailing
        Some(Err(e)) => Some(e),
        // There was no path
        None => None,
    };
    // Try to detect type from content before bailing
    if let Some(f) = csvizmo_depgraph::detect::detect(input) {
        tracing::info!("Detected input format: {f:?} from content");
        return Ok(f);
    }
    // Bail, but try to give a better error based on whether the extension detection failed
    match ext_err {
        Some(e) => Err(e.wrap_err("cannot detect input format; use --from")),
        None => eyre::bail!("cannot detect input format; use --from"),
    }
}

/// Resolve output format: explicit flag > file extension > default to DOT.
fn resolve_output_format(
    flag: Option<OutputFormat>,
    path: Option<&Path>,
) -> eyre::Result<OutputFormat> {
    if let Some(f) = flag {
        return Ok(f);
    }
    match path.map(OutputFormat::try_from) {
        Some(Ok(f)) => {
            tracing::info!("Detected output format: {f:?} from file extension");
            Ok(f)
        }
        Some(Err(e)) => {
            // TODO: I can't decide if this should error or default to DOT
            Err(e.wrap_err("Failed to detect output format from file extension; use --to"))
        }
        None => Ok(OutputFormat::Dot),
    }
}
