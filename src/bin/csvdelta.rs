use std::io::IsTerminal;
use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use csvizmo::csv::{
    column_index, exit_after_first_failed_read, map_column_records, parse_column_records,
    parse_field,
};
use csvizmo::stats::OnlineStats;
use csvizmo::stdio::{get_input_reader, get_output_writer};

/// Compute inter-row deltas on a column from a CSV
#[derive(Debug, Parser)]
#[clap(version, verbatim_doc_comment)]
struct Args {
    #[clap(short, long, default_value_t = tracing::Level::INFO)]
    log_level: tracing::Level,

    /// Path to the input. stdin if '-' or if not passed
    input: Option<PathBuf>,

    /// Column name to operate on. May also be a zero-based column index.
    #[clap(short, long)]
    column: String,

    /// Indicate the input CSV does not have a header
    #[clap(long)]
    no_header: bool,

    /// CSV delimiter
    #[clap(short, long, default_value_t = ',')]
    delimiter: char,

    /// Path to the output. stdout if '-' or if not passed
    #[clap(conflicts_with = "in_place")]
    output: Option<PathBuf>,

    /// Output column name, if there is a header
    #[clap(short = 'C', long)]
    output_column: Option<String>,

    /// Modify the input file in-place (swap the input file with the output file)
    ///
    /// May not be used with an output file. Input must be a file, and not stdin.
    #[clap(short, long, conflicts_with = "output", requires = "input")]
    in_place: bool,

    /// Mean-center the specified column
    ///
    /// Requires reading the whole CSV into memory
    #[clap(long, group = "centering")]
    center_mean: bool,

    /// Center the specified column around its first value
    #[clap(long, group = "centering")]
    center_first: bool,

    /// Center the specified column around the given value
    #[clap(long, group = "centering")]
    center_value: Option<f64>,
}

fn main() -> eyre::Result<()> {
    let use_color = std::io::stderr().is_terminal();
    if use_color {
        color_eyre::install()?;
    }

    let mut args = Args::parse();

    let filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(args.log_level.into())
        .with_env_var("CSV_LOG")
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(use_color)
        .with_writer(std::io::stderr)
        .init();

    let mut tmp_output = None;
    if args.in_place {
        let input = args
            .input
            .as_ref()
            .expect("Clap enforces this is set for --in-place");
        if input.as_os_str() == "-" {
            eyre::bail!("Cannot use --in-place together with input from stdin");
        }

        // atomic swapping requires both files be on the same filesystem, so we need to get the
        // directory where the input file is located.
        let input_dir = args
            .input
            .as_ref()
            .expect("Clap enforces this is set for --in-place")
            .parent()
            .expect("The [input] file doesn't have a parent directory");
        let tmp = tempfile::Builder::new()
            .prefix(".csvdelta")
            .suffix(".csv")
            .keep(true)
            .tempfile_in(input_dir)?;
        args.output = Some(tmp.path().to_path_buf());
        tmp_output = Some(tmp);
    }

    let has_header = !args.no_header;
    let input = get_input_reader(&args.input)?;
    let mut input = csv::ReaderBuilder::new()
        .has_headers(has_header)
        .delimiter(args.delimiter as u8)
        .from_reader(input);

    let output = get_output_writer(&args.output)?;
    let mut output = csv::Writer::from_writer(output);

    let header = has_header.then_some(input.headers()?).cloned();

    // Find the column index of the input column
    let column_index = column_index(&mut input, args.column)?;

    // Write the new header to the output file
    if let Some(header) = header {
        let new_name = args.output_column.unwrap_or_else(|| {
            let old_name = header
                .get(column_index)
                .expect("Index is verified to exist above");
            if args.center_mean || args.center_first || args.center_value.is_some() {
                format!("{old_name}-centered")
            } else {
                format!("{old_name}-deltas")
            }
        });
        tracing::debug!("Adding new column with name {new_name:?}");

        let mut new_header = header.clone();
        new_header.push_field(&new_name);
        output.write_record(new_header.iter())?;
    }

    let start = Instant::now();

    if args.center_mean {
        tracing::info!("Mean-centering column {column_index}");
        let records = exit_after_first_failed_read(input.into_records());
        let records = parse_column_records(records, column_index);

        // Calculating the mean requires reading all the records into memory. An alternative could
        // be just to read the input file twice, which for very very large files, might be better?
        let records: Vec<_> = records.collect();
        let values = records.iter().flat_map(|(_rec, maybe_value)| maybe_value);
        let stats = OnlineStats::from_unsorted_iter(values);

        let records = map_column_records(records.into_iter(), |maybe_value| {
            maybe_value.map(|v| v - stats.mean).ok()
        });
        for record in records {
            output.write_record(record.iter())?;
        }
    } else if args.center_first {
        tracing::info!("Centering column {column_index} around its first value");
        let records = exit_after_first_failed_read(input.into_records());
        let mut records = records.peekable();

        let first_record = records.peek().ok_or(eyre::eyre!("No rows in input CSV"))?;
        let first = parse_field(first_record, column_index)?;

        let records = parse_column_records(records, column_index);
        let records =
            map_column_records(records, |maybe_value| maybe_value.map(|v| v - first).ok());
        for record in records {
            output.write_record(record.iter())?;
        }
    } else if let Some(value) = args.center_value {
        tracing::info!("Centering column {column_index} around the value {value}");
        let records = exit_after_first_failed_read(input.into_records());
        let records = parse_column_records(records, column_index);
        let records =
            map_column_records(records, |maybe_value| maybe_value.map(|v| v - value).ok());
        for record in records {
            output.write_record(record.iter())?;
        }
    } else {
        let records = exit_after_first_failed_read(input.into_records());
        let records = parse_column_records(records, column_index);
        let mut prev_value = None;
        let records = map_column_records(records, |maybe_value| {
            match maybe_value {
                Ok(value) => {
                    // Can't center until prev_value is set
                    let result = prev_value.map(|prev| value - prev);
                    prev_value = Some(value);
                    result
                }
                // Can't center if this record's value is missing
                Err(e) => {
                    tracing::warn!("Skipping delta calculation because: {e}");
                    prev_value = None;
                    None
                }
            }
        });
        for record in records {
            output.write_record(record.iter())?;
        }
    }

    tracing::info!("Finished after {:?}", start.elapsed());

    output.flush()?;

    // Now that we're done writing to the tmpfile, atomically replace the input file with it. This
    // requires the tmpfile and the input file be on the same filesystem.
    if let Some(output) = tmp_output {
        let input = args
            .input
            .expect("Clap enforces this is set for --in-place");
        output.persist(input)?;
    }

    Ok(())
}
