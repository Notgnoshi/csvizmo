use std::io::{IsTerminal, Read, Write};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use csvizmo_depgraph::algorithm::query::edges::EdgesArgs;
use csvizmo_depgraph::algorithm::query::nodes::NodesArgs;
use csvizmo_depgraph::algorithm::query::{OutputFields, metrics};
use csvizmo_depgraph::parse::InputFormat;
use csvizmo_utils::stdio::get_input_reader;

/// Query properties of dependency graphs.
///
/// Produces plain text output (not graph output) answering
/// "what's in this graph?" -- listing nodes, edges, and computing metrics.
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

    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List nodes with optional filtering and sorting
    Nodes(NodesArgs),
    /// List edges with optional filtering and sorting
    Edges(EdgesArgs),
    /// Compute and display graph metrics
    Metrics,
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

    let mut input = get_input_reader(&input_path)?;
    let mut input_text = String::new();
    input.read_to_string(&mut input_text)?;

    let input_format = csvizmo_depgraph::parse::resolve_input_format(
        args.input_format,
        input_path.as_deref(),
        &input_text,
    )?;

    let graph = csvizmo_depgraph::parse::parse(input_format, &input_text)?;
    tracing::info!(
        "Parsed graph with {} nodes, {} edges, and {} subgraphs",
        graph.all_nodes().len(),
        graph.all_edges().len(),
        graph.subgraphs.len()
    );

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    match &args.command {
        Command::Nodes(nodes_args) => {
            let result = csvizmo_depgraph::algorithm::query::nodes::nodes(&graph, nodes_args)?;
            for (id, label, count) in &result {
                let field = match nodes_args.format {
                    OutputFields::Id => id.as_str(),
                    OutputFields::Label => label.as_str(),
                };
                match count {
                    Some(n) => writeln!(out, "{field}\t{n}")?,
                    None => writeln!(out, "{field}")?,
                }
            }
        }
        Command::Edges(edges_args) => {
            let result = csvizmo_depgraph::algorithm::query::edges::edges(&graph, edges_args)?;
            for (source, target, label) in &result {
                match label {
                    Some(l) if !l.is_empty() => writeln!(out, "{source}\t{target}\t{l}")?,
                    _ => writeln!(out, "{source}\t{target}")?,
                }
            }
        }
        Command::Metrics => {
            let m = metrics::metrics(&graph);
            write!(out, "{m}")?;
        }
    }

    Ok(())
}
