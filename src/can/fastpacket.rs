use crate::can::{CanFrame, CanMessage, Session};

/// NMEA 2000 Fast Packet [Session]
///
/// Fast Packet messages are burst sent with no kind of ACK/NACK.
///
/// Fast Packet has two frame types:
///
/// # First Frame
///
/// | Byte 1 LS Nibble | Byte 1 MS Nibble | Bytes 2    | Bytes 3 .. 8 |
/// |------------------|------------------|------------|--------------|
/// | frame ctr        | group id         | num frames | data         |
///
/// # Following Frames
///
/// | Byte 1 LS Nibble | Byte 1 MS Nibble | Bytes 2..8 |
/// |------------------|------------------|------------|
/// | frame ctr        | group id         | data       |
#[derive(Default)]
pub struct FastPacketSession {
    message: Option<CanMessage>,
    session_data_length: usize,
    session_group_id: u8,
    current_frame_counter: u8,
    span: Option<tracing::span::EnteredSpan>,
}

/// Private impl just for Fast Packet
impl CanFrame {
    /// The index of this frame within this session
    #[inline]
    #[must_use]
    fn frame_counter(&self) -> u8 {
        self.data()[0] & 0x0F
    }

    /// Identifies a group of frames that belong together in the fast packet session
    #[inline]
    #[must_use]
    fn group_id(&self) -> u8 {
        self.data()[0] >> 4
    }

    #[inline]
    #[must_use]
    fn is_first_frame(&self) -> bool {
        self.frame_counter() == 0
    }

    #[inline]
    #[must_use]
    fn session_data_length(&self) -> usize {
        debug_assert!(self.is_first_frame());
        self.data()[1] as usize
    }

    #[inline]
    #[must_use]
    fn session_data(&self) -> &[u8] {
        if self.is_first_frame() {
            &self.data()[2..]
        } else {
            &self.data()[1..]
        }
    }
}

impl Session for FastPacketSession {
    fn accepts_frame(frame: &CanFrame) -> bool {
        // GNSS Position Data is the only Fast Packet message type I care about for now
        frame.pgn() == 0x1F805
    }

    fn handle_frame(&mut self, frame: CanFrame) -> eyre::Result<Option<CanMessage>> {
        if self.message.is_none() {
            self.handle_first_frame(frame)?;
        } else {
            self.handle_following_frame(frame)?;
        }

        if self.is_session_finished() {
            let msg = unsafe { self.message.take().unwrap_unchecked() };
            tracing::debug!(
                "Finished FP session. seq: {:#X} len: {}",
                self.session_group_id,
                msg.data.len()
            );
            Ok(Some(msg))
        } else {
            Ok(None)
        }
    }
}

impl FastPacketSession {
    pub fn new() -> Self {
        Self::default()
    }

    fn is_session_finished(&self) -> bool {
        if let Some(msg) = &self.message {
            msg.data.len() == self.session_data_length
        } else {
            false
        }
    }

    fn handle_first_frame(&mut self, frame: CanFrame) -> eyre::Result<()> {
        // TODO: Will spans be useful?
        let span = tracing::debug_span!("FP", seq_id = frame.group_id()).entered();
        self.span = Some(span);
        self.session_data_length = frame.session_data_length();
        self.session_group_id = frame.group_id();
        self.current_frame_counter = frame.frame_counter();
        let data: Vec<u8> = frame.session_data().into();
        tracing::debug!(
            "Start FP session.  ctr: {:#X} seq: {:#X} len: {}/{}",
            self.current_frame_counter,
            self.session_group_id,
            data.len(),
            self.session_data_length,
        );
        let mut msg: CanMessage = frame.into();
        msg.dlc = self.session_data_length;
        msg.data = data;
        self.message = Some(msg);

        Ok(())
    }

    fn handle_following_frame(&mut self, frame: CanFrame) -> eyre::Result<()> {
        let ctr = frame.frame_counter();
        let seq = self.session_group_id;
        let exp = self.current_frame_counter + 1;
        if ctr != exp {
            eyre::bail!(
                "Received FP frame out of order: ctr {ctr:#X} (expected {exp:#X}) for seq {seq:#X}",
            );
        }
        self.current_frame_counter = exp;
        let msg = unsafe { self.message.as_mut().unwrap_unchecked() };
        msg.data.extend_from_slice(frame.session_data());
        tracing::trace!(
            "Received FP frame. ctr: {ctr:#X} seq: {seq:#X} len: {}/{}",
            msg.data.len(),
            self.session_data_length,
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fast_packet_fixture() -> ([CanFrame; 4], CanMessage) {
        let frames = [
            CanFrame::new(
                0.0,
                "can0".to_string(),
                0x1F805FE,
                8,
                [0xE0, 0x1B, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06],
            ),
            CanFrame::new(
                0.0,
                "can0".to_string(),
                0x1F805FE,
                8,
                [0xE1, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D],
            ),
            CanFrame::new(
                0.0,
                "can0".to_string(),
                0x1F805FE,
                8,
                [0xE2, 0x0E, 0x0F, 0x10, 0x11, 0x12, 0x13, 0x14],
            ),
            CanFrame::new(
                0.0,
                "can0".to_string(),
                0x1F805FE,
                8,
                [0xE3, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B],
            ),
        ];
        let msg = CanMessage {
            timestamp: 0.0,
            interface: "can0".to_string(),
            canid: 0x1F805FE,
            priority: 0,
            pgn: 0x1F805,
            src: 0xFE,
            dst: 0xFF,
            dlc: 0x1B,
            data: vec![
                0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0A, 0x0B, 0x0C, 0x0D, 0x0E,
                0x0F, 0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1A, 0x1B,
            ],
        };
        (frames, msg)
    }

    #[test]
    fn test_fast_packet() {
        let (frames, expected) = fast_packet_fixture();

        let mut session = FastPacketSession::new();
        let result = session.handle_frame(frames[0].clone()).unwrap();
        assert_eq!(result, None);
        let result = session.handle_frame(frames[1].clone()).unwrap();
        assert_eq!(result, None);
        let result = session.handle_frame(frames[2].clone()).unwrap();
        assert_eq!(result, None);
        let result = session.handle_frame(frames[3].clone()).unwrap();
        assert_eq!(result, Some(expected));
    }
}
