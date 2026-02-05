use std::io::{BufReader, BufWriter, IsTerminal, Write};
use std::path::PathBuf;
use std::time::Instant;

use clap::Parser;
use csvizmo_can::{CandumpParser, reconstruct_transport_sessions};
use csvizmo_utils::stdio::{get_input_reader, get_output_writer};

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
    let mut writer = BufWriter::new(output);

    let start = Instant::now();

    let msgs = CandumpParser::new(input);
    let msgs = msgs.filter_map(|f| {
        f.inspect_err(|e| tracing::warn!("Failed to parse msg: {e}"))
            .ok()
    });
    let msgs = reconstruct_transport_sessions(msgs);
    let msgs = msgs.filter_map(|m| {
        m.inspect_err(|e| tracing::error!("Failed to reconstruct session: {e}"))
            .ok()
    });
    for msg in msgs {
        msg.write(&mut writer)?;
    }
    writer.flush()?;

    tracing::info!("Finished in {:?}", start.elapsed());

    Ok(())
}
