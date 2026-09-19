//! cena-platform
//!
//! The bottom layer: the pipe a session's bytes come through, and the log of
//! what went through it. `plan/12:74` gives this crate "storage, paths,
//! logging, config primitives (deps: none)" -- none *intra-workspace*, which
//! still holds: nothing here knows what a `Frame` is.
//!
//! Read [`bytes`] for why the transport is a trait and why it yields bytes
//! rather than lines or frames, [`live`] for the three spike findings that
//! port verbatim (and for why nothing here may be run against the live game),
//! [`eaccess`] for the login handshake and why it lives at this layer,
//! [`replay`] for criterion 7's no-network source, and [`record`] for what a
//! recording contains and why it is bytes rather than frames.
//!
//! # [`AnsweringSource`] is TEST SCAFFOLDING that ships
//!
//! It is a stand-in for the game -- it answers each written command and never
//! hangs up -- and nothing in the shipped binary constructs one. It lives here
//! rather than in a `tests/` directory because **three crates' tests need it**
//! (`cena-session`, `cena-behavior`, and the end-to-end tests), and a
//! `tests/` module is not reachable across a crate boundary. Duplicating it
//! three times is the alternative and is worse: three copies drift, and the
//! one a failing test used stops being the one a reader fixes.
//!
//! It is deliberately NOT hidden behind a cargo feature. A feature-gated test
//! helper is a second build configuration that CI must remember to enable, and
//! `plan/05` §0 -- a rule that is not enforced is a wish -- applies to
//! coverage too: a helper nobody compiles by default is a helper whose own
//! bugs are invisible.
//!
//! # No `Clock` trait, deliberately
//!
//! An earlier design had one, with a real and a virtual implementor, to make
//! the replay deterministic. It is not needed and would be the abstraction
//! Rule -1 (`plan/05` §-1) forbids: `tokio::time::pause()` already makes
//! `tokio::time::Instant` and every `sleep`/`timeout` in the tree virtual, for
//! the real code under test rather than for a stand-in. The session therefore
//! uses `tokio::time` directly and a test drives it with `pause()`/`advance()`.

pub mod answering;
pub mod bytes;
pub mod eaccess;
pub mod live;
pub mod record;
pub mod replay;
pub mod sink;

pub use answering::{AnsweringSource, TranscriptHandle};
pub use bytes::ByteSource;
pub use eaccess::{Credentials, EaccessError, LaunchPayload, authenticate, connect_game};
pub use live::LiveSource;
pub use record::{MAX_RECORDED_BYTES, RecordedEvent, Recorder};
pub use replay::ReplaySource;
pub use sink::{DEFAULT_LOG_DIR, Redactions, SessionSink, date_dir, file_stamp, log_dir};
