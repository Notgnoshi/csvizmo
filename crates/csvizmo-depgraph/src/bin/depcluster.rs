use std::io::{IsTerminal, Read};
use std::path::PathBuf;

use clap::Parser;
use csvizmo_depgraph::algorithm::cluster::{graphrs_bridge, lpa};
use csvizmo_depgraph::emit::OutputFormat;
use csvizmo_depgraph::parse::InputFormat;
use csvizmo_utils::stdio::{get_input_reader, get_output_writer};

/// Cluster nodes in a dependency graph using community detection algorithms.
///
/// Runs a community detection algorithm on the input graph and outputs the result
/// with one subgraph per cluster. Cross-cluster edges appear at the top level.
#[derive(Debug, Parser)]
#[clap(version, verbatim_doc_comment)]
struct Args {
    /// Logging level
    #[clap(long, default_value_t = tracing::Level::INFO)]
    log_level: tracing::Level,

    /// Input file (stdin if '-' or omitted)
    #[clap(short, long)]
    input: Option<PathBuf>,

    /// Input format (auto-detected from extension/content if omitted)
    #[clap(short = 'I', long)]
    input_format: Option<InputFormat>,

    /// Output file (stdout if '-' or omitted)
    #[clap(short, long)]
    output: Option<PathBuf>,

    /// Output format (auto-detected from extension, defaults to DOT)
    #[clap(short = 'O', long)]
    output_format: Option<OutputFormat>,

    /// Clustering algorithm
    #[clap(short, long, default_value_t, value_enum)]
    algorithm: Algorithm,

    /// Use directed edges only (default: undirected/bidirectional)
    #[clap(long)]
    directed: bool,

    /// Maximum iterations (LPA only)
    #[clap(long, default_value_t = 100)]
    max_iter: usize,

    /// Random seed (LPA: shuffle order; Louvain: reproducibility)
    #[clap(long)]
    seed: Option<u64>,

    /// Resolution parameter; higher = more clusters (Louvain/Leiden only)
    #[clap(long, default_value_t = 1.0)]
    resolution: f64,
}

#[derive(Clone, Copy, Debug, Default, clap::ValueEnum)]
enum Algorithm {
    Lpa,
    #[default]
    Louvain,
    Leiden,
}

impl std::fmt::Display for Algorithm {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Algorithm::Lpa => write!(f, "lpa"),
            Algorithm::Louvain => write!(f, "louvain"),
            Algorithm::Leiden => write!(f, "leiden"),
        }
    }
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

    let graph = match args.algorithm {
        Algorithm::Lpa => lpa::lpa(&graph, args.directed, args.max_iter, args.seed),
        Algorithm::Louvain => {
            graphrs_bridge::louvain_clustering(&graph, args.directed, args.resolution, args.seed)?
        }
        Algorithm::Leiden => {
            graphrs_bridge::leiden_clustering(&graph, args.directed, args.resolution)?
        }
    };

    let mut output = get_output_writer(&output_path)?;
    csvizmo_depgraph::emit::emit(output_format, &graph, &mut output)?;

    Ok(())
}
