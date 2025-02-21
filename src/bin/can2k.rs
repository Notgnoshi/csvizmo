use std::io::{BufReader, IsTerminal};
use std::path::PathBuf;

use clap::Parser;
use csv::Writer;
use csvizmo::can::{parse_n2k_gps, reconstruct_transport_sessions, CandumpParser};
use csvizmo::stdio::{get_input_reader, get_output_writer};

/// Parse NMEA 2000 GPS data out of a candump
#[derive(Debug, Parser)]
#[clap(version, verbatim_doc_comment)]
struct Args {
    #[clap(short, long, default_value_t = tracing::Level::INFO)]
    log_level: tracing::Level,

    /// Path to the input. stdin if '-' or if not passed
    input: Option<PathBuf>,

    /// Path to the output. stdout if '-' or if not passed
    output: Option<PathBuf>,
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

    let input = get_input_reader(&args.input)?;
    let input = BufReader::new(input);

    let output = get_output_writer(&args.output)?;
    let mut writer = Writer::from_writer(output);

    let msgs = CandumpParser::new(input);
    let msgs = msgs.filter_map(|f| {
        f.inspect_err(|e| tracing::warn!("Failed to parse msg: {e}"))
            .ok()
    });
    let msgs = reconstruct_transport_sessions(msgs);
    let msgs = msgs.filter_map(|m| {
        m.inspect_err(|e| tracing::warn!("Failed to reconstruct msg: {e}"))
            .ok()
    });
    let msgs = parse_n2k_gps(msgs);
    for msg in msgs {
        if let Err(e) = writer.serialize(msg) {
            tracing::warn!("Failed to serialize msg: {e}");
        }
    }

    Ok(())
}
