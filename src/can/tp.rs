use crate::can::{CanFrame, CanMessage, Session};

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
/// * Conn_Aborts should be logged
/// * Find a nice way of handling BAM; either handle it separately, or find an elegant way to mix,
///   but don't fill in a ton of special cases
/// * Write tests around
///   * BAM
///   * Nominal DT
///   * Conn_Abort
///   * Timeout
///   * RTS during an existing session
#[derive(Default)]
pub struct Iso11783TransportProtocolSession {}

// TODO: Impl Debug for these newtypes?
// TODO: Impl for parsing fields from each of these newtypes
#[repr(transparent)]
struct TpDt(CanFrame);

#[repr(transparent)]
struct TpCmRts(CanFrame);

#[repr(transparent)]
struct TpCmCts(CanFrame);

#[repr(transparent)]
struct TpCmEndOfMsgAck(CanFrame);

#[repr(transparent)]
struct TpCmConnAbort(CanFrame);

#[repr(transparent)]
struct TpCmBam(CanFrame);

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
                "ISO 11783-3 Transport Protocol only uses 0xEC00 and 0xEB00 pgns. Got {:#x}",
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
            _ => unreachable!("TP.CM Control byte {control_byte:#x} is reserved"),
        }
    }

    fn handle_data_transfer(&mut self, frame: TpDt) -> eyre::Result<Option<CanMessage>> {
        Ok(None)
    }

    fn handle_request_to_send(&mut self, frame: TpCmRts) -> eyre::Result<Option<CanMessage>> {
        Ok(None)
    }

    fn handle_broadcast_announce(&mut self, frame: TpCmBam) -> eyre::Result<Option<CanMessage>> {
        Ok(None)
    }

    fn handle_clear_to_send(&mut self, frame: TpCmCts) -> eyre::Result<Option<CanMessage>> {
        Ok(None)
    }

    fn handle_end_of_message(
        &mut self,
        frame: TpCmEndOfMsgAck,
    ) -> eyre::Result<Option<CanMessage>> {
        Ok(None)
    }

    fn handle_connection_abort(
        &mut self,
        frame: TpCmConnAbort,
    ) -> eyre::Result<Option<CanMessage>> {
        Ok(None)
    }
}
