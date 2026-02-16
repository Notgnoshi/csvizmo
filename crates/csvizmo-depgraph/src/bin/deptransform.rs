use std::io::{IsTerminal, Read};
use std::path::PathBuf;

use clap::{Parser, Subcommand};
use csvizmo_depgraph::algorithm::shorten::ShortenArgs;
use csvizmo_depgraph::algorithm::sub::{SubKey, Substitution};
use csvizmo_depgraph::emit::OutputFormat;
use csvizmo_depgraph::parse::InputFormat;
use csvizmo_depgraph::{DepGraph, algorithm};
use csvizmo_utils::stdio::{get_input_reader, get_output_writer};

/// Arguments for the `sub` subcommand.
#[derive(Debug, clap::Parser)]
struct SubArgs {
    /// Sed-style substitution: s/pattern/replacement/
    ///
    /// Uses Rust regex syntax: (...) for capture groups, $1/${name} in replacement.
    /// Supports alternate delimiters: s|...|...|, s#...#...#, etc.
    expr: String,

    /// Field to apply substitution to: id, node:NAME, or edge:NAME
    #[clap(long, default_value = "id")]
    key: String,
}

/// Arguments for the `merge` subcommand.
#[derive(Debug, clap::Parser)]
struct MergeArgs {
    /// Input files to merge (use '-' for stdin, at most once).
    /// The global --input/-i flag, if set, is included as an additional file.
    #[clap(required = true)]
    files: Vec<PathBuf>,
}

/// Structural transformations on dependency graphs.
///
/// Operations are performed via subcommands.
/// Chain operations by piping: deptransform ... | deptransform ...
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
    /// Reverse the direction of all edges
    Reverse,
    /// Remove redundant edges via transitive reduction
    Simplify,
    /// Shorten node IDs and/or labels using path transforms
    Shorten(ShortenArgs),
    /// Apply sed-style regex substitution to graph fields
    ///
    /// Uses Rust regex syntax: (...) for capture groups, $1/${name} in replacement.
    /// When applied to node IDs, nodes that map to the same ID are merged.
    Sub(SubArgs),
    /// Merge multiple graphs into one
    ///
    /// Nodes are unioned by ID (later files overwrite on collision).
    /// Edges are deduplicated by (from, to); first label wins, attributes are merged.
    /// The global --input/-i flag, if set, is included as the first file.
    Merge(MergeArgs),
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
    let output_path = args.output.filter(|p| !is_stdio(p));
    let output_format =
        csvizmo_depgraph::emit::resolve_output_format(args.output_format, output_path.as_deref())?;

    let graph = match &args.command {
        // Merge can't handle the same input handling as the rest of the commands
        Command::Merge(merge_args) => {
            let mut files = Vec::new();
            if let Some(input) = &args.input {
                files.push(input);
            }
            files.extend(&merge_args.files);
            if files.len() < 2 {
                eyre::bail!("merge requires at least 2 input files");
            }
            let mut graphs = Vec::new();
            for file in &files {
                graphs.push(read_graph(Some(file), args.input_format)?);
            }
            algorithm::merge::merge(&graphs)
        }
        command => {
            let graph = read_graph(args.input.as_ref(), args.input_format)?;
            tracing::info!(
                "Parsed graph with {} nodes, {} edges, and {} subgraphs",
                graph.all_nodes().len(),
                graph.all_edges().len(),
                graph.subgraphs.len()
            );

            match command {
                Command::Reverse => algorithm::reverse::reverse(&graph),
                Command::Simplify => algorithm::simplify::simplify(&graph)?,
                Command::Shorten(shorten_args) => {
                    let transforms = algorithm::shorten::build_transforms(shorten_args);
                    algorithm::shorten::shorten(
                        &graph,
                        &shorten_args.separator,
                        shorten_args.key,
                        &transforms,
                    )
                }
                Command::Sub(sub_args) => {
                    let substitution = Substitution::parse(&sub_args.expr)?;
                    let key = SubKey::parse(&sub_args.key)?;
                    algorithm::sub::sub(&graph, &substitution, &key)
                }
                Command::Merge(_) => unreachable!(),
            }
        }
    };

    let mut output = get_output_writer(&output_path)?;
    csvizmo_depgraph::emit::emit(output_format, &graph, &mut output)?;

    Ok(())
}

/// Read and parse a graph from a file path (or stdin if None / "-").
fn read_graph(path: Option<&PathBuf>, input_format: Option<InputFormat>) -> eyre::Result<DepGraph> {
    let is_stdio = |p: &PathBuf| p.as_os_str() == "-";
    let file_path: Option<PathBuf> = path.filter(|p| !is_stdio(p)).cloned();
    let mut reader = get_input_reader(&file_path)?;
    let mut text = String::new();
    reader.read_to_string(&mut text)?;
    let fmt =
        csvizmo_depgraph::parse::resolve_input_format(input_format, file_path.as_deref(), &text)?;
    csvizmo_depgraph::parse::parse(fmt, &text)
}
