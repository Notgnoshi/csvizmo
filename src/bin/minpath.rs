use std::io::IsTerminal;
use std::path::{Path, PathBuf};

use clap::Parser;
use csvizmo::minpath::ShortenedPaths;

/// Given a list of file paths, shrink each of them to the shortest unique path
///
/// The given paths do not need to exist on the filesystem.
#[derive(Debug, Parser)]
#[clap(version, verbatim_doc_comment)]
struct Args {
    #[clap(short, long, default_value_t = tracing::Level::INFO)]
    log_level: tracing::Level,

    /// Do not replace /home/<user> paths with ~
    #[clap(short = 'T', long)]
    no_tilde: bool,

    /// Do not try to resolve relative paths
    ///
    /// The filesystem won't be accessed, so not all relative paths can be resolved.
    #[clap(short = 'R', long)]
    no_resolve_relative: bool,

    /// Do not strip path prefix until a unique suffix is found
    #[clap(short = 'M', long)]
    no_minimal_suffix: bool,

    /// Shorten directory path components to single-letter abbreviations
    #[clap(short = 's', long)]
    single_letter: bool,

    /// Abbreviate source -> src, Documents -> docs, etcs
    #[clap(short = 'a', long)]
    smart_abbreviate: bool,

    /// Remove the given prefix if found; may be specified multiple times
    #[clap(short = 'p', long)]
    prefix: Vec<PathBuf>,

    /// Make paths relative to the specified ancestor
    #[clap(short, long)]
    relative_to: Option<PathBuf>,

    /// Sort and uniquify the output paths
    ///
    /// By default the output paths will be in the same order as the input paths, and are allowed
    /// to contain duplicates.
    #[clap(long)]
    sort: bool,

    /// Only output the shortened paths for the given patterns
    ///
    /// If not given, all input paths will be shortened and output.
    ///
    /// May be given multiple times. Supports '**', '*', '?', '{glob1,glob2}', '[az]' glob
    /// patterns. Patterns are matched against the full-length input paths before any
    /// transformations are applied.
    #[clap(long)]
    select: Vec<String>,

    /// Exclude the given patterns from the output
    ///
    /// If not given, all input paths matching the --select patterns will be shortnened and output.
    ///
    /// May be given multiple times. Supports '**', '*', '?', '{glob1,glob2}', '[az]' glob
    /// patterns. Patterns are matched against the full-length input paths before any
    /// transformations are applied.
    #[clap(short = 'x', long)]
    exclude: Vec<String>,

    /// Input paths; if not given, read from stdin
    input: Vec<PathBuf>,
}

fn sort_and_filter<'a>(
    shortened: &'a ShortenedPaths,
    sort: bool,
    select: &'a globset::GlobSet,
    exclude: &'a globset::GlobSet,
) -> Vec<&'a Path> {
    let mut pairs: Vec<_> = shortened.iter().collect();

    if sort {
        // Sort and dedup by the shortened path
        pairs.sort_unstable_by(|a, b| a.1.cmp(b.1));
        pairs.dedup_by(|a, b| a.1 == b.1);
    }

    pairs
        .into_iter()
        .filter_map(|(original, short)| {
            // An empty GlobSet matches nothing
            if select.is_empty() || select.is_match(original) {
                if exclude.is_match(original) {
                    return None;
                }
                Some(short)
            } else {
                None
            }
        })
        .collect()
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

    let reader = std::io::BufReader::new(std::io::stdin().lock());
    // Read once and held in memory; none of the transforms will modify the inputs since we likely
    // need to iterate over them multiple times during different transformations.
    let inputs = csvizmo::stdio::read_inputs(&args.input, reader)?;

    // User-specified prefixes are removed first, before any other transforms, so they are applied
    // to the untransformed paths rather than hidden intermediate transforms.
    let transforms = csvizmo::minpath::PathTransforms::new()
        .strip_prefix(args.prefix.clone())
        .home_dir(!args.no_tilde)
        .resolve_relative(!args.no_resolve_relative)
        .relative_to(args.relative_to.as_ref())
        .smart_abbreviate(args.smart_abbreviate)
        .strip_common_prefix(true)
        .minimal_unique_suffix(!args.no_minimal_suffix)
        .single_letter(args.single_letter);

    let shortened = transforms.build(&inputs);

    let mut selector = globset::GlobSet::builder();
    for pattern in &args.select {
        selector.add(globset::Glob::new(pattern)?);
    }
    let selector = selector.build()?;

    let mut excluder = globset::GlobSetBuilder::new();
    for pattern in &args.exclude {
        excluder.add(globset::Glob::new(pattern)?);
    }
    let excluder = excluder.build()?;

    let filtered = sort_and_filter(&shortened, args.sort, &selector, &excluder);
    for path in filtered {
        println!("{}", path.display());
    }

    Ok(())
}
