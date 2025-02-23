use std::io::{IsTerminal, Write};
use std::path::PathBuf;

use clap::Parser;
use csvizmo::csv::column_index;
use csvizmo::stdio::{get_input_reader, get_output_writer};
use eyre::WrapErr;

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

    // Write the header, with the new column to the output file
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

    if args.center_mean {
        tracing::debug!("Mean-centering column {column_index}");
        let records = skip_failed_records(input.into_records());
        // Calculating the mean requires reading all the records into memory. An alternative could
        // be just to read the input file twice, which for very very large files, might be better?
        let records: Vec<_> = records.collect();
        let mean = mean(&records, column_index)?;
        // TODO: This unfortunately parses each field as an f64 twice since we can't stuff the
        // parsed value into the csv::StringRecord as an f64. If we wanted to use even more memory,
        // we could build a parallel array of f64s to avoid the second str.parse()
        center(records.into_iter(), &mut output, column_index, mean)?;
    } else if args.center_first {
        tracing::debug!("Centering column {column_index} around its first value");
        let mut records = skip_failed_records(input.into_records());
        let mut first_record = records.next().ok_or(eyre::eyre!("No rows in input CSV"))?;
        let first = parse_field(&first_record, column_index)?;
        center_record(&mut first_record, column_index, first)?;
        output.write_record(first_record.iter())?;
        center(records, &mut output, column_index, first)?;
    } else if let Some(value) = args.center_value {
        tracing::debug!("Centering column {column_index} around the value {value}");
        let records = skip_failed_records(input.into_records());
        center(records, &mut output, column_index, value)?;
    } else {
        let records = skip_failed_records(input.into_records());
        inter_row_deltas(records, &mut output, column_index)?;
    }

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

fn parse_field(record: &csv::StringRecord, index: usize) -> eyre::Result<f64> {
    let field = record
        .get(index)
        .ok_or(eyre::eyre!("Record {record:?} missing field {index}"))?;
    let value: f64 = field
        .parse()
        .wrap_err(format!("Failed to parse field {field:?} as f64"))?;
    Ok(value)
}

fn center_record(record: &mut csv::StringRecord, index: usize, center: f64) -> eyre::Result<()> {
    let value = parse_field(record, index)?;
    let centered = value - center;
    record.push_field(&format!("{centered}"));
    Ok(())
}

// TODO: Pull record parsing out
fn skip_failed_records(
    records: impl Iterator<Item = Result<csv::StringRecord, csv::Error>>,
) -> impl Iterator<Item = csv::StringRecord> {
    records.filter_map(|record| match record {
        Ok(r) => Some(r),
        Err(e) => {
            tracing::warn!("Skipping record: {e}");
            None
        }
    })
}

// TODO: Pull this out, and refactor to return the rolling stats
fn mean(records: &[csv::StringRecord], index: usize) -> eyre::Result<f64> {
    // Uses Welford's algorithm for rolling variance, which doesn't require summing all the values,
    // and *then* dividing by N, which means it's more suitable to lots and lots of values.
    let mut stats = rolling_stats::Stats::<f64>::new();
    for record in records {
        let value = parse_field(record, index)?;
        stats.update(value);
    }

    Ok(stats.mean)
}

fn center<W: Write>(
    records: impl Iterator<Item = csv::StringRecord>,
    writer: &mut csv::Writer<W>,
    index: usize,
    center: f64,
) -> eyre::Result<()> {
    for mut record in records {
        center_record(&mut record, index, center)?;
        writer.write_record(record.iter())?;
    }
    Ok(())
}

fn inter_row_deltas<W: Write>(
    records: impl Iterator<Item = csv::StringRecord>,
    writer: &mut csv::Writer<W>,
    index: usize,
) -> eyre::Result<()> {
    let mut prev_value = None;
    for mut record in records {
        let value = parse_field(&record, index)?;
        if let Some(prev) = prev_value {
            let delta = value - prev;
            record.push_field(&format!("{delta}"));
        } else {
            record.push_field("");
        }
        prev_value = Some(value);

        writer.write_record(record.iter())?;
    }
    Ok(())
}
