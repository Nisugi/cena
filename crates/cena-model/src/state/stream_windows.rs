//! What a stream's text does when its window is **closed**: the wire's own
//! rule, read off `<streamWindow ifClosed= styleIfClosed=>`.
//!
//! # The question this answers
//!
//! > **AUTHOR, 2026-09-21:** *"When a stream comes in and it has an ifClosed=
//! > attribute, that attribute tells us what to do with that stream if it's
//! > stream window is closed."*
//!
//! It came up from a real duplicate. The author pasted this, and it is the
//! whole problem in four lines:
//!
//! ```text
//! <pushStream id="speech"/><preset id='speech'>You <a ...>say</a></preset>, "Yep."
//! <popStream/>
//! <preset id='speech'>You <a ...>say</a></preset>, "Yep."
//! <prompt time="1790052928">&gt;</prompt>
//! ```
//!
//! The same sentence twice: once inside the `speech` stream, once in the main
//! window. A renderer that shows both -- which Despana does -- prints
//! everything the character says twice.
//!
//! **That is not a defect in the game.** `speech` is declared `ifClosed=''`,
//! and the protocol wiki (`reference/wiki_clean/Wrayth protocol.txt:78`) says
//! what that means: *"The stream is a duplicate -- the server also sends the
//! same line to main, so the closed window's copy drops harmlessly"*. The
//! duplicate is the wire keeping a promise to a client that has a speech
//! window open. A client without one is supposed to drop the copy.
//!
//! # The four behaviours, MEASURED
//!
//! The wiki names four. All four are here in real traffic -- census over the
//! **208** `.xml` logs in `E:\Gemstone\dev\lich-5\logs\GSIV-Nisugi`,
//! **157,220** `<streamWindow>` tags:
//!
//! | behaviour | declaration | streams seen |
//! |---|---|---|
//! | [`Closed::Drop`] | `ifClosed=''` | `room` 77,411 · `society` 1,087 · **`speech` 201** · `inv` 162 · `Spells` 86 · `announcements` · `bounty` · `loot` · `reserve` · `charprofile` |
//! | [`Closed::Styled`] | `styleIfClosed='x'`, no `ifClosed` | `thoughts` → *thought* 86 · `familiar` → *watching* 86 |
//! | [`Closed::Main`] | neither | `main` 77,497 · `death` 86 · `logons` 86 |
//! | [`Closed::Route`] | `ifClosed='<window>'` | **none** -- see below |
//!
//! **`Route` is UNVERIFIED.** The wiki cites `voln → thoughts → main-as-thought`
//! and this character never declared one, so it is implemented from the wiki
//! and labelled rather than claimed. It is also the only behaviour that
//! chains, which is why [`Windows::route`] is written as a loop with a
//! visit-set rather than a `match`.
//!
//! **`ambients` inverts the usual shape** -- `ifClosed` absent,
//! `styleIfClosed` **present and empty** -- which the wiki's table does not
//! name. Read here as [`Closed::Main`]: an empty style is no style, and
//! falling through unstyled is what the `Unspecified` row does anyway.
//!
//! # Why `ifClosed=''` and a missing `ifClosed` cannot be one case
//!
//! They mean opposite things -- drop the text, versus show it in main -- so
//! this module lives or dies on telling them apart. `Attrs` is
//! `Vec<(String, String)>` (`frame.rs:143`), so an empty value survives as
//! `Some("")` and an absent one is `None`. VERIFIED against the parser before
//! this file was written: `speech` reads `Some("")`, `logons` reads `None`.
//!
//! Collapsing them -- `attrs.get("ifClosed").unwrap_or_default()` -- would make
//! every `Unspecified` stream drop its text, losing `death` and `logons`
//! entirely. That is the `plan/12` §5.2 mistake in miniature: absent is not
//! empty.
//!
//! # This decides routing; it does not decide windows
//!
//! Whether a window is *open* is a frontend fact, so it is a parameter rather
//! than state here. `streams.rs` keeps the same line: a map of buffers and a
//! routing rule, with layout left to `cena-ui`. This module adds the second
//! half of that rule -- the part that needs to know what the window would have
//! been for.

use std::collections::{BTreeMap, BTreeSet};

use cena_protocol::frame::Attrs;

/// What happens to a stream's text when its window is closed.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Closed {
    /// `ifClosed=''`: the line is a **duplicate**. The server sends the same
    /// text to main as well, so this copy is dropped.
    ///
    /// This is `speech`, and it is why the author saw every spoken line twice.
    Drop,
    /// `ifClosed='<window>'`: send it to that window instead, which may itself
    /// be closed -- so this chains.
    ///
    /// **UNVERIFIED against live traffic.** The wiki's example is
    /// `voln → thoughts`; the 208-file census found no `Route` declaration.
    Route(String),
    /// `styleIfClosed='x'` with no `ifClosed`: falls through to main, wrapped
    /// in that style. The classic inline-thoughts look.
    Styled(String),
    /// Neither attribute: falls through to main, unstyled.
    Main,
}

impl Closed {
    /// Read the behaviour off a `<streamWindow>`'s attributes.
    ///
    /// Order matters: `ifClosed` is checked before `styleIfClosed`, because
    /// the wiki's `Exclusive` row is *"`ifClosed` absent, `styleIfClosed`
    /// set"* -- a stream carrying both is routed, not styled.
    #[must_use]
    pub fn read(attrs: &Attrs) -> Self {
        let value = |name: &str| {
            attrs
                .iter()
                .find(|(key, _)| key == name)
                .map(|(_, value)| value.as_str())
        };
        match value("ifClosed") {
            // Present and empty: a duplicate the server also sent to main.
            Some("") => Self::Drop,
            Some(window) => Self::Route(window.to_owned()),
            // An EMPTY `styleIfClosed` is no style: `ambients` declares it that
            // way, and an unstyled fall-through is `Main`.
            None => match value("styleIfClosed") {
                Some("") | None => Self::Main,
                Some(style) => Self::Styled(style.to_owned()),
            },
        }
    }
}

/// Where a line should actually go, once the closed-window rule is applied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Destination {
    /// Its own window, which is open.
    Window(String),
    /// The main window, unstyled.
    Main,
    /// The main window, wrapped in this style.
    MainStyled(String),
    /// Nowhere: the server already sent this text to main.
    Dropped,
}

/// What each stream declared it does when closed.
///
/// A map rather than a fixed table, because the ids are the game's: the census
/// found 16 declared against 8 ever pushed to, and `plan/15` warns that a
/// frontend must not assume either set is closed. An
/// undeclared stream is not an error -- see [`Self::route`].
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Windows {
    declared: BTreeMap<String, Closed>,
}

impl Windows {
    /// Record a `<streamWindow>` declaration.
    ///
    /// Later declarations replace earlier ones: `main` and `room` are
    /// re-declared on **every room move** (77,497 and 77,411 of the census's
    /// 157,220 tags), carrying the room name as a subtitle, so this runs
    /// constantly and must be idempotent rather than accumulate.
    pub fn declare(&mut self, id: &str, attrs: &Attrs) {
        self.declared.insert(id.to_owned(), Closed::read(attrs));
    }

    /// What a stream declared, if it declared anything.
    #[must_use]
    pub fn declared(&self, id: &str) -> Option<&Closed> {
        self.declared.get(id)
    }

    /// Where a line on `stream` should go, given which windows are open.
    ///
    /// `is_open` is the frontend's answer, asked rather than stored: whether a
    /// window is on screen is not a fact about the game.
    ///
    /// # An undeclared stream goes to main
    ///
    /// Not dropped. The census found **16** declared ids and `plan/15` §1
    /// records that windows are declared far more widely than they are pushed
    /// to -- so a stream nobody declared is likelier to be one this build has
    /// not seen than one the server means to suppress. Dropping on a guess
    /// would lose text; showing it in main is recoverable and visible.
    ///
    /// # The chain terminates
    ///
    /// [`Closed::Route`] can point at a window that is also closed, and the
    /// wiki's own example is two hops (`voln → thoughts → main`). Nothing
    /// stops a server -- or a third-party tool injecting windows, which
    /// `plan/15` says is indistinguishable by design -- from declaring a cycle.
    /// A visited set makes that terminate at main rather than hang.
    #[must_use]
    pub fn route(&self, stream: &str, is_open: &impl Fn(&str) -> bool) -> Destination {
        // The main window is not a stream with a fallback; it IS the fallback.
        if stream.is_empty() || stream == MAIN {
            return Destination::Main;
        }
        let mut at = stream.to_owned();
        let mut seen = BTreeSet::new();
        loop {
            if is_open(&at) {
                return Destination::Window(at);
            }
            if !seen.insert(at.clone()) {
                // A cycle among closed windows. Main is the only safe answer.
                return Destination::Main;
            }
            match self.declared.get(&at) {
                Some(Closed::Drop) => return Destination::Dropped,
                Some(Closed::Styled(style)) => return Destination::MainStyled(style.clone()),
                Some(Closed::Route(next)) => at = next.clone(),
                // Declared as plain fall-through, or never declared at all.
                Some(Closed::Main) | None => return Destination::Main,
            }
        }
    }
}

/// The main window's stream id.
///
/// The wire writes main-window text with an **empty** stream, and names the
/// window `main` in `<streamWindow id="main">`. Both are handled by
/// [`Windows::route`], which is why this is one constant and not two.
pub const MAIN: &str = "main";

impl crate::GameState {
    /// Apply a `<streamWindow>`: **two independent facts ride one tag.**
    ///
    /// The room's name, on the `room`/`main` re-title that fires on every move
    /// (MEASURED: 77,497 and 77,411 of the census's 157,220 tags), and what
    /// this stream does when its window is closed. Both are recorded; only the
    /// first can change the room.
    ///
    /// Here rather than in `state.rs`'s `apply` under Rule 4.1 -- move code
    /// down, do not raise the cap. Adding this inline put `state.rs` at 567
    /// against its 550, and `split_parents_stay_facades` said so.
    /// Takes the whole frame rather than its three parts: destructuring it in
    /// `apply`'s match arm cost six lines there and put `state.rs` over its
    /// cap. Unpacking here instead is the same Rule 4.1 move in miniature.
    pub(super) fn apply_stream_window(&mut self, frame: &cena_protocol::Frame) {
        let cena_protocol::Frame::StreamWindow {
            id,
            subtitle,
            attrs,
            ..
        } = frame
        else {
            return;
        };
        self.stream_windows.declare(id, attrs);
        self.name_room(id, subtitle.as_deref());
    }
}
