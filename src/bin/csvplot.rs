use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use csvizmo::csv::{column_index, exit_after_first_failed_read, parse_multi_columns};
use csvizmo::stdio::get_input_reader;
use gnuplot::AxesCommon;

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

    /// Dump gnuplot commands to stdout
    #[clap(short, long)]
    verbose: bool,

    /// Path to the input. stdin if '-' or if not passed
    input: Option<PathBuf>,
    /// Path to an output filename. PDF, EPS, PNG, SVG, or HTML file extensions are supported
    #[clap(short, long)]
    output: Option<PathBuf>,

    /// Indicate the input CSV does not have a header
    #[clap(long)]
    no_header: bool,

    /// CSV delimiter
    #[clap(short, long, default_value_t = ',')]
    delimiter: char,

    /// Whether to use a scatter plot or the default line plot
    #[clap(long)]
    scatter: bool,

    #[clap(long)]
    xmin: Option<f64>,
    #[clap(long)]
    xmax: Option<f64>,
    #[clap(long)]
    ymin: Option<f64>,
    #[clap(long)]
    ymax: Option<f64>,

    #[clap(long)]
    xlabel: Option<String>,
    #[clap(long)]
    ylabel: Option<String>,

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

    let parse_start = Instant::now();

    let header = reader.headers()?.clone();

    let mut ylabels = Vec::new();
    let mut column_indices = Vec::new();
    for y in args.y {
        let idx = column_index(&mut reader, y)?;
        if has_header {
            ylabels.push(header.get(idx).unwrap());
        }

        column_indices.push(idx);
    }
    let mut xlabel = "";
    // Put the X axis at the end, so that it's easier to add/remove if the X axis isn't specified.
    if let Some(x) = &args.x {
        let idx = column_index(&mut reader, x)?;
        if has_header {
            xlabel = header.get(idx).unwrap();
        }
        column_indices.push(idx);
    }
    let records = exit_after_first_failed_read(reader.into_records());
    let mut data = parse_multi_columns(records, &column_indices);
    assert!(!data.is_empty());

    tracing::info!("Parsed data after {:?}", parse_start.elapsed());

    let figure_start = Instant::now();
    let mut fig = gnuplot::Figure::new();
    let axes = fig.axes2d();

    if args.x.is_none() {
        tracing::info!("No X-axis column given. Proceeding with time series");
        xlabel = "time";
        let indices: Vec<_> = (0..data[0].len()).map(|i| i as f64).collect();
        data.push(indices);
    }
    if let Some(name) = &args.xlabel {
        xlabel = name;
    }

    let mut ylabel = "";
    if ylabels.len() == 1 {
        ylabel = ylabels[0];
    }
    if let Some(name) = &args.ylabel {
        ylabel = name;
    }
    // TODO: Disable LaTeX label formatting?
    axes.set_x_label(xlabel, &[]);
    axes.set_y_label(ylabel, &[]);

    let xs = data.remove(data.len() - 1);

    for (ys, yname) in data.iter().zip(ylabels) {
        let plotter = if args.scatter {
            gnuplot::Axes2D::points
        } else {
            gnuplot::Axes2D::lines
        };

        // TODO: Color each column differently
        // TODO: Try to mirror seaborn's default style
        let mut options = vec![gnuplot::PlotOption::LineWidth(2.0)];
        if has_header {
            options.push(gnuplot::PlotOption::Caption(yname));
        }
        plotter(axes, xs.iter(), ys, &options);
    }

    axes.set_x_range(
        args.xmin
            .map(gnuplot::AutoOption::Fix)
            .unwrap_or(gnuplot::AutoOption::Auto),
        args.xmax
            .map(gnuplot::AutoOption::Fix)
            .unwrap_or(gnuplot::AutoOption::Auto),
    );
    axes.set_y_range(
        args.ymin
            .map(gnuplot::AutoOption::Fix)
            .unwrap_or(gnuplot::AutoOption::Auto),
        args.ymax
            .map(gnuplot::AutoOption::Fix)
            .unwrap_or(gnuplot::AutoOption::Auto),
    );
    axes.set_x_grid(true);
    axes.set_y_grid(true);

    if let Some(path) = args.input {
        let name = path.file_stem().unwrap().to_string_lossy();
        fig.set_title(&name);
        fig.set_pre_commands(&format!("set terminal qt title '{name} csvplot'"));
    } else {
        fig.set_pre_commands("set terminal qt title 'csvplot'");
    }
    tracing::info!(
        "Created figure after {:?} (total {:?})",
        figure_start.elapsed(),
        parse_start.elapsed()
    );
    if args.verbose {
        fig.echo(&mut std::io::stdout());
    }
    if let Some(output) = args.output {
        let Some(ext) = output.extension() else {
            eyre::bail!("Output file must have a valid extension");
        };
        let terminal = match ext.to_string_lossy().as_ref() {
            "pdf" | "PDF" => "pdfcairo",
            "eps" | "EPS" => "epscairo",
            "png" | "PNG" => "pngcairo",
            "svg" | "SVG" => "svg",
            "html" | "HTML" => "canvas",
            _ => eyre::bail!("Unsupported output extension '{ext:?}'"),
        };

        fig.set_terminal(terminal, output.as_os_str().to_string_lossy().as_ref());
    }
    fig.show()?;

    Ok(())
}
