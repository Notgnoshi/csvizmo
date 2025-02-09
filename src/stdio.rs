use std::fs::File;
use std::io::{BufReader, Read, Write};
use std::path::PathBuf;

use eyre::WrapErr;

/// Get a writer for the given path.
///
/// If `-` or if `None`, use stdout, otherwise use the given file
///
/// The generated writer is _not_ buffered, because [csv::Writer] is buffered
pub fn get_output_writer(output: &Option<PathBuf>) -> eyre::Result<Box<dyn Write>> {
    match output {
        None => Ok(Box::new(std::io::stdout())),
        Some(path) if path.as_os_str() == "-" => Ok(Box::new(std::io::stdout())),
        Some(path) => {
            let file =
                File::create(path).wrap_err(format!("Failed to create output file: {path:?}"))?;
            Ok(Box::new(file))
        }
    }
}

/// Get a [BufReader] for the given path.
///
/// If `-` or if `None`, use stdin, otherwise use the given file
pub fn get_input_reader(input: &Option<PathBuf>) -> eyre::Result<BufReader<Box<dyn Read>>> {
    match input {
        None => Ok(BufReader::new(Box::new(std::io::stdin()))),
        Some(path) if path.as_os_str() == "-" => Ok(BufReader::new(Box::new(std::io::stdin()))),
        Some(path) => {
            let file = File::open(path).wrap_err(format!("Failed to open input file: {path:?}"))?;
            Ok(BufReader::new(Box::new(file)))
        }
    }
}
