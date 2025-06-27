use std::collections::HashMap;

use crate::can::{CanFrame, CanMessage, FastPacketSession, Iso11783TransportProtocolSession};

/// A transport layer session
///
/// A [Session] is all about adapting possibly multiple [CanFrame]s into one [CanMessage]. That is,
/// it's about reconstructing larger messages that have been broken across multiple frames.
pub trait Session {
    /// Determine if the given [CanFrame] has this [Session] type
    fn accepts_frame(frame: &CanFrame) -> bool;

    /// Get the session ID for the given [CanFrame], assuming that the frame is of this [Session]s
    /// type.
    ///
    /// This ID, specifies which particular instance of this [Session] type the given frame belongs
    /// to.
    fn session_id(frame: &CanFrame) -> u32 {
        let src = frame.src() as u32;
        let dst = frame.dst() as u32;

        // It's okay to have two sessions of the same type concurrently sending ecu1->ecu2 and
        // ecu2->ecu1
        //
        // BUT: In at least the case of TP, there are messages back and forth between both ECUs, so
        // you need to understand the PGN and control bytes to understand which session to
        // associate the frames with.
        (src << 8) | dst
    }

    /// Handle a new frame that's associated with this session
    ///
    /// [Session]s require in-order frames, and will happily explode in your face if given
    /// out-of-order messages.
    fn handle_frame(&mut self, frame: CanFrame) -> eyre::Result<Option<CanMessage>>;
}

/// Pass through [CanFrame]s unchanged into [CanMessage]s
///
/// Most [CanFrame]s are actually complete [CanMessage]s, and don't need to be reconstructed.
pub struct IdentitySession;

impl Session for IdentitySession {
    fn accepts_frame(_: &CanFrame) -> bool {
        true
    }

    fn handle_frame(&mut self, frame: CanFrame) -> eyre::Result<Option<CanMessage>> {
        Ok(Some(frame.into()))
    }
}

/// Reconstruct known transport layer protocols into [CanMessage]s
///
/// Unknown [CanFrame]s are passed through with the assumption they don't need to be reconstructed.
pub fn reconstruct_transport_sessions<I: Iterator<Item = CanFrame>>(
    frames: I,
) -> SessionManager<I> {
    SessionManager {
        frames,
        identity: IdentitySession,
        fast_packet: HashMap::new(),
        transport_protocol: HashMap::new(),
    }
}

pub struct SessionManager<I: Iterator<Item = CanFrame>> {
    frames: I,

    identity: IdentitySession,
    fast_packet: HashMap<u32, FastPacketSession>,
    transport_protocol: HashMap<u32, Iso11783TransportProtocolSession>,
}

// TODO: Build tracing spans for each session that get entered before calling session.handle_frame
// for each session.
impl<I: Iterator<Item = CanFrame>> Iterator for SessionManager<I> {
    // TODO: Maybe if this fails to reconstruct a session, still pass the individual frames
    // through?
    type Item = eyre::Result<CanMessage>;

    fn next(&mut self) -> Option<Self::Item> {
        let frame = self.frames.next()?;

        if FastPacketSession::accepts_frame(&frame) {
            let session_id = FastPacketSession::session_id(&frame);
            let mut session = self.fast_packet.remove(&session_id).unwrap_or_default();

            match session.handle_frame(frame) {
                Err(e) => {
                    return Some(Err(
                        e.wrap_err("Failed to handle FP message; aborting session")
                    ));
                }
                Ok(Some(msg)) => return Some(Ok(msg)),
                Ok(None) => {
                    self.fast_packet.insert(session_id, session);
                    // Spooky recursion to keep handling the next frame until there's any finished
                    // session
                    return self.next();
                }
            }
        } else if Iso11783TransportProtocolSession::accepts_frame(&frame) {
            let session_id = Iso11783TransportProtocolSession::session_id(&frame);
            let mut session = self
                .transport_protocol
                .remove(&session_id)
                .unwrap_or_default();

            match session.handle_frame(frame) {
                Err(e) => {
                    // Don't insert the session back into the map
                    return Some(Err(e));
                }
                Ok(Some(msg)) => return Some(Ok(msg)),
                Ok(None) => {
                    self.transport_protocol.insert(session_id, session);
                    // Spooky recursion to keep handling the next frame until there's any finished
                    // session, including identity sessions, which results in not much recursion,
                    // unless you have malicious data.
                    return self.next();
                }
            }
        } else if IdentitySession::accepts_frame(&frame) {
            let msg = unsafe {
                self.identity
                    .handle_frame(frame)
                    // Guaranteed to be safe because the IdentitySession is always successful
                    .unwrap_unchecked()
                    .unwrap_unchecked()
            };
            return Some(Ok(msg));
        }

        None
    }
}

impl<I: Iterator<Item = CanFrame>> Drop for SessionManager<I> {
    fn drop(&mut self) {
        if !self.fast_packet.is_empty() {
            let in_flight_sessions: Vec<_> = self.fast_packet.keys().collect();
            tracing::warn!(
                "Reconstruction ended with still-active FP sessions: {in_flight_sessions:X?}"
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::can::parse_candump;

    #[test]
    fn test_abort_then_full_tp_session() {
        let candump = "\
            (1750963033.251412) can0 18EC2A1C#101600040400EF00	// TP.CM_RTS          \n\
            (1750963033.270725) can0 18EC1C2A#FF01FFFFFF00EF00	// TP.Conn_Abort      \n\
            (1750963079.757877) can0 18EC2A1C#101600040400EF00	// TP.CM_RTS          \n\
            (1750963079.775206) can0 18EC1C2A#110401FFFF00EF00	// TP.CM_CTS          \n\
            (1750963079.778342) can0 14EB2A1C#0111111111111111	// TP.DT              \n\
            (1750963079.779468) can0 14EB2A1C#0222222222222222	// TP.DT              \n\
            (1750963079.780613) can0 14EB2A1C#0333333333333333	// TP.DT              \n\
            (1750963079.781778) can0 14EB2A1C#0444FFFFFFFFFFFF	// TP.DT              \n\
            (1750963079.795905) can0 18EC1C2A#13160004FF00EF00	// TP.CM_EndofMsgACK  \n\
        ";
        let frames = parse_candump(candump);
        let msgs = reconstruct_transport_sessions(frames);
        let msgs: Vec<_> = msgs.collect();
        assert_eq!(msgs.len(), 1);
    }

    #[test]
    fn test_concurrent_sessions_between_the_same_ecus() {
        let candump = "\
            (1750992427.225496) can0 18ECF9A4#101F0005FFDAFE00	// TP.CM_RTS (A4 -> F9) \n\
            (1750992427.243729) can0 1CECA4F9#110501FFFFDAFE00	// TP.CM_CTS            \n\
            (1750992427.253501) can0 1CECA4F9#103B0009FFC5FD00	// TP.CM_RTS (F9 -> A4) \n\
            (1750992427.261216) can0 1CEBF9A4#0111111111111111	// TP.DT                \n\
            (1750992427.261791) can0 1CEBF9A4#0222222222222222	// TP.DT                \n\
            (1750992427.262356) can0 1CEBF9A4#0333333333333333	// TP.DT                \n\
            (1750992427.262911) can0 1CEBF9A4#0444444444444444	// TP.DT                \n\
            (1750992427.263480) can0 1CEBF9A4#05555555FFFFFFFF	// TP.DT                \n\
            (1750992427.266323) can0 1CECA4F9#131F0005FFDAFE00	// TP.CM_EndofMsgACK    \n\
            (1750992427.268593) can0 18ECF9A4#110901FFFFC5FD00	// TP.CM_CTS            \n\
            (1750992427.271783) can0 1CEBA4F9#0111111111111111	// TP.DT                \n\
            (1750992427.271819) can0 1CEBA4F9#0222222222222222	// TP.DT                \n\
            (1750992427.274755) can0 1CEBA4F9#0333333333333333	// TP.DT                \n\
            (1750992427.275845) can0 1CEBA4F9#0444444444444444	// TP.DT                \n\
            (1750992427.276926) can0 1CEBA4F9#0555555555555555	// TP.DT                \n\
            (1750992427.278029) can0 1CEBA4F9#0666666666666666	// TP.DT                \n\
            (1750992427.279146) can0 1CEBA4F9#0777777777777777	// TP.DT                \n\
            (1750992427.280212) can0 1CEBA4F9#0888888888888888	// TP.DT                \n\
            (1750992427.281361) can0 1CEBA4F9#09999999FFFFFFFF	// TP.DT                \n\
            (1750992427.295025) can0 18ECF9A4#133B0009FFC5FD00	// TP.CM_EndofMsgACK    \n\
        ";
        let frames = parse_candump(candump);
        let msgs = reconstruct_transport_sessions(frames);
        let msgs: Vec<_> = msgs.collect();
        assert_eq!(msgs.len(), 2);
    }
}
