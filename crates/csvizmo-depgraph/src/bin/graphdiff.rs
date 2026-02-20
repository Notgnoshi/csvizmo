use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};

use clap::{Parser, Subcommand};
use csvizmo_depgraph::DepGraph;
use csvizmo_depgraph::algorithm::diff;
use csvizmo_depgraph::emit::OutputFormat;
use csvizmo_depgraph::parse::InputFormat;
use csvizmo_utils::stdio::{get_input_reader, get_output_writer};

/// Compute differences between two dependency graphs.
///
/// Takes two graph files (before and after) and produces various
/// diff representations via subcommands.
#[derive(Debug, Parser)]
#[clap(version, verbatim_doc_comment)]
struct Args {
    /// Logging level
    #[clap(long, default_value_t = tracing::Level::INFO)]
    log_level: tracing::Level,

    /// Input format (auto-detected from extension/content if omitted)
    #[clap(short = 'I', long, global = true)]
    input_format: Option<InputFormat>,

    /// Output file (stdout if '-' or omitted)
    #[clap(short, long, global = true)]
    output: Option<PathBuf>,

    /// Output format (auto-detected from extension, defaults to DOT)
    #[clap(short = 'O', long, global = true)]
    output_format: Option<OutputFormat>,

    /// Exit with code 1 if the graphs differ
    #[clap(long, global = true)]
    check: bool,

    #[clap(subcommand)]
    command: Command,
}

/// Shared positional arguments for the two input graphs.
#[derive(Debug, clap::Args)]
struct Inputs {
    /// The "before" graph file (use '-' for stdin)
    before: PathBuf,
    /// The "after" graph file (use '-' for stdin)
    after: PathBuf,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Annotated graph highlighting added/removed/changed/moved nodes and edges
    Annotate {
        #[command(flatten)]
        inputs: Inputs,
        /// Group added/removed nodes into DOT cluster subgraphs
        #[clap(long)]
        cluster: bool,
    },
    /// Tab-delimited listing of changed nodes and edges
    List {
        #[command(flatten)]
        inputs: Inputs,
    },
    /// Set difference: nodes only in the "before" graph
    Subtract {
        #[command(flatten)]
        inputs: Inputs,
    },
    /// Tab-delimited summary counts of changes
    Summary {
        #[command(flatten)]
        inputs: Inputs,
    },
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

    let inputs = match &args.command {
        Command::Annotate { inputs, .. } => inputs,
        Command::List { inputs } => inputs,
        Command::Subtract { inputs } => inputs,
        Command::Summary { inputs } => inputs,
    };

    let is_stdio = |p: &PathBuf| p.as_os_str() == "-";
    if is_stdio(&inputs.before) && is_stdio(&inputs.after) {
        eyre::bail!("at most one input can be '-' (stdin)");
    }

    let before = read_graph(&inputs.before, args.input_format)?;
    let after = read_graph(&inputs.after, args.input_format)?;

    tracing::info!(
        "Before: {} nodes, {} edges; After: {} nodes, {} edges",
        before.all_nodes().len(),
        before.all_edges().len(),
        after.all_nodes().len(),
        after.all_edges().len()
    );

    let graph_diff = diff::diff(&before, &after);

    let output_path = args.output.filter(|p| !is_stdio(p));
    let mut output = get_output_writer(&output_path)?;

    match &args.command {
        Command::Annotate { cluster, .. } => {
            let graph = diff::annotate_graph(&graph_diff, &after, *cluster);
            let output_format = csvizmo_depgraph::emit::resolve_output_format(
                args.output_format,
                output_path.as_deref(),
            )?;
            csvizmo_depgraph::emit::emit(output_format, &graph, &mut output)?;
        }
        Command::List { .. } => {
            diff::write_list(&graph_diff, &mut output)?;
        }
        Command::Subtract { .. } => {
            let graph = diff::subtract_graph(&graph_diff, &before);
            let output_format = csvizmo_depgraph::emit::resolve_output_format(
                args.output_format,
                output_path.as_deref(),
            )?;
            csvizmo_depgraph::emit::emit(output_format, &graph, &mut output)?;
        }
        Command::Summary { .. } => {
            diff::write_summary(&graph_diff, &mut output)?;
        }
    }

    if args.check && graph_diff.has_changes() {
        std::process::exit(1);
    }

    Ok(())
}

/// Read and parse a graph from a file path (or stdin if "-").
fn read_graph(path: &Path, input_format: Option<InputFormat>) -> eyre::Result<DepGraph> {
    let file_path = if path.as_os_str() == "-" {
        None
    } else {
        Some(path.to_path_buf())
    };
    let mut reader = get_input_reader(&file_path)?;
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    let fmt =
        csvizmo_depgraph::parse::resolve_input_format(input_format, file_path.as_deref(), &text)?;
    csvizmo_depgraph::parse::parse(fmt, &text)
}
