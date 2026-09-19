//! **The server warns before it kicks**, and nothing was listening.
//!
//! ```text
//! \x07YOU HAVE BEEN IDLE TOO LONG. PLEASE RESPOND.\x07
//! ```
//!
//! MEASURED 2026-09-19 across the log archive: 6 occurrences, 6 different
//! characters (`GSIV-Dicate` x2, `GSIV-Getho`, `GSIV-Hypate`, `GSIV-Monstr`,
//! `GSIV-Zoleta`). It is a bare text line wrapped in bell characters, with no
//! tag of any kind, and it is followed by an ordinary `<prompt>`.
//!
//! **It is a WARNING, not a kick.** Every one of the six sessions continued:
//! `GSIV-Getho` and `GSIV-Zoleta` show the player answering with `look` and
//! carrying on; `GSIV-Hypate` shows vitals still arriving 27 seconds later.
//! `GSIV-Dicate` shows a `quit` a few seconds after -- the player left, which is
//! a decision, not a disconnect.
//!
//! # Why the supervisor needs this and cannot infer it
//!
//! `MAX_UNATTENDED_LOSSES = 2` exists because *"the game idle-kicks after ~30
//! minutes, and without this cap the supervisor would re-login all night"*
//! (ported from `VellumFE`). But an idle kick is the case where the connection
//! **worked perfectly** -- so `worked` resets the backoff ladder, and the
//! supervisor reconnects at the one-second rung, plays for 30 minutes of
//! nothing, and is kicked again. The cap catches it only after two full cycles,
//! which is an hour of pointless re-login.
//!
//! The server says it is about to happen, in advance, in plain text. Lich
//! **strips the bells and discards the line**
//! (`XMLCleaner.fix_invalid_characters`, `reference/lich-5/lib/games.rb:315-325`;
//! its own test at `spec/lib/games_spec.rb:38-43` asserts only that the bell is
//! gone). `VellumFE` does not mention it. This is a fact both references throw
//! away.
//!
//! # Why it lives in the model and not the supervisor
//!
//! The supervisor never sees a frame -- the actor does, and it already folds
//! every frame into `GameState` (`cena-session/src/actor/io.rs:344`). So the
//! model **observes** and the supervisor **decides**, which is the layering
//! `plan/12` already has. A supervisor that scanned text would need its own
//! parser feed.
//!
//! # Why this is not the text-scraping `plan/12` 7.1 puts Out
//!
//! Out is the ~160 regexes in Lich's `infomon/parser.rb`, which match prose
//! whose shape the game may change at will. This is **one exact server string**
//! with no capture groups and nothing to extract -- the presence of the line
//! *is* the fact. If Simutronics ever rewords it, the warning stops being
//! observed and the supervisor falls back to the cap it uses today, which is the
//! behaviour before this existed. It degrades to the status quo rather than
//! misfiring.

use cena_model::GameState;
use cena_protocol::{Frame, Parser};

/// Fold a real wire chunk, the way the actor does.
fn fold(wire: &[u8]) -> GameState {
    let mut state = GameState::default();
    let mut parser = Parser::new();
    for frame in parser.push_bytes(wire) {
        state.apply(&frame);
    }
    state
}

/// The line as the wire actually carries it, bells included.
const IDLE_LINE: &[u8] = b"\x07YOU HAVE BEEN IDLE TOO LONG. PLEASE RESPOND.\x07\n";

#[test]
fn the_warning_is_recorded_with_the_server_clock() {
    // A prompt first, so there is a server clock to stamp against, then the
    // warning -- the order `GSIV-Getho` shows.
    let mut wire = b"<prompt time=\"1756968447\">J&gt;</prompt>\n".to_vec();
    wire.extend_from_slice(IDLE_LINE);
    let state = fold(&wire);

    assert_eq!(
        state.idle_warned_at(),
        Some(1_756_968_447),
        "the idle warning was not recorded; the supervisor cannot tell an idle \
         kick from a network drop"
    );
}

#[test]
fn an_ordinary_line_does_not_arm_it() {
    let state = fold(b"<prompt time=\"1756968447\">J&gt;</prompt>\nYou see nothing unusual.\n");
    assert_eq!(state.idle_warned_at(), None);
}

#[test]
fn a_line_merely_containing_the_words_does_not_arm_it() {
    // A player could say this. So could a sign, a book, or a scripted NPC. The
    // server's line is the WHOLE line; anything embedded in prose is someone
    // else talking.
    let mut wire = b"<prompt time=\"1756968447\">J&gt;</prompt>\n".to_vec();
    wire.extend_from_slice(
        b"Someone says, \"YOU HAVE BEEN IDLE TOO LONG. PLEASE RESPOND. or so I heard\"\n",
    );
    let state = fold(&wire);
    assert_eq!(
        state.idle_warned_at(),
        None,
        "prose containing the words armed the warning; a player could then make \
         the supervisor stop reconnecting by saying it"
    );
}

#[test]
fn no_inbound_frame_disarms_it_however_much_arrives() {
    // THE PREMISE THIS TEST ORIGINALLY HAD WAS FALSE, and the corpus said so.
    // It asserted that a later prompt disarms the warning, on the theory that a
    // prompt means the player answered. MEASURED on `GSIV-Dicate` (2024-10-12,
    // 12:18): after the warning the session idled on for over half a minute with
    // a prompt every few seconds, driven by `dialogData id='Buffs'` refreshes and
    // by a bystander emoting. A prompt means SOMETHING happened, not that the
    // player acted.
    //
    // So nothing inbound clears it. Only an outbound command does, and that is
    // the actor's job -- this model is inbound-only.
    let mut wire = b"<prompt time=\"1728753477\">&gt;</prompt>
"
    .to_vec();
    wire.extend_from_slice(IDLE_LINE);
    wire.extend_from_slice(
        b"<prompt time=\"1728753482\">&gt;</prompt>
",
    );
    // A buff refresh and a bystander, exactly as the capture has them.
    wire.extend_from_slice(
        b"<dialogData id='Buffs' clear='t'></dialogData>
",
    );
    wire.extend_from_slice(
        b"<prompt time=\"1728753487\">&gt;</prompt>
",
    );
    wire.extend_from_slice(
        b"Hypate writhes, her face contorting.
",
    );
    wire.extend_from_slice(
        b"<prompt time=\"1728753504\">&gt;</prompt>
",
    );
    let state = fold(&wire);

    assert!(
        state.idle_warned(),
        "inbound traffic disarmed the warning, so a session idling toward a kick          would look attended: the capture shows 27 seconds of exactly this"
    );
    assert_eq!(
        state.idle_warned_at(),
        Some(1_728_753_477),
        "and the timestamp must stay the moment of the WARNING"
    );
}

#[test]
fn answering_it_is_what_clears_it() {
    // What the actor calls on an outbound write.
    let mut wire = b"<prompt time=\"1756968447\">J&gt;</prompt>
"
    .to_vec();
    wire.extend_from_slice(IDLE_LINE);
    let mut state = fold(&wire);
    assert!(state.idle_warned());

    state.answer_idle_warning();

    assert!(!state.idle_warned(), "an outbound command did not clear it");
    assert_eq!(state.idle_warned_at(), None);
}

#[test]
fn a_warning_with_no_clock_yet_is_still_recorded() {
    // Defensive: the login burst carries no prompt until the end, so a warning
    // arriving before any prompt has no server clock. It must still be a fact.
    let state = fold(IDLE_LINE);
    assert!(
        state.idle_warned(),
        "a warning with no clock was dropped entirely"
    );
    assert_eq!(state.idle_warned_at(), None, "and it has no timestamp");
}

#[test]
fn a_reconnect_forgets_the_warning() {
    // `plan/12` 5.2: a new connection re-teaches everything. An idle warning is
    // a fact about the connection that ended, and carrying it across would make
    // the FIRST disconnect after a reconnect look like a second idle kick.
    let mut wire = b"<prompt time=\"1756968447\">J&gt;</prompt>\n".to_vec();
    wire.extend_from_slice(IDLE_LINE);
    let mut state = fold(&wire);
    assert!(state.idle_warned(), "armed before the reconnect");

    state.invalidate_for_reconnect();

    assert!(!state.idle_warned(), "the warning survived a reconnect");
}

#[test]
fn the_bells_are_not_required() {
    // The parser strips control characters before the text reaches a frame
    // (`text::strip_control_chars`), so the model must match on the stripped
    // form. Pinned because a future change to stripping would otherwise turn
    // this off silently.
    let state =
        fold(b"<prompt time=\"1\">&gt;</prompt>\nYOU HAVE BEEN IDLE TOO LONG. PLEASE RESPOND.\n");
    assert_eq!(state.idle_warned_at(), Some(1));
}

#[test]
fn the_frame_is_published_either_way() {
    // Rule 2.2: observing a line must not consume it. A renderer still has to
    // show the player the warning.
    let mut parser = Parser::new();
    let frames = parser.push_bytes(IDLE_LINE);
    let shown: Vec<&str> = frames
        .iter()
        .filter_map(|f| match f {
            Frame::Text(t) => Some(t.content.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(shown, vec!["YOU HAVE BEEN IDLE TOO LONG. PLEASE RESPOND."]);
}
