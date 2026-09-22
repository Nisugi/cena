//! The closed-window routing rule: `ifClosed` / `styleIfClosed`.
//!
//! The author asked the question that started this:
//!
//! > **2026-09-21:** *"When a stream comes in and it has an ifClosed=
//! > attribute, that attribute tells us what to do with that stream if it's
//! > stream window is closed. I forget the rules though"*
//!
//! The rules are the protocol wiki's
//! (`reference/wiki_clean/Wrayth protocol.txt:73-79`), and the declarations
//! below are the ones a real login sends -- censused over the **208** `.xml`
//! logs in `E:\Gemstone\dev\lich-5\logs\GSIV-Nisugi`, **157,220**
//! `<streamWindow>` tags.
//!
//! The headline test is [`a_speech_line_is_not_shown_twice`]: the author's own
//! paste, which is the duplicate that prompted all of this.

use cena_model::GameState;
use cena_model::state::stream_windows::{Closed, Destination};
use cena_protocol::Parser;

/// The login burst's declarations, verbatim in shape from the wire.
///
/// MEASURED: 15 of the 16 declared ids arrive before the first prompt, so this
/// is what a real connection teaches. Attribute spelling matters -- `ifClosed=''`
/// and an absent `ifClosed` mean opposite things.
const DECLARATIONS: &str = concat!(
    "<streamWindow id='main' title='Story' location='center' target='drop' resident='true'/>",
    "<streamWindow id='speech' title='Speech' scroll='auto' ifClosed='' appearance='story' location='right' resident='true' save='true' timestamp='on'/>",
    "<streamWindow id='thoughts' title='Thoughts' location='center' resident='true' timestamp='on' styleIfClosed='thought'/>",
    "<streamWindow id='familiar' title='Familiar' location='center' resident='true' styleIfClosed='watching'/>",
    "<streamWindow id='logons' title='Arrivals' location='left' resident='true' timestamp='on' nameFilterOption='true'/>",
    "<streamWindow id='death' title='Deaths' location='left' resident='true' timestamp='on' nameFilterOption='true'/>",
    "<streamWindow id='ambients' title='Ambients' location='center' resident='true' styleIfClosed=''/>",
    "<streamWindow id='inv' title='My Inventory' ifClosed='' location='right' resident='true'/>",
    "\n",
);

fn fed(wire: &str) -> GameState {
    let mut state = GameState::default();
    for frame in Parser::new().push_bytes(wire.as_bytes()) {
        state.apply(&frame);
    }
    state
}

/// Nothing is open. The frontend that has no windows at all.
fn all_closed(_: &str) -> bool {
    false
}

#[test]
fn the_four_declarations_read_as_four_behaviours() {
    let state = fed(DECLARATIONS);
    let windows = state.stream_windows();

    // Copy: a duplicate, because the server also sent it to main.
    assert_eq!(windows.declared("speech"), Some(&Closed::Drop));
    assert_eq!(windows.declared("inv"), Some(&Closed::Drop));
    // Exclusive: falls through to main wearing a style.
    assert_eq!(
        windows.declared("familiar"),
        Some(&Closed::Styled("watching".to_owned()))
    );
    // Unspecified: falls through to main, plain.
    assert_eq!(windows.declared("logons"), Some(&Closed::Main));
    assert_eq!(windows.declared("death"), Some(&Closed::Main));
}

/// **`ifClosed=''` and no `ifClosed` are opposite, and this is the test that
/// says so.**
///
/// They are one character apart in the markup and mean "drop this text" versus
/// "show this text in main". A reader that used `unwrap_or_default()` would
/// collapse them and silently drop every `logons` and `death` line.
#[test]
fn an_empty_if_closed_is_not_an_absent_one() {
    let state = fed(DECLARATIONS);
    let windows = state.stream_windows();
    assert_eq!(windows.declared("speech"), Some(&Closed::Drop));
    assert_eq!(windows.declared("logons"), Some(&Closed::Main));
    assert_ne!(windows.declared("speech"), windows.declared("logons"));
}

/// `thoughts` is Exclusive: ESP falls through to main wearing `thought`.
///
/// **A fixture I wrote by hand claimed `thoughts` sent BOTH `ifClosed=''` and
/// `styleIfClosed='thought'`, and a test asserted it read as `Drop`.** Checked
/// against the wire before trusting it:
///
/// ```text
/// <streamWindow id="thoughts" title="Thoughts" location="center"
///               resident="true" timestamp='on' styleIfClosed="thought"/>
/// ```
///
/// No `ifClosed` at all. So it is the wiki's `Exclusive` row, and the inline
/// `thought` look is exactly what falling through to main is FOR. Had the
/// invented fixture stood, this test would have asserted that ESP is silently
/// dropped by a client with no Thoughts window.
#[test]
fn thoughts_falls_through_to_main_wearing_its_style() {
    let state = fed(DECLARATIONS);
    assert_eq!(
        state.stream_windows().declared("thoughts"),
        Some(&Closed::Styled("thought".to_owned()))
    );
    assert_eq!(
        state.stream_windows().route("thoughts", &all_closed),
        Destination::MainStyled("thought".to_owned())
    );
}

/// `ifClosed` is still read before `styleIfClosed` when a stream sends both.
///
/// The wiki's `Exclusive` row is *"`ifClosed` absent, `styleIfClosed` set"*, so
/// a stream carrying both is routed rather than styled. **No live declaration
/// sends both** -- the census found none -- so this pins the precedence rule
/// against a synthetic input, and says so.
#[test]
fn if_closed_is_read_before_style_if_closed() {
    let state = fed("<streamWindow id='both' ifClosed='' styleIfClosed='thought'/>\n");
    assert_eq!(state.stream_windows().declared("both"), Some(&Closed::Drop));
}

/// An **empty** `styleIfClosed` is no style at all.
///
/// `ambients` declares it that way -- a shape the wiki's four-row table does
/// not name. An empty style string would otherwise become
/// `MainStyled("")`, and a renderer would look up a style called `""`.
#[test]
fn an_empty_style_is_no_style() {
    let state = fed(DECLARATIONS);
    assert_eq!(
        state.stream_windows().declared("ambients"),
        Some(&Closed::Main)
    );
    assert_eq!(
        state.stream_windows().route("ambients", &all_closed),
        Destination::Main
    );
}

#[test]
fn an_open_window_gets_its_own_text_whatever_it_declared() {
    let state = fed(DECLARATIONS);
    // The whole rule is about CLOSED windows. Open ones are unremarkable, and
    // that must hold even for a `Drop` stream -- the copy is only redundant
    // when there is nowhere to put it.
    let open = |id: &str| id == "speech";
    assert_eq!(
        state.stream_windows().route("speech", &open),
        Destination::Window("speech".to_owned())
    );
}

#[test]
fn a_closed_copy_stream_is_dropped_and_a_closed_styled_one_is_not() {
    let state = fed(DECLARATIONS);
    let windows = state.stream_windows();
    assert_eq!(windows.route("speech", &all_closed), Destination::Dropped);
    assert_eq!(
        windows.route("familiar", &all_closed),
        Destination::MainStyled("watching".to_owned())
    );
    assert_eq!(windows.route("logons", &all_closed), Destination::Main);
}

/// Main is the destination, never a stream with a fallback of its own.
///
/// The wire writes main-window text with an **empty** stream id and also
/// declares a window called `main`. Both have to answer `Main` or the rule
/// would recurse on the thing it falls back to.
#[test]
fn the_main_window_is_always_itself() {
    let state = fed(DECLARATIONS);
    assert_eq!(
        state.stream_windows().route("", &all_closed),
        Destination::Main
    );
    assert_eq!(
        state.stream_windows().route("main", &all_closed),
        Destination::Main
    );
}

/// An undeclared stream goes to main rather than being dropped.
///
/// MEASURED: 16 ids are declared and only 6 are ever pushed to, and `plan/15`
/// warns a frontend must not assume the set. So an unknown stream is likelier
/// to be one this build has not seen than one the server means to suppress, and
/// showing text in the wrong place is recoverable where dropping it is not.
#[test]
fn a_stream_nobody_declared_falls_through_to_main() {
    let state = fed(DECLARATIONS);
    assert_eq!(
        state.stream_windows().route("percWindow", &all_closed),
        Destination::Main
    );
}

/// The `Routed` chain, and the reason it is a loop.
///
/// **UNVERIFIED against live traffic**: the wiki cites `voln → thoughts` and
/// the 208-file census found no `ifClosed='<window>'` at all. Built from the
/// wiki and tested from the wiki, labelled as such in both places.
#[test]
fn a_routed_stream_chains_to_its_target() {
    let wire = concat!(
        "<streamWindow id='thoughts' styleIfClosed='thought'/>",
        "<streamWindow id='voln' ifClosed='thoughts'/>\n",
    );
    let state = fed(wire);
    // voln is closed, so it routes to thoughts; thoughts is closed too, so it
    // falls through to main wearing `thought`. Two hops, the wiki's example.
    assert_eq!(
        state.stream_windows().route("voln", &all_closed),
        Destination::MainStyled("thought".to_owned())
    );
    // With thoughts OPEN, the chain stops there instead.
    let thoughts_open = |id: &str| id == "thoughts";
    assert_eq!(
        state.stream_windows().route("voln", &thoughts_open),
        Destination::Window("thoughts".to_owned())
    );
}

/// A cycle among closed windows terminates at main.
///
/// Nothing in the protocol forbids one, and `plan/15` §530 records that
/// third-party tools inject windows through the same grammar and are
/// *"indistinguishable from Simutronics traffic by design"* -- so this is a
/// hostile-input case, not a hypothetical. Without the visited set it hangs.
#[test]
fn a_cycle_of_closed_windows_does_not_hang() {
    let wire = concat!(
        "<streamWindow id='a' ifClosed='b'/>",
        "<streamWindow id='b' ifClosed='a'/>\n",
    );
    let state = fed(wire);
    assert_eq!(
        state.stream_windows().route("a", &all_closed),
        Destination::Main
    );
}

/// A re-declaration replaces, rather than accumulating.
///
/// MEASURED: `main` and `room` are re-declared on **every room move** -- 77,497
/// and 77,411 of the census's 157,220 tags -- so this path runs constantly.
#[test]
fn re_declaring_a_window_replaces_what_it_said() {
    let state = fed(concat!(
        "<streamWindow id='loot' ifClosed=''/>",
        "<streamWindow id='loot' styleIfClosed='whisper'/>\n",
    ));
    assert_eq!(
        state.stream_windows().declared("loot"),
        Some(&Closed::Styled("whisper".to_owned())),
        "the later declaration is the live one"
    );
}

/// **THE HEADLINE: the author's duplicate, end to end.**
///
/// Verbatim from the author (2026-09-21), the wire that started this:
///
/// ```text
/// <pushStream id="speech"/><preset id='speech'>You <a ...>say</a></preset>, "Yep."
/// <popStream/>
/// <preset id='speech'>You <a ...>say</a></preset>, "Yep."
/// <prompt time="1790052928">&gt;</prompt>
/// ```
///
/// The same sentence arrives twice. The parser reports both -- correctly, they
/// are both on the wire -- one tagged `speech` and one on main. A renderer that
/// shows every line shows the character saying everything twice, which is what
/// Despana does today.
///
/// With no speech window open, the rule says the `speech` copy is `Dropped` and
/// the main copy is `Main`: one line on screen.
#[test]
fn a_speech_line_is_not_shown_twice() {
    let wire = concat!(
        "<streamWindow id='speech' title='Speech' ifClosed='' resident='true'/>\n",
        "<pushStream id=\"speech\"/><preset id='speech'>You ",
        "<a exist=\"-10966483\" coord=\"2524,1836\" noun=\"help\">say</a></preset>, \"Yep.\"\n",
        "<popStream/>\n",
        "<preset id='speech'>You ",
        "<a exist=\"-10966483\" coord=\"2524,1836\" noun=\"help\">say</a></preset>, \"Yep.\"\n",
        "<prompt time=\"1790052928\">&gt;</prompt>\n",
    );
    let state = fed(wire);

    // Both copies really are in the model -- this is not the parser losing one.
    assert_eq!(state.stream("speech").len(), 1, "the stream copy");
    assert_eq!(state.stream("").len(), 1, "the main copy");

    // The rule resolves the duplicate: with no speech window, one survives.
    let windows = state.stream_windows();
    assert_eq!(windows.route("speech", &all_closed), Destination::Dropped);
    assert_eq!(windows.route("", &all_closed), Destination::Main);

    // And with a speech window open, BOTH are shown -- in different windows,
    // which is what the duplicate is for.
    let open = |id: &str| id == "speech";
    assert_eq!(
        windows.route("speech", &open),
        Destination::Window("speech".to_owned())
    );
    assert_eq!(windows.route("", &open), Destination::Main);
}

/// The declarations do not survive a reconnect, and the burst re-teaches them.
///
/// MEASURED: 15 of 16 declared ids arrive before the first prompt. The
/// exception is `charprofile`, which only a `profile` command declares -- and a
/// command-taught fact from a connection that ended is exactly what
/// `invalidate_for_reconnect` clears.
///
/// Keeping a stale `ifClosed` would be worse than keeping other stale state,
/// because this one decides whether text is DROPPED.
#[test]
fn a_reconnect_forgets_what_the_windows_declared() {
    let mut state = fed(DECLARATIONS);
    assert!(state.stream_windows().declared("speech").is_some());
    state.invalidate_for_reconnect();
    assert_eq!(
        state.stream_windows().declared("speech"),
        None,
        "the next connection's burst says what its windows do"
    );
    // And with nothing declared, text is shown rather than dropped.
    assert_eq!(
        state.stream_windows().route("speech", &all_closed),
        Destination::Main
    );
}
