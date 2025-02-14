use crate::can::{CanFrame, CanMessage};

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

        let srcdst: u32 = (src << 8) | dst;
        let dstsrc: u32 = (dst << 8) | src;

        // Take the min, so that ecu1 -> ecu2 and ecu1 <- ecu2 both map to the same session ID.
        u32::min(srcdst, dstsrc)
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
