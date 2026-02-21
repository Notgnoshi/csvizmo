use std::io::{IsTerminal, Read};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use csvizmo_depgraph::algorithm;
use csvizmo_depgraph::algorithm::between::BetweenArgs;
use csvizmo_depgraph::algorithm::cycles::CyclesArgs;
use csvizmo_depgraph::algorithm::select::SelectArgs;
use csvizmo_depgraph::algorithm::slice::SliceArgs;
use csvizmo_depgraph::emit::OutputFormat;
use csvizmo_depgraph::parse::InputFormat;
use csvizmo_utils::stdio::{get_input_reader, get_output_writer};

/// Select or exclude nodes from dependency graphs.
///
/// Operations are performed via select, between, or cycles subcommands.
/// Chain operations by piping: depfilter ... | depfilter ...
#[derive(Debug, Parser)]
#[clap(version, verbatim_doc_comment)]
struct Args {
    /// Logging level
    #[clap(long, default_value_t = tracing::Level::INFO)]
    log_level: tracing::Level,

    /// Input file (stdin if '-' or omitted)
    #[clap(short, long, global = true)]
    input: Option<PathBuf>,

    /// Input format (auto-detected from extension/content if omitted)
    #[clap(short = 'I', long, global = true)]
    input_format: Option<InputFormat>,

    /// Output file (stdout if '-' or omitted)
    #[clap(short, long, global = true)]
    output: Option<PathBuf>,

    /// Output format (auto-detected from extension, defaults to DOT)
    #[clap(short = 'O', long, global = true)]
    output_format: Option<OutputFormat>,

    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// Select nodes matching patterns and optionally their deps/rdeps
    Select(SelectArgs),
    /// Extract the subgraph of all directed paths between matched query nodes
    Between(BetweenArgs),
    /// Detect cycles (strongly connected components) and output each as a subgraph
    Cycles(CyclesArgs),
    /// Cut edges between subgraphs, isolating each subgraph
    Slice(SliceArgs),
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

    let input_format = csvizmo_depgraph::parse::resolve_input_format(
        args.input_format,
        input_path.as_deref(),
        &input_text,
    )?;
    let output_format =
        csvizmo_depgraph::emit::resolve_output_format(args.output_format, output_path.as_deref())?;

    let graph = csvizmo_depgraph::parse::parse(input_format, &input_text)?;
    tracing::info!(
        "Parsed graph with {} nodes, {} edges, and {} subgraphs",
        graph.all_nodes().len(),
        graph.all_edges().len(),
        graph.subgraphs.len()
    );

    let graph = match &args.command {
        Command::Select(select_args) => algorithm::select::select(&graph, select_args)?,
        Command::Between(between_args) => algorithm::between::between(&graph, between_args)?,
        Command::Cycles(cycles_args) => algorithm::cycles::cycles(&graph, cycles_args)?,
        Command::Slice(slice_args) => algorithm::slice::slice(&graph, slice_args)?,
    };

    let mut output = get_output_writer(&output_path)?;
    csvizmo_depgraph::emit::emit(output_format, &graph, &mut output)?;

    Ok(())
}
