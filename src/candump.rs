//! Utilities for parsing candumps
use std::io::{BufRead, Lines};

use eyre::WrapErr;

/// File format of the candump
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum CandumpFormat {
    /// Try to auto-negotiate the file format
    ///
    /// Assumes that all lines follow the same format, and will pick the first format to
    /// successfully parse a line.
    Auto,
    /// candump -L/-l format
    CanUtilsFile,
    /// candump -ta format
    CanUtilsCli,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanFrame {
    pub timestamp: f64,
    pub interface: String,
    pub canid: u32,
    pub dlc: usize,

    data: [u8; 8],
}

impl CanFrame {
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data[..self.dlc]
    }
}

/// Parse [CanFrame]s from the given reader
pub struct CandumpParser<R: BufRead> {
    format: CandumpFormat,
    lines: Lines<R>,
}

impl<R: BufRead> CandumpParser<R> {
    /// Create a new [CandumpParser] using [CandumpFormat::Auto]
    pub fn new(reader: R) -> Self {
        Self {
            format: CandumpFormat::Auto,
            lines: reader.lines(),
        }
    }

    /// Create a new [CandumpParser] using the given format
    pub fn with_format(reader: R, format: CandumpFormat) -> Self {
        Self {
            format,
            lines: reader.lines(),
        }
    }
}

/// There will be one Item for each input line. The iterator runs out when the input lines run out
impl<R: BufRead> Iterator for CandumpParser<R> {
    type Item = eyre::Result<CanFrame>;

    fn next(&mut self) -> Option<Self::Item> {
        let line = self.lines.next()?;
        match line {
            Ok(line) => Some(self.format.parse(&line)),
            Err(e) => Some(Err(eyre::eyre!("Failed to read line: {e:?}"))),
        }
    }
}

impl CandumpFormat {
    /// Attempt to parse a [CanFrame] from the given line
    pub fn parse(&mut self, line: &str) -> eyre::Result<CanFrame> {
        match self {
            CandumpFormat::Auto => {
                if let Ok(result) = parse_candump_file_msg(line) {
                    // Assume that all lines follow the format of the first successfully parsed
                    // line
                    *self = CandumpFormat::CanUtilsFile;
                    Ok(result)
                } else if let Ok(result) = parse_candump_cli_msg(line) {
                    *self = CandumpFormat::CanUtilsCli;
                    Ok(result)
                } else {
                    eyre::bail!("Failed to parse {line:?} with all known candump formats")
                }
            }
            CandumpFormat::CanUtilsFile => parse_candump_file_msg(line),
            CandumpFormat::CanUtilsCli => parse_candump_cli_msg(line),
        }
    }
}

fn strip_outer_brackets(field: &str, first: char, last: char) -> &str {
    let field = if let Some(stripped) = field.strip_prefix(first) {
        stripped
    } else {
        field
    };
    let field = if let Some(stripped) = field.strip_suffix(last) {
        stripped
    } else {
        field
    };
    field
}

/// Parse candumps with the format
///
/// ```text
/// $ candump -ta can0
/// (1739136517.221471)  can0  123   [3]  FF FF FF
/// ```
fn parse_candump_cli_msg(line: &str) -> eyre::Result<CanFrame> {
    let mut parts = line.split_ascii_whitespace();

    let Some(maybe_timestamp) = parts.next() else {
        eyre::bail!("Line {line:?} empty");
    };
    let maybe_timestamp = strip_outer_brackets(maybe_timestamp, '(', ')');
    let timestamp: f64 = maybe_timestamp
        .parse()
        .wrap_err("Failed to parse timestamp as f64")?;
    let Some(interface) = parts.next() else {
        eyre::bail!("Failed to parse interface from: {line:?}");
    };
    let Some(maybe_canid) = parts.next() else {
        eyre::bail!("Failed to parse canid from: {line:?}");
    };
    let canid = u32::from_str_radix(maybe_canid, 16).wrap_err("Failed to parse canid as u32")?;
    let Some(maybe_dlc) = parts.next() else {
        eyre::bail!("Failed to parse dlc from: {line:?}");
    };
    let maybe_dlc = strip_outer_brackets(maybe_dlc, '[', ']');
    let dlc: usize = maybe_dlc.parse().wrap_err("Failed to parse dlc as usize")?;
    if dlc > 8 {
        eyre::bail!("dlc {dlc} exceeds maximum dlc of 8 bytes");
    }

    let mut data = [0, 0, 0, 0, 0, 0, 0, 0];
    #[allow(clippy::needless_range_loop)]
    for i in 0..dlc {
        let Some(maybe_byte) = parts.next() else {
            eyre::bail!("Failed to parse data byte {i} from line: {line:?}");
        };
        if maybe_byte.len() != 2 {
            eyre::bail!(
                "Failed to parse data byte {i} from {maybe_byte:?}: incorrect string length"
            );
        }
        let byte = u8::from_str_radix(maybe_byte, 16).wrap_err("Failed to parse data byte")?;
        data[i] = byte;
    }
    Ok(CanFrame {
        timestamp,
        interface: interface.to_string(),
        canid,
        dlc,
        data,
    })
}

/// Parse candumps with the format
///
/// ```text
/// $ candump -L can0
/// (1739136482.503244) can0 123#FFFFFF
/// ```
fn parse_candump_file_msg(line: &str) -> eyre::Result<CanFrame> {
    let mut parts = line.split_ascii_whitespace();
    let Some(maybe_timestamp) = parts.next() else {
        eyre::bail!("Line {line:?} empty");
    };
    let maybe_timestamp = strip_outer_brackets(maybe_timestamp, '(', ')');
    let timestamp: f64 = maybe_timestamp
        .parse()
        .wrap_err("Failed to parse timestamp as f64")?;
    let Some(interface) = parts.next() else {
        eyre::bail!("Failed to parse interface from: {line:?}");
    };

    let Some(maybe_frame) = parts.next() else {
        eyre::bail!("Failed to parse frame data from: {line:?}");
    };
    let mut frame = maybe_frame.split('#');
    let Some(maybe_canid) = frame.next() else {
        eyre::bail!("Failed to parse canid from: {maybe_frame:?} in line {line:?}");
    };
    let canid = u32::from_str_radix(maybe_canid, 16).wrap_err("Failed to parse canid as u32")?;
    let Some(maybe_data) = frame.next() else {
        eyre::bail!("Failed to parse data from: {maybe_frame:?} in line {line:?}");
    };
    if maybe_data.len() > 16 || maybe_data.len() % 2 != 0 {
        eyre::bail!("Failed to parse data from: {maybe_data:?}: incorrect length");
    }
    let dlc = maybe_data.len() / 2;
    let mut data = [0, 0, 0, 0, 0, 0, 0, 0];
    #[allow(clippy::needless_range_loop)]
    for i in 0..dlc {
        let j = i * 2;
        let byte =
            u8::from_str_radix(&maybe_data[j..j + 2], 16).wrap_err("Failed to parse byte")?;
        data[i] = byte;
    }

    Ok(CanFrame {
        timestamp,
        interface: interface.to_string(),
        canid,
        dlc,
        data,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cli_format_fixture() -> (&'static str, CanFrame) {
        let line = "(1739136517.221471)  can0  123   [3]  0A B0 3f\n";
        let frame = CanFrame {
            timestamp: 1739136517.221471,
            interface: String::from("can0"),
            canid: 0x123,
            dlc: 3,
            data: [0x0A, 0xB0, 0x3F, 0, 0, 0, 0, 0],
        };
        (line, frame)
    }

    fn file_format_fixture() -> (&'static str, CanFrame) {
        let line = "(1739136482.503244) can0 123#0AB03f\n";
        let frame = CanFrame {
            timestamp: 1739136482.503244,
            interface: String::from("can0"),
            canid: 0x123,
            dlc: 3,
            data: [0x0A, 0xB0, 0x3F, 0, 0, 0, 0, 0],
        };
        (line, frame)
    }

    #[test]
    fn test_parse_cli_format() {
        let (line, expected) = cli_format_fixture();
        let actual = parse_candump_cli_msg(line).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_parse_file_format() {
        let (line, expected) = file_format_fixture();
        let actual = parse_candump_file_msg(line).unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_auto_cli() {
        let (line, expected) = cli_format_fixture();
        let mut format = CandumpFormat::Auto;
        let actual = format.parse(line).unwrap();
        assert_eq!(format, CandumpFormat::CanUtilsCli);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_auto_file() {
        let (line, expected) = file_format_fixture();
        let mut format = CandumpFormat::Auto;
        let actual = format.parse(line).unwrap();
        assert_eq!(format, CandumpFormat::CanUtilsFile);
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_parser_file_format() {
        let lines = b"(01) can0 123#0A\n
                      (02) can0 124#0B\n
                      (03) can0 125#0C\n
                     ";
        let expected = [
            CanFrame {
                timestamp: 01.0,
                interface: String::from("can0"),
                canid: 0x123,
                dlc: 1,
                data: [0x0A, 0, 0, 0, 0, 0, 0, 0],
            },
            CanFrame {
                timestamp: 02.0,
                interface: String::from("can0"),
                canid: 0x124,
                dlc: 1,
                data: [0x0B, 0, 0, 0, 0, 0, 0, 0],
            },
            CanFrame {
                timestamp: 03.0,
                interface: String::from("can0"),
                canid: 0x125,
                dlc: 1,
                data: [0x0C, 0, 0, 0, 0, 0, 0, 0],
            },
        ];
        let actual: Vec<_> = CandumpParser::new(&lines[..])
            .filter_map(|m| m.ok())
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_parser_cli_format() {
        let lines = b"(01) can0 123 [1] 0A\n
                      (02) can0 124 [1] 0B\n
                      (03) can0 125 [1] 0C\n
                     ";
        let expected = [
            CanFrame {
                timestamp: 01.0,
                interface: String::from("can0"),
                canid: 0x123,
                dlc: 1,
                data: [0x0A, 0, 0, 0, 0, 0, 0, 0],
            },
            CanFrame {
                timestamp: 02.0,
                interface: String::from("can0"),
                canid: 0x124,
                dlc: 1,
                data: [0x0B, 0, 0, 0, 0, 0, 0, 0],
            },
            CanFrame {
                timestamp: 03.0,
                interface: String::from("can0"),
                canid: 0x125,
                dlc: 1,
                data: [0x0C, 0, 0, 0, 0, 0, 0, 0],
            },
        ];
        let actual: Vec<_> = CandumpParser::new(&lines[..])
            .filter_map(|m| m.ok())
            .collect();
        assert_eq!(actual, expected);
    }
}
