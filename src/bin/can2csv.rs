use std::io::{BufReader, IsTerminal};
use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use csv::Writer;
use csvizmo::can::{CandumpParser, reconstruct_transport_sessions};
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

    // I don't expect anything parsing FP, TP, or ETP messages to operate on this CSV output. This
    // is done just for testing, and deeper troubleshooting when sessions aren't quite right.
    /// Reconstruct transport layer sessions
    ///
    /// Intermediate frames for a session will not be produced, only the final product after
    /// reconstruction.
    #[clap(short, long)]
    reconstruct: bool,

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
    let input = BufReader::new(input);
    let output = get_output_writer(&args.output)?;
    let mut writer = Writer::from_writer(output);

    let start = Instant::now();

    let msgs = CandumpParser::new(input);
    if args.reconstruct {
        let msgs = msgs.filter_map(|f| {
            f.inspect_err(|e| tracing::warn!("Failed to parse msg: {e}"))
                .ok()
        });
        let msgs = reconstruct_transport_sessions(msgs);
        // Yeah, there's some copy-pasta, but one of these is an iterator of CanFrames, and the
        // other an iterator of CanMessages. I could make that work, or I could just copy paste and
        // move on.
        for msg in msgs {
            match msg {
                Err(e) => tracing::warn!("Failed to parse msg: {e}"),
                Ok(msg) => {
                    if let Err(e) = writer.serialize(msg) {
                        tracing::warn!("Failed to serialize msg: {e}");
                    }
                }
            }
            if should_line_buffer {
                let _eat_err = writer.flush();
            }
        }
    } else {
        for msg in msgs {
            match msg {
                Err(e) => tracing::warn!("Failed to parse msg: {e}"),
                Ok(msg) => {
                    if let Err(e) = writer.serialize(msg) {
                        tracing::warn!("Failed to serialize msg: {e}");
                    }
                }
            }
            if should_line_buffer {
                let _eat_err = writer.flush();
            }
        }
    }
    let _eat_err = writer.flush();

    tracing::info!("Finished after {:?}", start.elapsed());

    Ok(())
}
