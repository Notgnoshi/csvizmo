use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use csvizmo::csv::{column_index, exit_after_first_failed_read, parse_multi_columns};
use csvizmo::plot::Axes2DExt;
use csvizmo::stats::OnlineStats;
use csvizmo::stdio::get_input_reader;
use gnuplot::AxesCommon;

/// Calculate summary statistics for a column in a CSV
///
/// The column data must be convertible to f64, and anything that fails to parse will be skipped.
#[derive(Debug, Parser)]
#[clap(version, verbatim_doc_comment)]
struct Args {
    #[clap(short, long, default_value_t = tracing::Level::INFO)]
    log_level: tracing::Level,

    /// Dump gnuplot commands to stdout
    #[clap(short, long)]
    verbose: bool,

    /// Path to the input. stdin if '-' or if not passed
    input: Option<PathBuf>,

    /// Indicate the input CSV does not have a header
    #[clap(long)]
    no_header: bool,

    /// CSV delimiter
    #[clap(short, long, default_value_t = ',')]
    delimiter: char,

    /// The CSV column name(s) or indices to calculate statistics for
    ///
    /// May be given multiple times. Accepts a comma-delimited list.
    #[clap(short, long, value_delimiter = ',', required = true)]
    column: Vec<String>,

    /// If plotting a histogram, generate one bin for each unique value, centered on the value
    ///
    /// If not set, the variable will be assumed to be continuous, and the bins will be linspaced
    /// between [min..max]
    #[clap(long)]
    discrete: bool,

    /// Filter out values less than this minimum
    #[clap(short = 'm', long)]
    min: Option<f64>,

    /// Filter out values greater than this maximum
    #[clap(short = 'M', long)]
    max: Option<f64>,

    /// Plot a histogram
    #[clap(short = 'H', long)]
    histogram: bool,

    /// Use the given number of histogram bins
    ///
    /// If not given, use the Freedman-Diaconis rule to determine the bin-width and number of bins.
    #[clap(short, long)]
    bins: Option<usize>,
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

    let parse_start = Instant::now();

    let has_header = !args.no_header;
    let input = get_input_reader(&args.input)?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(has_header)
        .delimiter(args.delimiter as u8)
        .from_reader(input);

    let mut columns = Vec::new();
    for col_name in &args.column {
        let col_idx = column_index(&mut reader, col_name)?;
        columns.push(col_idx);
    }

    let records = exit_after_first_failed_read(reader.into_records());
    let mut data = parse_multi_columns(records, &columns);
    assert_eq!(
        data.len(),
        args.column.len(),
        "Did not parse as many columns as --column arguments"
    );
    assert_eq!(
        data.len(),
        columns.len(),
        "Did not parse as many columns as column indices"
    );

    tracing::info!(
        "Parsed {} rows after {:?}",
        data[0].len(),
        parse_start.elapsed()
    );
    let stats_start = Instant::now();

    let mut all_stats = Vec::new();
    for (colname, col_data) in args.column.iter().zip(&mut data) {
        let stats = OnlineStats::from_unsorted_mut(col_data, args.min, args.max);

        println!("Stats for column {colname:?}:");
        println!("{stats}");

        all_stats.push(stats);
    }

    tracing::info!(
        "Calculated stats after {:?} (total {:?})",
        stats_start.elapsed(),
        parse_start.elapsed()
    );
    if !args.histogram {
        return Ok(());
    }

    for (colname, col_data, stats) in itertools::izip!(args.column.iter(), data, all_stats) {
        let mut fig = gnuplot::Figure::new();
        let axes = fig.axes2d();

        if args.discrete {
            axes.histplot_discrete(col_data, &stats, args.min, args.max, args.bins);
        } else {
            axes.histplot_continuous(col_data, &stats, args.min, args.max, args.bins);
        }

        axes.set_x_label(colname, &[]);
        axes.set_x_grid(true);
        axes.set_y_grid(true);
        if let Some(path) = args.input.as_ref() {
            let name = path.file_stem().unwrap().to_string_lossy();
            fig.set_title(&name);
            fig.set_pre_commands(&format!("set terminal qt title '{name}'"));
        } else {
            fig.set_pre_commands("set terminal qt title 'csvstats'");
        }
        if args.verbose {
            fig.echo(&mut std::io::stdout());
        }
        fig.show()?;
    }

    Ok(())
}
