//! Utilities for parsing candumps
use std::io::{BufRead, BufReader, Lines, Read};

use eyre::WrapErr;

use crate::can::CanFrame;

/// Parse the given candump, skipping over any parse errors
pub fn parse_candump(candump: &str) -> impl Iterator<Item = CanFrame> {
    CandumpParser::new(candump.as_bytes()).filter_map(|r| r.ok())
}

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

/// Parse [CanFrame]s from the given reader
pub struct CandumpParser<R: Read> {
    format: CandumpFormat,
    lines: Lines<BufReader<R>>,
}

impl<R: Read> CandumpParser<R> {
    /// Create a new [CandumpParser] using [CandumpFormat::Auto]
    pub fn new(reader: R) -> Self {
        let reader = BufReader::new(reader);
        Self {
            format: CandumpFormat::Auto,
            lines: reader.lines(),
        }
    }

    /// Create a new [CandumpParser] using the given format
    pub fn with_format(reader: R, format: CandumpFormat) -> Self {
        let reader = BufReader::new(reader);
        Self {
            format,
            lines: reader.lines(),
        }
    }
}

/// There will be one Item for each input line. The iterator runs out when the input lines run out
impl<R: Read> Iterator for CandumpParser<R> {
    type Item = eyre::Result<CanFrame>;

    fn next(&mut self) -> Option<Self::Item> {
        let line = self.lines.next()?;
        match line {
            Ok(line) => Some(self.format.parse(&line)),
            Err(e) => Some(Err(eyre::eyre!("Failed to read line: {e}"))),
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
    if let Some(stripped) = field.strip_suffix(last) {
        stripped
    } else {
        field
    }
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
    Ok(CanFrame::new(
        timestamp,
        interface.to_string(),
        canid,
        dlc,
        data,
    ))
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

    Ok(CanFrame::new(
        timestamp,
        interface.to_string(),
        canid,
        dlc,
        data,
    ))
}

#[cfg(test)]
mod tests {
    use csv::Writer;

    use super::*;

    fn cli_format_fixture() -> (&'static str, CanFrame) {
        let line = "(1739136517.221471)  can0  123   [3]  0A B0 3f\n";
        let frame = CanFrame::new(
            1739136517.221471,
            String::from("can0"),
            0x123,
            3,
            [0x0A, 0xB0, 0x3F, 0, 0, 0, 0, 0],
        );
        (line, frame)
    }

    fn file_format_fixture() -> (&'static str, CanFrame) {
        let line = "(1739136482.503244) can0 123#0AB03f\n";
        let frame = CanFrame::new(
            1739136482.503244,
            String::from("can0"),
            0x123,
            3,
            [0x0A, 0xB0, 0x3F, 0, 0, 0, 0, 0],
        );
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
            CanFrame::new(
                01.0,
                String::from("can0"),
                0x123,
                1,
                [0x0A, 0, 0, 0, 0, 0, 0, 0],
            ),
            CanFrame::new(
                02.0,
                String::from("can0"),
                0x124,
                1,
                [0x0B, 0, 0, 0, 0, 0, 0, 0],
            ),
            CanFrame::new(
                03.0,
                String::from("can0"),
                0x125,
                1,
                [0x0C, 0, 0, 0, 0, 0, 0, 0],
            ),
        ];
        let actual: Vec<_> = CandumpParser::new(&lines[..])
            .filter_map(|m| m.ok())
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_parser_cli_format() {
        let lines = b"(01) can0 123 [1] 0A\n\
                      (02) can0 124 [1] 0B\n\
                      (03) can0 125 [1] 0C\n\
                     ";
        let expected = [
            CanFrame::new(
                01.0,
                String::from("can0"),
                0x123,
                1,
                [0x0A, 0, 0, 0, 0, 0, 0, 0],
            ),
            CanFrame::new(
                02.0,
                String::from("can0"),
                0x124,
                1,
                [0x0B, 0, 0, 0, 0, 0, 0, 0],
            ),
            CanFrame::new(
                03.0,
                String::from("can0"),
                0x125,
                1,
                [0x0C, 0, 0, 0, 0, 0, 0, 0],
            ),
        ];
        let actual: Vec<_> = CandumpParser::new(&lines[..])
            .filter_map(|m| m.ok())
            .collect();
        assert_eq!(actual, expected);
    }

    #[test]
    fn test_csv_format() {
        let lines = b"(01) can0 0CAC1C13#0AB0\n\
                      (02) can0 18EF1CF5#0BC0\n\
                      (03) can0 09F8051C#0CD0\n\
                     ";
        let msgs = CandumpParser::new(&lines[..]);

        let writer = Vec::<u8>::new();
        let mut writer = Writer::from_writer(writer);

        for msg in msgs {
            println!("{msg:?}");
            let msg = msg.unwrap();
            writer.serialize(msg).unwrap();
        }

        let bytes = writer.into_inner().unwrap();
        let csv_str = String::from_utf8(bytes).unwrap();
        let expected = "timestamp,interface,canid,dlc,priority,src,dst,pgn,data\n\
                        1.0,can0,0xCAC1C13,2,3,0x13,0x1C,0xAC00,0AB0\n\
                        2.0,can0,0x18EF1CF5,2,6,0xF5,0x1C,0xEF00,0BC0\n\
                        3.0,can0,0x9F8051C,2,2,0x1C,0xFF,0x1F805,0CD0\n\
                       ";
        assert_eq!(csv_str, expected);
    }
}
