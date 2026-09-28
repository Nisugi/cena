//! How a `quit` went: moved out of `handle.rs` when the M7 and GUI lines met
//! there and took it past its cap (`plan/05` Rule 4.4: move code down).

/// How an orderly exit went (`plan/16` §5b.3).
///
/// Ports the three-way distinction Lich draws at
/// `reference/lich-5/lib/common/orderly_shutdown.rb:181-191`, where sending the
/// command, the reader stopping in time, and the stream actually reaching EOF
/// are three separate checks that can each fail on their own.
///
/// **All three still close the socket.** This reports what happened; it does
/// not decide whether to clean up.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Farewell {
    /// The command went out and the server closed the stream. A clean exit.
    Acknowledged,
    /// The command went out and the server never closed within the timeout.
    ///
    /// Lich raises `ServerExitTimeout` here. The session still ends -- the
    /// difference is that the game may not have registered the logout, so a
    /// character can be left in-world for the usual link-dead interval.
    TimedOut,
    /// The command could not be written at all: the transport was already
    /// gone.
    ///
    /// **Not a failure of the shutdown** -- there is nothing to say goodbye to.
    /// Distinguished from [`Self::TimedOut`] because a log that conflates them
    /// cannot tell a dead socket from an unresponsive server.
    Unsent,
}
