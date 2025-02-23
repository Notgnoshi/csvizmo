use std::io::IsTerminal;
use std::path::PathBuf;

use clap::Parser;
use csvizmo::csv::{column_index, exit_after_first_failed_read, parse_multi_columns};
use csvizmo::stdio::get_input_reader;

/// Plot data from a CSV file
///
/// You may plot:
///
/// 1. A single Y column as a time series (row index will be used as X)
/// 2. One X column and multiple Y columns
///
/// The columns must be convertible to f64, and anything that fails to parse will result in a gap
/// in the plot.
#[derive(Debug, Parser)]
#[clap(version, verbatim_doc_comment)]
struct Args {
    #[clap(short, long, default_value_t = tracing::Level::INFO)]
    log_level: tracing::Level,

    /// Path to the input. stdin if '-' or if not passed
    input: Option<PathBuf>,

    /// Indicate the input CSV does not have a header
    #[clap(long)]
    no_header: bool,

    /// CSV delimiter
    #[clap(short, long, default_value_t = ',')]
    delimiter: char,

    /// Whether to use a scatter plot or the default line plot
    #[clap(long)]
    scatter: bool,

    /// Filter the x axis range to just the given inclusive range
    #[clap(long, number_of_values = 2, value_names = ["XMIN", "XMAX"])]
    xlim: Vec<f64>,

    /// Filter the y axis range to just the given inclusive range
    #[clap(long, number_of_values = 2, value_names = ["YMIN", "YMAX"])]
    ylim: Vec<f64>,

    /// The X column name or index. If not specified, the Y column will be treated as a time series
    #[clap(short, value_name = "X COLUMN")]
    x: Option<String>,

    /// The Y column name(s) or indices. May be given multiple times. Also accepts comma-delimited
    /// values
    #[clap(short, required = true, value_name = "Y COLUMN", value_delimiter = ',')]
    y: Vec<String>,
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

    let has_header = !args.no_header;
    let input = get_input_reader(&args.input)?;
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(has_header)
        .delimiter(args.delimiter as u8)
        .from_reader(input);

    let mut column_indices = Vec::new();
    for y in args.y {
        let idx = column_index(&mut reader, y)?;
        column_indices.push(idx);
    }
    // Put the X axis at the end, so that it's easier to add/remove if the X axis isn't specified.
    if let Some(x) = &args.x {
        let idx = column_index(&mut reader, x)?;
        column_indices.push(idx);
    }
    let records = exit_after_first_failed_read(reader.into_records());
    let mut data = parse_multi_columns(records, &column_indices);
    assert!(!data.is_empty());

    if args.x.is_none() {
        tracing::info!("No X-axis column given. Proceeding with time series");
        let indices: Vec<_> = (0..data[0].len()).map(|i| i as f64).collect();
        data.push(indices);
    }

    let xs = data.remove(data.len() - 1);
    let mut fig = gnuplot::Figure::new();
    for ys in data {
        fig.axes2d().lines(xs.iter(), ys, &[]);
    }
    // TODO: xlim,ylim
    // TODO: Scatter
    // TODO: Legend, title, axis labels
    fig.show()?;

    Ok(())
}
