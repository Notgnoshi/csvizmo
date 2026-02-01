use std::fs::File;
use std::io::{BufRead, Read, Write};
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

/// Get a reader for the given path.
///
/// If `-` or if `None`, use stdin, otherwise use the given file
pub fn get_input_reader(input: &Option<PathBuf>) -> eyre::Result<Box<dyn Read>> {
    match input {
        None => Ok(Box::new(std::io::stdin())),
        Some(path) if path.as_os_str() == "-" => Ok(Box::new(std::io::stdin())),
        Some(path) => {
            let file = File::open(path).wrap_err(format!("Failed to open input file: {path:?}"))?;
            Ok(Box::new(file))
        }
    }
}

/// Read paths from a reader, one per line, trimming whitespace and skipping empty lines
pub fn read_paths_from_reader<R: BufRead>(reader: R) -> eyre::Result<Vec<PathBuf>> {
    let mut paths = Vec::new();
    for line in reader.lines() {
        let line = line?;
        let line = line.trim();
        if !line.is_empty() {
            paths.push(PathBuf::from(line));
        }
    }
    Ok(paths)
}

/// Read paths from a slice of input paths and/or a reader
///
/// If the inputs slice is empty, read from the reader. Otherwise, for each input:
/// - If the input is "-", read paths from the reader at that position
/// - Otherwise, use the input path as-is
///
/// This allows interleaving stdin with explicit paths, e.g. `file1.rs - file2.rs`
pub fn read_inputs<R: BufRead>(inputs: &[PathBuf], mut reader: R) -> eyre::Result<Vec<PathBuf>> {
    let mut paths = Vec::with_capacity(inputs.len());

    if !inputs.is_empty() {
        for input in inputs {
            if input.as_os_str() == "-" {
                paths.extend(read_paths_from_reader(&mut reader)?);
            } else {
                paths.push(input.clone());
            }
        }
    } else {
        paths.extend(read_paths_from_reader(reader)?);
    }
    Ok(paths)
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;

    use super::*;

    #[test]
    fn test_read_inputs_empty_args_reads_stdin() {
        let input = "/foo/bar.rs\n/baz/qux.rs\n";
        let reader = BufReader::new(input.as_bytes());
        let paths = read_inputs(&[], reader).unwrap();
        assert_eq!(
            paths,
            vec![PathBuf::from("/foo/bar.rs"), PathBuf::from("/baz/qux.rs")]
        );
    }

    #[test]
    fn test_read_inputs_with_args_only() {
        let input = "from_stdin.rs\n";
        let reader = BufReader::new(input.as_bytes());
        let args = vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")];
        let paths = read_inputs(&args, reader).unwrap();
        // from_stdin.rs was ignored
        assert_eq!(paths, vec![PathBuf::from("a.rs"), PathBuf::from("b.rs")]);
    }

    #[test]
    fn test_read_inputs_interleaving() {
        let input = "/from/stdin.rs\n";
        let reader = BufReader::new(input.as_bytes());
        let args = vec![
            PathBuf::from("a.rs"),
            PathBuf::from("-"),
            PathBuf::from("b.rs"),
        ];
        let paths = read_inputs(&args, reader).unwrap();
        assert_eq!(
            paths,
            vec![
                PathBuf::from("a.rs"),
                PathBuf::from("/from/stdin.rs"),
                PathBuf::from("b.rs")
            ]
        );
    }
}
