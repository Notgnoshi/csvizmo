use std::io::IsTerminal;
use std::path::PathBuf;

use clap::Parser;
use csv::Writer;
use csvizmo::candump::CandumpParser;
use csvizmo::stdio::{get_input_reader, get_output_writer};

/// Convert a can-utils candump to a CSV
///
#[derive(Debug, Parser)]
#[clap(version, verbatim_doc_comment)]
struct Args {
    #[clap(short, long, default_value_t = tracing::Level::INFO)]
    log_level: tracing::Level,

    /// Path to the input. stdin if '-' or if not passed
    input: Option<PathBuf>,

    /// Path to the output. stdout if '-' or if not passed
    output: Option<PathBuf>,

    /// Disable line-buffering on the CSV output
    ///
    /// If line-buffering is disabled, the output will be buffered with a much larger buf size.
    #[clap(long)]
    no_line_buffer: bool,
}

fn main() -> eyre::Result<()> {
    let use_color = std::io::stderr().is_terminal();
    if use_color {
        color_eyre::install()?;
    }

    let args = Args::parse();
    let should_line_buffer = !args.no_line_buffer;

    let filter = tracing_subscriber::EnvFilter::builder()
        .with_default_directive(args.log_level.into())
        .with_env_var("CSV_LOG")
        .from_env_lossy();
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_ansi(use_color)
        .with_writer(std::io::stderr)
        .init();

    let input = get_input_reader(&args.input)?;
    let output = get_output_writer(&args.output)?;
    let mut writer = Writer::from_writer(output);

    let msgs = CandumpParser::new(input);
    for msg in msgs {
        match msg {
            Err(e) => tracing::warn!("Failed to parse msg: {e:?}"),
            Ok(msg) => {
                if let Err(e) = writer.serialize(msg) {
                    tracing::warn!("Failed to serialize msg: {e:?}");
                }
            }
        }
        if should_line_buffer {
            let _eat_err = writer.flush();
        }
    }
    let _eat_err = writer.flush();

    Ok(())
}
