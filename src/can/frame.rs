use std::io::Write;

use serde::ser::SerializeStruct;

#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanFrame {
    pub timestamp: f64,
    pub interface: String,
    pub canid: u32,
    pub dlc: usize,

    data: [u8; 8],
}

/// [CanFrame]s are restricted to 8-bytes, [CanMessage]s are arbitrarily sized
#[derive(Clone, Debug, Default, PartialEq)]
pub struct CanMessage {
    pub timestamp: f64,
    pub interface: String,
    pub canid: u32,
    pub priority: u8,
    pub pgn: u32,
    pub src: u8,
    pub dst: u8,
    pub dlc: usize,
    pub data: Vec<u8>,
}

impl From<CanFrame> for CanMessage {
    fn from(frame: CanFrame) -> CanMessage {
        let mut data: Vec<u8> = frame.data.into();
        data.truncate(frame.dlc);

        CanMessage {
            priority: frame.priority(),
            pgn: frame.pgn(),
            src: frame.src(),
            dst: frame.dst(),
            canid: frame.canid,
            dlc: frame.dlc,
            timestamp: frame.timestamp,
            interface: frame.interface,
            data,
        }
    }
}

impl CanMessage {
    pub fn new(timestamp: f64, interface: String, canid: u32, data: Vec<u8>) -> Self {
        Self {
            timestamp,
            interface,
            canid,
            priority: priority(canid),
            pgn: pgn(canid),
            src: src(canid),
            dst: dst(canid),
            dlc: data.len(),
            data,
        }
    }

    pub fn write<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writeln!(
            writer,
            "({:.6}) {} {}#{}",
            self.timestamp,
            self.interface,
            hex::encode_upper(self.canid.to_be_bytes()),
            hex::encode_upper(&self.data)
        )
    }
}

#[inline]
#[must_use]
fn dst(canid: u32) -> u8 {
    if is_point_to_point(canid) {
        pdu_specific(canid) as u8
    } else {
        0xFF // global
    }
}

#[inline]
#[must_use]
fn src(canid: u32) -> u8 {
    (canid & 0xFF) as u8
}

#[inline]
#[must_use]
fn priority(canid: u32) -> u8 {
    let shifted = canid >> 26;
    let masked = shifted & 0b0111;
    masked as u8
}

#[inline]
#[must_use]
fn is_point_to_point(canid: u32) -> bool {
    // destination-specific range is 00..=EF
    // broadcast range is F0..=FF
    pdu_format(canid) <= 0xEF
}

#[inline]
#[must_use]
fn pdu_format(canid: u32) -> u32 {
    (canid & 0xFF0000) >> 16
}

#[inline]
#[must_use]
fn pdu_specific(canid: u32) -> u32 {
    (canid & 0x00FF00) >> 8
}

#[inline]
#[must_use]
fn pgn(canid: u32) -> u32 {
    let orig = canid;
    // Shift off the src address
    let canid = canid >> 8;
    // Mask off the priority bits, leaving the EDP and DP data page bits
    let canid = canid & 0x3FFFF;

    if is_point_to_point(orig) {
        canid & 0x3FF00
    } else {
        canid
    }
}

impl CanFrame {
    pub fn write<W: Write>(&self, writer: &mut W) -> std::io::Result<()> {
        writeln!(
            writer,
            "({:.6}) {} {}#{}",
            self.timestamp,
            self.interface,
            hex::encode_upper(self.canid.to_be_bytes()),
            hex::encode_upper(self.data())
        )
    }
}

impl CanFrame {
    pub fn new(timestamp: f64, interface: String, canid: u32, dlc: usize, data: [u8; 8]) -> Self {
        Self {
            timestamp,
            interface,
            canid,
            dlc,
            data,
        }
    }

    #[inline]
    #[must_use]
    pub fn data(&self) -> &[u8] {
        &self.data[..self.dlc]
    }

    #[inline]
    #[must_use]
    pub fn dst(&self) -> u8 {
        dst(self.canid)
    }

    #[inline]
    #[must_use]
    pub fn src(&self) -> u8 {
        src(self.canid)
    }

    #[inline]
    #[must_use]
    pub fn priority(&self) -> u8 {
        priority(self.canid)
    }

    #[inline]
    #[must_use]
    pub fn is_point_to_point(&self) -> bool {
        is_point_to_point(self.canid)
    }

    #[inline]
    #[must_use]
    pub fn pdu_format(&self) -> u32 {
        pdu_format(self.canid)
    }

    #[inline]
    #[must_use]
    pub fn pdu_specific(&self) -> u32 {
        pdu_specific(self.canid)
    }

    #[inline]
    #[must_use]
    pub fn pgn(&self) -> u32 {
        pgn(self.canid)
    }
}

impl serde::Serialize for CanMessage {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("CanMessage", 9)?;
        state.serialize_field("timestamp", &self.timestamp)?;
        state.serialize_field("interface", &self.interface)?;
        state.serialize_field("canid", &format!("{:#X}", self.canid))?;
        state.serialize_field("dlc", &self.dlc)?;
        state.serialize_field("priority", &self.priority)?;
        state.serialize_field("src", &format!("{:#X}", self.src))?;
        state.serialize_field("dst", &format!("{:#X}", self.dst))?;
        state.serialize_field("pgn", &format!("{:#X}", self.pgn))?;
        state.serialize_field("data", &hex::encode_upper(&self.data))?;
        state.end()
    }
}

impl serde::Serialize for CanFrame {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("CanFrame", 9)?;
        state.serialize_field("timestamp", &self.timestamp)?;
        state.serialize_field("interface", &self.interface)?;
        state.serialize_field("canid", &format!("{:#X}", self.canid))?;
        state.serialize_field("dlc", &self.dlc)?;
        state.serialize_field("priority", &self.priority())?;
        state.serialize_field("src", &format!("{:#X}", self.src()))?;
        state.serialize_field("dst", &format!("{:#X}", self.dst()))?;
        state.serialize_field("pgn", &format!("{:#X}", self.pgn()))?;
        state.serialize_field("data", &hex::encode_upper(self.data()))?;
        state.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_canid_parsing() {
        let frame = CanFrame {
            canid: 0x0CAC1C13,
            ..Default::default()
        };
        assert_eq!(frame.pgn(), 0xAC00);
        assert_eq!(frame.src(), 0x13);
        assert_eq!(frame.dst(), 0x1C);

        let frame = CanFrame {
            canid: 0x18FF3F13,
            ..Default::default()
        };
        assert_eq!(frame.pgn(), 0xFF3F);
        assert_eq!(frame.src(), 0x13);
        assert_eq!(frame.dst(), 0xFF);

        let frame = CanFrame {
            canid: 0x18EF1CF5,
            ..Default::default()
        };
        assert_eq!(frame.priority(), 0x6);
        assert_eq!(frame.pgn(), 0xEF00);
        assert_eq!(frame.src(), 0xF5);
        assert_eq!(frame.dst(), 0x1C);

        let frame = CanFrame {
            canid: 0x09F8051C,
            ..Default::default()
        };
        assert_eq!(frame.priority(), 0x2);
        assert_eq!(frame.pgn(), 0x1F805); // has the data page bit set
        assert_eq!(frame.src(), 0x1C);
        assert_eq!(frame.dst(), 0xFF);

        let frame = CanFrame {
            canid: 0x18EEFF1C,
            ..Default::default()
        };
        assert_eq!(frame.priority(), 0x6);
        assert_eq!(frame.pgn(), 0xEE00);
        assert_eq!(frame.src(), 0x1C);
        assert_eq!(frame.dst(), 0xFF);
    }
}
