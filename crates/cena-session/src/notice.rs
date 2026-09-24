//! Hydra speaking to the player: the port of `Lich::Messaging`
//! (`lib/messaging.rb`), and why it is not a port of its mechanism.
//!
//! # What Lich has
//!
//! Four things, in 148 lines: a message with a **kind** that picks its colour
//! (`msg`, `:50-122`), a **mono** block for tables (`mono`, `:138`), a
//! clickable **command link** (`make_cmd_link`, `:132`) and a push into a
//! named **stream window** (`stream_window`, `:21`). Every script that tells
//! the player anything goes through it -- `route2.lic` prints its table with
//! `respond`, and half of `dr-scripts` reports failure with
//! `Lich::Messaging.msg('bold', ...)`.
//!
//! # What Hydra does not copy
//!
//! Lich writes **XML strings** -- `<preset id='whisper'>`, `<output
//! class="mono"/>`, `<pushBold/>` -- and branches on which frontend is
//! listening, because it sits in front of someone else's client and the only
//! way to reach the player is to forge game markup. Hydra is one binary
//! (`CLAUDE.md`, settled): emitting markup only to parse it again is a round
//! trip with a forgery in the middle, and it would put text the game never
//! sent into a stream whose whole value is that the game sent it.
//!
//! So a notice is **typed**, and travels as an [`Event`](crate::Event) beside
//! the frames rather than among them. The terminal prints it; a web frontend
//! draws a mono block in a fixed-width box; neither parses anything, and
//! neither can mistake Hydra's voice for the game's.
//!
//! > **CORRECTED 2026-09-21 (author: *"That could be part of Messaging!"*).**
//! > `plan/20` §1 looked at `messaging.rb`, saw that it parses no speech, and
//! > filed it as "only emits markup, belongs to `cena-ui`" -- right about
//! > speech, and wrong to stop there. The emitting half is a feature every
//! > behavior needs: a route table, a trip that ends with a sword still
//! > stored, a crossing that stops and says why. It was being treated as three
//! > future frontend problems, and it is one thing.
//!
//! # What is built, and what is not
//!
//! The kind and the two bodies, because each has a user today. **Not** the
//! command link or the target window: nothing here would send one yet, and a
//! field with no writer is the option-with-one-value `plan/05` section -1
//! forbids. They are named here so they are found when their first user
//! arrives.

/// What a notice is about, which is how a frontend colours it.
///
/// Lich's `msg` takes some twenty spellings -- `"error"`, `"yellow"`,
/// `"bold"`, `"monster"` are one colour -- that fold to four presets plus
/// debug (`messaging.rb:61-73`). These are the four meanings, without the
/// colour names: what colour an error is belongs to whoever draws it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NoticeKind {
    /// Something went wrong, and the player should know. Lich's `bold`.
    Error,
    /// Something is off, and nothing has failed yet. Lich's `thought`.
    Warn,
    /// An answer to something the player asked. Lich's `whisper`.
    Info,
    /// For whoever is chasing a fault. Lich drops these unless debug
    /// messaging is on (`messaging.rb:128`); here that is the frontend's
    /// choice, since a log may want what a screen does not.
    Debug,
}

/// What a notice says.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Body {
    /// Prose, a line at a time. May be re-wrapped by whoever draws it.
    Lines(Vec<String>),
    /// Lines whose **columns matter** -- a table. Drawn fixed-width and never
    /// re-wrapped. Lich's `mono`.
    Mono(Vec<String>),
}

/// One thing Hydra says to the player.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Notice {
    /// What it is about, which is how a frontend colours it (and whether it
    /// shows a `Debug` one at all).
    pub kind: NoticeKind,
    /// The words: prose to wrap, or a fixed-width table.
    pub body: Body,
}

impl Notice {
    /// One line of prose.
    #[must_use]
    pub fn line(kind: NoticeKind, text: impl Into<String>) -> Notice {
        Notice {
            kind,
            body: Body::Lines(vec![text.into()]),
        }
    }

    /// A table, as the lines that make it.
    #[must_use]
    pub fn table(kind: NoticeKind, lines: Vec<String>) -> Notice {
        Notice {
            kind,
            body: Body::Mono(lines),
        }
    }

    /// The lines, whichever body holds them: what a plain terminal prints.
    #[must_use]
    pub fn lines(&self) -> &[String] {
        match &self.body {
            Body::Lines(lines) | Body::Mono(lines) => lines,
        }
    }
}
