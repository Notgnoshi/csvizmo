use std::path::PathBuf;

use clap::Parser;
use csvizmo_utils::stdio::{get_input_reader, get_output_writer};

/// Concatenate CSV files to stdout
///
/// Requires shape of the CSV files match. Will use the header from the first file processed. All
/// files must have a header, or not have a header; mixing is not allowed.
#[derive(Debug, Parser)]
#[clap(version, verbatim_doc_comment)]
struct Args {
    #[clap(short, long, default_value_t = tracing::Level::INFO)]
    log_level: tracing::Level,

    /// Path to the input file. stdin if '-'
    input: Vec<PathBuf>,

    /// Allow concatenating CSV files with different shapes
    #[clap(long)]
    allow_ragged: bool,

    /// Indicate the input CSVs do not have a header
    #[clap(long)]
    no_header: bool,
}

/// Write the header from the given reader to the given writer
fn write_header_from(
    reader: &mut csv::Reader<Box<dyn std::io::Read>>,
    writer: &mut csv::Writer<Box<dyn std::io::Write>>,
) -> eyre::Result<()> {
    let header = reader.headers()?;
    if !header.is_empty() {
        writer.write_record(header.iter())?;
    }

    Ok(())
}

fn concatenate_from(
    reader: csv::Reader<Box<dyn std::io::Read>>,
    writer: &mut csv::Writer<Box<dyn std::io::Write>>,
    num_fields: &mut Option<usize>,
    allow_ragged: bool,
) -> eyre::Result<()> {
    for record in reader.into_records() {
        let record = record?;
        if let Some(num_fields) = num_fields {
            // the csv::Writer writes fields field-by-field, and only detects the raggedness after
            // writing a field that's out-of-bounds; So instead, we check for raggedness here and
            // avoid writing the ragged record at all;
            if !allow_ragged && record.len() != *num_fields {
                eyre::bail!("Ragged record detected! record: {record:?}");
            }
        } else {
            *num_fields = Some(record.len());
        }
        writer.write_record(record.iter())?;
    }
    Ok(())
}

fn main() -> eyre::Result<()> {
    color_eyre::install()?;

    let mut args = Args::parse();
    if args.input.is_empty() {
        args.input.push(PathBuf::from("-"));
    }

    let filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(args.log_level.into())
        .with_env_var("CSV_LOG")
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .init();

    let writer = get_output_writer(&None)?;
    let mut writer = csv::WriterBuilder::new()
        .flexible(args.allow_ragged)
        .has_headers(!args.no_header)
        .from_writer(writer);

    let mut num_fields = None;
    for (index, path) in args.input.into_iter().enumerate() {
        let reader = get_input_reader(&Some(path))?;
        let mut reader = csv::ReaderBuilder::new()
            .flexible(args.allow_ragged)
            .has_headers(!args.no_header)
            .from_reader(reader);

        // Write the header from the first file only
        if index == 0 && !args.no_header {
            write_header_from(&mut reader, &mut writer)?;
        }

        concatenate_from(reader, &mut writer, &mut num_fields, args.allow_ragged)?;
    }

    Ok(())
}
