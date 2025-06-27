use crate::can::{CanFrame, CanMessage, Session};

#[repr(transparent)]
struct TpDt(CanFrame);

impl TpDt {
    #[inline]
    #[must_use]
    fn seq_id(&self) -> u8 {
        self.0.data()[0]
    }

    #[inline]
    #[must_use]
    fn data(&self) -> &[u8] {
        &self.0.data()[1..]
    }
}

#[repr(transparent)]
struct TpCmRts(CanFrame);

impl TpCmRts {
    #[inline]
    #[must_use]
    fn total_message_bytes(&self) -> u16 {
        let low_byte = self.0.data()[1] as u16;
        let high_byte = (self.0.data()[2] as u16) << 8;

        let result = high_byte | low_byte;
        debug_assert!(result > 8);
        debug_assert!(result < 1786);
        result
    }

    #[inline]
    #[must_use]
    fn total_message_packets(&self) -> u8 {
        self.0.data()[3]
    }

    /// Maximum number of packets the sender is willing to send together in a burst
    ///
    /// `0xFF` indicates the sender has no limit.
    #[inline]
    #[must_use]
    #[allow(unused)]
    fn max_number_packets(&self) -> u8 {
        self.0.data()[4]
    }

    /// The PGN of the message being sent
    #[inline]
    #[must_use]
    fn message_pgn(&self) -> u32 {
        let low_byte = self.0.data()[5] as u32;
        let mid_byte = (self.0.data()[6] as u32) << 8;
        let high_byte = (self.0.data()[7] as u32) << 16;

        high_byte | mid_byte | low_byte
    }
}

#[repr(transparent)]
struct TpCmCts(CanFrame);

impl TpCmCts {
    /// Number of packets the receiver is allowing the sender to send in one burst
    #[inline]
    #[must_use]
    fn number_of_packets(&self) -> u8 {
        // must not be larger than the TpCmRts.total_message_packets or TpCmRts.max_number_packets
        self.0.data()[1]
    }

    /// The next packet number the receiver is expecting
    #[inline]
    #[must_use]
    fn next_packet(&self) -> u8 {
        self.0.data()[2]
    }

    /// The PGN of the message being received
    #[inline]
    #[must_use]
    fn message_pgn(&self) -> u32 {
        let low_byte = self.0.data()[5] as u32;
        let mid_byte = (self.0.data()[6] as u32) << 8;
        let high_byte = (self.0.data()[7] as u32) << 16;

        high_byte | mid_byte | low_byte
    }
}

#[repr(transparent)]
struct TpCmEndOfMsgAck(CanFrame);

impl TpCmEndOfMsgAck {
    #[inline]
    #[must_use]
    #[allow(unused)]
    fn total_message_bytes(&self) -> u16 {
        let low_byte = self.0.data()[1] as u16;
        let high_byte = (self.0.data()[2] as u16) << 8;

        let result = high_byte | low_byte;
        debug_assert!(result > 8);
        debug_assert!(result < 1786);
        result
    }

    #[inline]
    #[must_use]
    #[allow(unused)]
    fn total_message_packets(&self) -> u8 {
        self.0.data()[3]
    }

    /// The PGN of the message being acknowledged
    #[inline]
    #[must_use]
    fn message_pgn(&self) -> u32 {
        let low_byte = self.0.data()[5] as u32;
        let mid_byte = (self.0.data()[6] as u32) << 8;
        let high_byte = (self.0.data()[7] as u32) << 16;

        high_byte | mid_byte | low_byte
    }
}

#[repr(transparent)]
struct TpCmConnAbort(CanFrame);

#[repr(u8)]
#[derive(Debug, PartialEq, Eq)]
enum AbortReason {
    Reserved,
    ExistingTransportSession = 1,
    SystemResources = 2,
    Timeout = 3,
    CtsDuringDt = 4,
    MaxRetryLimit = 5,
    UnexpectedDt = 6,
    BadSequenceNumber = 7,
    DuplicateSequenceNumber = 8,
    MessageTooLarge = 9,
    UnknownReason = 250,
    // TODO: 251-255 are supposed to be defined by ISO 11783-7, but I can't find them...
}

impl TpCmConnAbort {
    #[inline]
    #[must_use]
    fn abort_reason(&self) -> AbortReason {
        match self.0.data()[1] {
            0 | 10..=249 => AbortReason::Reserved,
            1 => AbortReason::ExistingTransportSession,
            2 => AbortReason::SystemResources,
            3 => AbortReason::Timeout,
            4 => AbortReason::CtsDuringDt,
            5 => AbortReason::MaxRetryLimit,
            6 => AbortReason::UnexpectedDt,
            7 => AbortReason::BadSequenceNumber,
            8 => AbortReason::DuplicateSequenceNumber,
            9 => AbortReason::MessageTooLarge,
            250..=255 => AbortReason::UnknownReason,
        }
    }

    /// The PGN of the message being aborted
    #[inline]
    #[must_use]
    fn message_pgn(&self) -> u32 {
        let low_byte = self.0.data()[5] as u32;
        let mid_byte = (self.0.data()[6] as u32) << 8;
        let high_byte = (self.0.data()[7] as u32) << 16;

        high_byte | mid_byte | low_byte
    }
}

#[repr(transparent)]
struct TpCmBam(CanFrame);

impl TpCmBam {
    #[inline]
    #[must_use]
    fn total_message_bytes(&self) -> u16 {
        let low_byte = self.0.data()[1] as u16;
        let high_byte = (self.0.data()[2] as u16) << 8;

        let result = high_byte | low_byte;
        debug_assert!(result > 8);
        debug_assert!(result < 1786);
        result
    }

    #[inline]
    #[must_use]
    fn total_message_packets(&self) -> u8 {
        self.0.data()[3]
    }

    /// The PGN of the message being broadcast
    #[inline]
    #[must_use]
    fn message_pgn(&self) -> u32 {
        let low_byte = self.0.data()[5] as u32;
        let mid_byte = (self.0.data()[6] as u32) << 8;
        let high_byte = (self.0.data()[7] as u32) << 16;

        high_byte | mid_byte | low_byte
    }
}

/// ISO 11783-3 Transport Protocol Session
///
/// Transport Protocol is specified in ISO 11783-3:5.10, and Extended Transport Protocol in ISO
/// 11783-3:5.11. The maximum TP message size is 255 packets of 7 bytes/packet, giving a total of
/// 1,785 bytes. ETP maximum message size is 2^24-1 packets of 7 bytes/packet, giving a total of
/// 117,440,505 bytes.
///
/// There are two kinds of TP sessions
///
/// 1. Broadcast - global broadcasts with no ECU-ECU p2p connection
///
///    BAM sessions are initiated by a TP.CM_BAM control flow message, followed by a series of
///    TP.DT data transfer messages, with no flow control in between.
///
/// 2. Point to Point - messages from one ECU to another, with control flow and connection
///    initiation
///
///    Point to point TP sessions are initiated by a TP.CM_RTS request to send, and if
///    acknowledged, is followed up by series of TP.DT messages in sent in bursts whose size is
///    defined by the periodic flow control messages from the recipient.
///
/// and two kinds of TP PGNS
///
/// 1. `0xEB00` - Data Transfer (TP.DT)
/// 2. `0xEC00` - Connection Management (TP.CM)
///
///    There are multiple kinds of TP.CM messages defined by the first byte of the message (the
///    Control Byte):
///
///    1. `0x10` - Request To Send (TP.CM_RTS)
///    2. `0x11` - Clear To Send (TP.CM_CTS)
///    3. `0x13` - End of Message Acknowledgement (TP.CM_EndofMsgACK)
///    4. `0xFF` - Connection Abort (TP.Conn_Abort)
///    5. `0x20` - Broadcast Announce Message (TP.CM_BAM)
///
///    Other control byte values are reserved.
///
/// # Which ISO-TP?
///
/// There are two distinct Transport Protocols in the ISO CAN world. There's the ISO TP and ETP
/// defined by ISO 11783-3, and there's the "ISO-TP" defined by ISO 15765-2. The two are close
/// enough to be easily confused.
///
/// Awkwardly, it's the ISO 15765-2 ISO-TP that the Linux kernel supports, which leads to confusion
/// in the Precision Ag world, where it's the ISO 11783-3 TP and ETP that matter.
///
/// # TODO
///
/// * Write tests around
///   * Conn_Abort
///   * Timeout
///   * RTS during an existing session
#[derive(Default)]
pub struct Iso11783TransportProtocolSession {
    /// The CanMessage being reconstructed from each of the CanFrames
    msg: Option<CanMessage>,
    /// Does this session need flow control, or is it a BAM session?
    needs_ack: bool,
    /// Current number of packets processed in this session
    current_packets: u8,
    /// Total number of packets expected to be processed
    expected_packets: u8,
    /// Total number of expected bytes. Use msg.data.len() for actual.
    expected_bytes: usize,
}

impl Session for Iso11783TransportProtocolSession {
    fn accepts_frame(frame: &CanFrame) -> bool {
        frame.pgn() == 0xEB00 || frame.pgn() == 0xEC00
    }

    fn handle_frame(&mut self, frame: CanFrame) -> eyre::Result<Option<CanMessage>> {
        if frame.pgn() == 0xEC00 {
            self.handle_control_message(frame)
        } else if frame.pgn() == 0xEB00 {
            self.handle_data_transfer(TpDt(frame))
        } else {
            unreachable!(
                "ISO 11783-3 Transport Protocol only uses 0xEC00 and 0xEB00 pgns. Got {:#X}",
                frame.pgn()
            );
        }
    }
}

/// Public API
impl Iso11783TransportProtocolSession {
    /// Create a new TP Session reconstruction object
    pub fn new() -> Self {
        Self::default()
    }
}

/// Private implementation
impl Iso11783TransportProtocolSession {
    fn handle_control_message(&mut self, frame: CanFrame) -> eyre::Result<Option<CanMessage>> {
        debug_assert!(frame.dlc == 8, "TP.CM messages must be 8 bytes");
        let control_byte = frame.data()[0];
        match control_byte {
            0x10 => self.handle_request_to_send(TpCmRts(frame)),
            0x11 => self.handle_clear_to_send(TpCmCts(frame)),
            0x13 => self.handle_end_of_message(TpCmEndOfMsgAck(frame)),
            0x20 => self.handle_broadcast_announce(TpCmBam(frame)),
            0xFF => self.handle_connection_abort(TpCmConnAbort(frame)),
            _ => unreachable!("TP.CM Control byte {control_byte:#X} is reserved"),
        }
    }

    fn handle_data_transfer(&mut self, frame: TpDt) -> eyre::Result<Option<CanMessage>> {
        let Some(msg) = self.msg.as_mut() else {
            eyre::bail!(
                "Unexpected TP.DT {:#X} -> {:#X} seq: {:#04X}/{:#04X} before TP.CM_RTS or TP.CM_BAM",
                frame.0.src(),
                frame.0.dst(),
                frame.seq_id(),
                self.expected_packets,
            );
        };
        self.current_packets += 1;
        msg.timestamp = frame.0.timestamp;

        // It's common for TP.DT frames to have 0xFF padding to fill out the full 7-bytes of data,
        // but we don't want to include that 0xFF padding in the reconstructed message.
        let bytes_so_far = msg.data.len();
        let bytes_remaining = self.expected_bytes - bytes_so_far;
        let bytes_remaining = usize::min(bytes_remaining, frame.data().len());
        msg.data.extend_from_slice(&frame.data()[..bytes_remaining]);

        tracing::trace!(
            "TP.DT     {:#X} -> {:#X} seq: {:#04X}/{:#04X} bytes: {}/{}",
            frame.0.src(),
            frame.0.dst(),
            frame.seq_id(),
            self.expected_packets,
            msg.data.len(),
            self.expected_bytes,
        );

        // If this is a broadcast BAM session, there's no ACK that finishes off the session. So we
        // have to yield the reconstructed message when we receive the final TP.DT frame.
        if !self.needs_ack && msg.data.len() == self.expected_bytes {
            Ok(self.msg.take())
        } else {
            Ok(None)
        }
    }

    fn handle_request_to_send(&mut self, frame: TpCmRts) -> eyre::Result<Option<CanMessage>> {
        tracing::debug!(
            "TP.CM_RTS {:#X} -> {:#X} packets: {}, bytes: {} pgn: {:#X}",
            frame.0.src(),
            frame.0.dst(),
            frame.total_message_packets(),
            frame.total_message_bytes(),
            frame.message_pgn()
        );
        self.handle_first_frame(
            true,
            frame.total_message_bytes() as usize,
            frame.total_message_packets(),
            frame.message_pgn(),
            frame.0,
        )?;
        Ok(None)
    }

    fn handle_broadcast_announce(&mut self, frame: TpCmBam) -> eyre::Result<Option<CanMessage>> {
        tracing::debug!(
            "TP.CM_BAM from {:#X} packets: {}, bytes: {} pgn: {:#X}",
            frame.0.src(),
            frame.total_message_packets(),
            frame.total_message_bytes(),
            frame.message_pgn()
        );
        self.handle_first_frame(
            false,
            frame.total_message_bytes() as usize,
            frame.total_message_packets(),
            frame.message_pgn(),
            frame.0,
        )?;
        Ok(None)
    }

    /// Handle both TP.CM_RTS and TP.CM_BAM the same
    fn handle_first_frame(
        &mut self,
        needs_ack: bool,
        bytes: usize,
        packets: u8,
        pgn: u32,
        frame: CanFrame,
    ) -> eyre::Result<()> {
        if self.msg.is_some() {
            tracing::error!(
                "Multiple TP.CM_RTS or TM.CM_BAM frames for the same session: {:#X} -> {:#X} @ {}",
                frame.dst(),
                frame.src(),
                frame.timestamp,
            );
        }
        // If this session has already been started, reset it if we get another TP.CM_RTS or TP.CM_BAM
        *self = Self::new();

        // Build the 29-bit canid as if it were a single-frame CAN message. Use the priority from
        // the first frame, but I've seen e.g., BAM sessions where the priority differs between the
        // TP.CM_BAM and TP.DT frames.
        //
        // | 3    | 1   | 1  | 8    | 8    | 8   |
        // | prio | EDP | DP | PDUF | PDUS | SRC |
        let mut canid = (frame.priority() as u32) << 26;
        canid |= pgn << 8;
        let pdu_format = (pgn & 0xFF00) >> 8;
        if pdu_format <= 0xEF {
            canid |= (frame.dst() as u32) << 8;
        }
        canid |= frame.src() as u32;

        let msg = CanMessage {
            src: frame.src(),
            dst: frame.dst(),
            priority: frame.priority(),
            timestamp: frame.timestamp,
            interface: frame.interface,
            canid,
            pgn,
            dlc: bytes,
            data: Vec::with_capacity(bytes),
        };
        self.msg = Some(msg);
        self.needs_ack = needs_ack;
        self.expected_packets = packets;
        self.expected_bytes = bytes;
        Ok(())
    }

    fn handle_clear_to_send(&mut self, frame: TpCmCts) -> eyre::Result<Option<CanMessage>> {
        // For the purpose of a TP parser, all we need is logging and error handling
        if self.msg.is_some() {
            tracing::trace!(
                "TP.CM_CTS {:#X} <- {:#X} seq: {:#04X} window: {} pgn: {:#X}",
                frame.0.dst(),
                frame.0.src(),
                frame.next_packet(),
                frame.number_of_packets(),
                frame.message_pgn()
            );
        } else {
            eyre::bail!(
                "Unexpected TP.CM_CTS {:#X} <- {:#X} seq: {:#04X} window: {} pgn: {:#X} before TP.CM_RTS or TP.CM_BAM",
                frame.0.dst(),
                frame.0.src(),
                frame.next_packet(),
                frame.number_of_packets(),
                frame.message_pgn()
            )
        }

        Ok(None)
    }

    fn handle_end_of_message(
        &mut self,
        frame: TpCmEndOfMsgAck,
    ) -> eyre::Result<Option<CanMessage>> {
        let Some(mut msg) = self.msg.take() else {
            eyre::bail!(
                "Unexpected TP.CM_ACK {:#X} <- {:#X} pgn {:#X} before TP.CM_RTS or TP.CM_BAM",
                frame.0.dst(),
                frame.0.src(),
                frame.message_pgn()
            );
        };
        tracing::debug!(
            "TP.CM_ACK {:#X} <- {:#X} bytes {} pgn {:#X}",
            frame.0.dst(),
            frame.0.src(),
            msg.data.len(),
            frame.message_pgn()
        );
        msg.timestamp = frame.0.timestamp;
        Ok(Some(msg))
    }

    fn handle_connection_abort(
        &mut self,
        frame: TpCmConnAbort,
    ) -> eyre::Result<Option<CanMessage>> {
        tracing::warn!(
            "TP.CM_ABRT {:#X} <- {:#X} reason {:?} pgn {:#X}",
            frame.0.dst(),
            frame.0.src(),
            frame.abort_reason(),
            frame.message_pgn()
        );
        *self = Self::default();
        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::can::{CandumpFormat, parse_candump};

    fn fixture_one_big_dt_chunk() -> impl Iterator<Item = CanFrame> {
        // TP.CM_CTS said "screw it, send everything"
        let candump = "\
            (1661789611.150752) can1 18EC1C2A#10900015FF00EF01 T \n\
                                                                 \n\
            (1661789611.153173) can1 1CEC2A1C#111501FFFF00EF01 R \n\
                                                                 \n\
            (1661789611.154815) can1 1CEB1C2A#0100112233445566 T \n\
            (1661789611.154824) can1 1CEB1C2A#02778899AABBCCDD T \n\
            (1661789611.154831) can1 1CEB1C2A#0300112233445566 T \n\
            (1661789611.154837) can1 1CEB1C2A#04778899AABBCCDD T \n\
            (1661789611.154844) can1 1CEB1C2A#0500112233445566 T \n\
            (1661789611.154851) can1 1CEB1C2A#06778899AABBCCDD T \n\
            (1661789611.154857) can1 1CEB1C2A#0700112233445566 T \n\
            (1661789611.154864) can1 1CEB1C2A#08778899AABBCCDD T \n\
            (1661789611.154871) can1 1CEB1C2A#0900112233445566 T \n\
            (1661789611.154878) can1 1CEB1C2A#0A778899AABBCCDD T \n\
            (1661789611.158875) can1 1CEB1C2A#0B00112233445566 T \n\
            (1661789611.158910) can1 1CEB1C2A#0C778899AABBCCDD T \n\
            (1661789611.158918) can1 1CEB1C2A#0D00112233445566 T \n\
            (1661789611.158927) can1 1CEB1C2A#0E778899AABBCCDD T \n\
            (1661789611.158936) can1 1CEB1C2A#0F00112233445566 T \n\
            (1661789611.158945) can1 1CEB1C2A#10778899AABBCCDD T \n\
            (1661789611.158961) can1 1CEB1C2A#1100112233445566 T \n\
            (1661789611.158969) can1 1CEB1C2A#12778899AABBCCDD T \n\
            (1661789611.158979) can1 1CEB1C2A#1300112233445566 T \n\
            (1661789611.158988) can1 1CEB1C2A#14778899AABBCCDD T \n\
            (1661789611.162934) can1 1CEB1C2A#1500112233FFFFFF T \n\
                                                                 \n\
            (1661789611.163251) can1 1CEC2A1C#13900015FF00EF01 R \n\
        ";
        parse_candump(candump)
    }

    fn fixture_multi_chunk() -> impl Iterator<Item = CanFrame> {
        let candump = "\
            (1665781494.217819) can1 18EC1C2A#10D8001FFF00EF00 \n\
                                                               \n\
            (1665781494.218976) can1 1CEC2A1C#110A01FFFF00EF00 \n\
                                                               \n\
            (1665781494.221950) can1 1CEB1C2A#0100112233445566 \n\
            (1665781494.222717) can1 1CEB1C2A#02778899AABBCCDD \n\
            (1665781494.223480) can1 1CEB1C2A#0300112233445566 \n\
            (1665781494.224304) can1 1CEB1C2A#04778899AABBCCDD \n\
            (1665781494.225153) can1 1CEB1C2A#0500112233445566 \n\
            (1665781494.226204) can1 1CEB1C2A#06778899AABBCCDD \n\
            (1665781494.227086) can1 1CEB1C2A#0700112233445566 \n\
            (1665781494.227949) can1 1CEB1C2A#08778899AABBCCDD \n\
            (1665781494.228994) can1 1CEB1C2A#0900112233445566 \n\
            (1665781494.229870) can1 1CEB1C2A#0A778899AABBCCDD \n\
                                                               \n\
            (1665781494.230484) can1 1CEC2A1C#110A0BFFFF00EF00 \n\
                                                               \n\
            (1665781494.234862) can1 1CEB1C2A#0B00112233445566 \n\
            (1665781494.235476) can1 1CEB1C2A#0C778899AABBCCDD \n\
            (1665781494.236538) can1 1CEB1C2A#0D00112233445566 \n\
            (1665781494.238741) can1 1CEB1C2A#0E778899AABBCCDD \n\
            (1665781494.239316) can1 1CEB1C2A#0F00112233445566 \n\
            (1665781494.240408) can1 1CEB1C2A#10778899AABBCCDD \n\
            (1665781494.240980) can1 1CEB1C2A#1100112233445566 \n\
            (1665781494.241552) can1 1CEB1C2A#12778899AABBCCDD \n\
            (1665781494.242674) can1 1CEB1C2A#1300112233445566 \n\
            (1665781494.243240) can1 1CEB1C2A#14778899AABBCCDD \n\
                                                               \n\
            (1665781494.244029) can1 1CEC2A1C#110A15FFFF00EF00 \n\
                                                               \n\
            (1665781494.247187) can1 1CEB1C2A#1500112233445566 \n\
            (1665781494.248155) can1 1CEB1C2A#16778899AABBCCDD \n\
            (1665781494.249013) can1 1CEB1C2A#1700112233445566 \n\
            (1665781494.250673) can1 1CEB1C2A#18778899AABBCCDD \n\
            (1665781494.251239) can1 1CEB1C2A#1900112233445566 \n\
            (1665781494.251810) can1 1CEB1C2A#1A778899AABBCCDD \n\
            (1665781494.252991) can1 1CEB1C2A#1B00112233445566 \n\
            (1665781494.253581) can1 1CEB1C2A#1C778899AABBCCDD \n\
            (1665781494.254156) can1 1CEB1C2A#1D00112233445566 \n\
            (1665781494.255261) can1 1CEB1C2A#1E778899AABBCCDD \n\
                                                               \n\
            (1665781494.256969) can1 1CEC2A1C#110A1FFFFF00EF00 \n\
                                                               \n\
            (1665781494.259475) can1 1CEB1C2A#1F001122334455FF \n\
                                                               \n\
            (1665781494.261703) can1 1CEC2A1C#13D8001FFF00EF00 \n\
        ";
        parse_candump(candump)
    }

    fn fixture_bam_dm1() -> impl Iterator<Item = CanFrame> {
        let candump = "\
            (1666812359.079961) can1 18ECFF1C#200E0002FFCAFE00 \n\
            (1666812359.131833) can1 14EBFF1C#0100FF7B1402030A \n\
            (1666812359.183336) can1 14EBFF1C#02FFF3020AF8F702 \n\
        ";
        parse_candump(candump)
    }

    #[test]
    fn test_parse_tp_dt() {
        let msg = "(1665782108.888314) can1 1CEB1C2A#021E1A8024052C69";
        let frame = CandumpFormat::CanUtilsFile.parse(msg).unwrap();
        let frame = TpDt(frame);

        assert_eq!(frame.seq_id(), 2);
        assert_eq!(frame.data()[0], 0x1E);
        assert_eq!(frame.data()[6], 0x69);
    }

    #[test]
    fn test_parse_tp_cm_rts() {
        let msg = "(1665782108.883474) can1 18EC1C2A#104D0130FF00EF01";
        let frame = CandumpFormat::CanUtilsFile.parse(msg).unwrap();
        let frame = TpCmRts(frame);

        assert_eq!(frame.total_message_bytes(), 0x014D);
        assert_eq!(frame.total_message_packets(), 0x30);
        assert_eq!(frame.max_number_packets(), 0xFF);
        assert_eq!(frame.message_pgn(), 0x1EF00);
    }

    #[test]
    fn test_parse_tp_cm_bam() {
        let msg = "(1666812359.079961) can1 18ECFF1C#200E0002FFCAFE00";
        let frame = CandumpFormat::CanUtilsFile.parse(msg).unwrap();
        let frame = TpCmBam(frame);

        assert_eq!(frame.total_message_bytes(), 0x0E);
        assert_eq!(frame.total_message_packets(), 0x02);
        assert_eq!(frame.message_pgn(), 0xFECA);
    }

    #[test]
    fn test_parse_tp_cm_cts() {
        let msg = "(1665782108.884614) can1 1CEC2A1C#110A01FFFF00EF01";
        let frame = CandumpFormat::CanUtilsFile.parse(msg).unwrap();
        let frame = TpCmCts(frame);

        assert_eq!(frame.number_of_packets(), 0x0A);
        assert_eq!(frame.next_packet(), 0x01);
        assert_eq!(frame.message_pgn(), 0x1EF00);
    }

    #[test]
    fn test_parse_tp_cm_ack() {
        let msg = "(1665782108.946324) can1 1CEC2A1C#134D0130FF00EF01";
        let frame = CandumpFormat::CanUtilsFile.parse(msg).unwrap();
        let frame = TpCmEndOfMsgAck(frame);

        assert_eq!(frame.total_message_bytes(), 0x014D);
        assert_eq!(frame.total_message_packets(), 0x30);
        assert_eq!(frame.message_pgn(), 0x1EF00);
    }

    #[test]
    fn test_parse_tp_cm_abort() {
        let msg = "(1665782108.946324) can1 1CEC2A1C#FF03FFFFFF00EF01";
        let frame = CandumpFormat::CanUtilsFile.parse(msg).unwrap();
        let frame = TpCmConnAbort(frame);

        assert_eq!(frame.abort_reason(), AbortReason::Timeout);
        assert_eq!(frame.message_pgn(), 0x1EF00);
    }

    #[test]
    fn test_bam_dm1() {
        let mut session = Iso11783TransportProtocolSession::new();
        let mut msg = None;
        for frame in fixture_bam_dm1() {
            msg = session.handle_frame(frame).unwrap();
        }
        let msg = msg.unwrap();

        assert_eq!(msg.timestamp, 1666812359.183336);
        assert_eq!(msg.interface, "can1");
        assert_eq!(msg.canid, 0x18FECA1C); // Prop B 0xFECA PGN without DP with priority 6
        assert_eq!(msg.priority, 6);
        assert_eq!(msg.pgn, 0xFECA);
        assert_eq!(msg.src, 0x1C);
        assert_eq!(msg.dst, 0xFF);
        assert_eq!(msg.dlc, 14);
        assert_eq!(msg.data[0], 0x00);
        assert_eq!(msg.data[6], 0x0A);
        assert_eq!(msg.data[7], 0xFF);
        assert_eq!(msg.data[13], 0x02);
    }

    #[test]
    fn test_one_big_dt_chunk() {
        let mut session = Iso11783TransportProtocolSession::new();
        let mut msg = None;
        for frame in fixture_one_big_dt_chunk() {
            msg = session.handle_frame(frame).unwrap();
        }
        let msg = msg.unwrap();

        assert_eq!(msg.timestamp, 1661789611.163251);
        assert_eq!(msg.interface, "can1");
        assert_eq!(msg.canid, 0x19EF1C2A); // Prop A2 0x1EF00 PGN with DP bit with priority 6
        assert_eq!(msg.priority, 6);
        assert_eq!(msg.pgn, 0x1EF00);
        assert_eq!(msg.src, 0x2A);
        assert_eq!(msg.dst, 0x1C);
        assert_eq!(msg.dlc, 144);
        assert_eq!(msg.data[0], 0x00);
        assert_eq!(msg.data[7], 0x77);
        assert_eq!(msg.data[143 - 7], 0xAA);
        assert_eq!(msg.data[143], 0x33);
    }

    #[test]
    fn test_multi_chunk() {
        let mut session = Iso11783TransportProtocolSession::new();
        let mut msg = None;
        for frame in fixture_multi_chunk() {
            msg = session.handle_frame(frame).unwrap();
        }
        let msg = msg.unwrap();

        assert_eq!(msg.timestamp, 1665781494.261703);
        assert_eq!(msg.interface, "can1");
        assert_eq!(msg.canid, 0x18EF1C2A); // Prop A 0xEF00 PGN without the DP bit with priority 6
        assert_eq!(msg.priority, 6);
        assert_eq!(msg.pgn, 0xEF00);
        assert_eq!(msg.src, 0x2A);
        assert_eq!(msg.dst, 0x1C);
        assert_eq!(msg.dlc, 216);
        assert_eq!(msg.data[0], 0x00);
        assert_eq!(msg.data[7], 0x77);
        assert_eq!(msg.data[215 - 7], 0xCC);
        assert_eq!(msg.data[215], 0x55);
    }

    #[test]
    fn test_canid_reconstruction() {
        // This is a Prop B PGN sent over a P2P TP session
        let candump = "\
            (1750963033.205621) can0 18EC801C#1042000A0A70FF00 \n\
            (1750963033.207887) can0 1CEC1C80#110A01FFFF70FF00 \n\
            (1750963033.208943) can0 14EB801C#01111E0000000000 \n\
            (1750963033.211735) can0 14EB801C#0200000000000000 \n\
            (1750963033.213493) can0 14EB801C#0300000000000000 \n\
            (1750963033.214642) can0 14EB801C#0400000000000000 \n\
            (1750963033.216342) can0 14EB801C#0500000000000000 \n\
            (1750963033.217422) can0 14EB801C#0600000000000000 \n\
            (1750963033.219104) can0 14EB801C#0700000000000000 \n\
            (1750963033.220252) can0 14EB801C#0800000000000000 \n\
            (1750963033.221376) can0 14EB801C#0900000000000000 \n\
            (1750963033.222869) can0 14EB801C#0A000000FFFFFFFF \n\
            (1750963033.223740) can0 1CEC1C80#1342000AFF70FF00 \n\
        ";
        let mut msg = None;
        let mut session = Iso11783TransportProtocolSession::new();
        for frame in parse_candump(candump) {
            msg = session.handle_frame(frame).unwrap();
        }
        let msg = msg.unwrap();

        assert_eq!(msg.timestamp, 1750963033.22374);
        assert_eq!(msg.pgn, 0x00FF70);
        assert_eq!(msg.canid, 0x18FF701C);
    }
}
