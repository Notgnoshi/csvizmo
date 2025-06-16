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
