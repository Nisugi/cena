//! The pseudonymiser, attacked on the three ways it could leak or corrupt.
//!
//! `scrub.rs` is what stands between the 49.55 GB corpus of real sessions and
//! a committed fixture. Its failures are not recoverable the way a parser bug
//! is: a real name that reaches a fixture is in the history, and a fixture
//! that silently merges two players teaches the parser about traffic that
//! never happened.
//!
//! Review finding PR-7 found three defects in one line --
//! `for (real, pseudonym) in &self.names { text = text.replace(..) }` -- and
//! one promise in the module doc that the marker list did not keep. Each is
//! pinned here.

use cena_protocol::scrub::Scrubber;

/// **Two players must not collapse into one.**
///
/// The replacements ran in sequence over the previous pass's OUTPUT, so with
/// `Al -> Bob` and `Bob -> Cy` -- an ordinary `BTreeMap` ordering -- an `Al`
/// became `Bob` and then `Cy`. Both players ended up as `Cy`.
///
/// A fixture that merges two people is worse than an unscrubbed one, because
/// it looks correct: the `exist=`-to-name correlation the fixtures exist to
/// test is now wrong in a way no test would notice.
#[test]
fn a_pseudonym_is_not_itself_rewritten_by_a_later_rule() {
    // **The pseudonym of one name is the real name of another.** That is
    // the collision: `Al -> Bob` puts a `Bob` into the text, and `Bob -> Cy`
    // then rewrites it. Chosen deliberately, because it is the case the old
    // sequential loop could not survive and an arbitrary pair would not test.
    let mut scrubber = Scrubber::default();
    scrubber.pseudonymise("Al", "Bob");
    scrubber.pseudonymise("Bob", "Cy");
    let (al, bob) = ("Bob", "Cy");

    let out = scrubber.scrub("Al waves to Bob.\n");
    let names: Vec<&str> = out.split_whitespace().collect();

    assert!(
        out.contains(al) && out.contains(bob),
        "both pseudonyms must appear: {out:?} (Al -> {al}, Bob -> {bob})"
    );
    assert_ne!(
        names.first(),
        names.last(),
        "the two players must still be two people after scrubbing: {out:?}"
    );
    assert!(
        !out.contains("Al ") && !out.contains(" Bob"),
        "no real name may survive: {out:?}"
    );
}

/// **A registered name must not rewrite the inside of a longer word.**
///
/// `replace` is a substring operation, so a registered `Eon` corrupted
/// `Eonake` into `<pseudonym>ake` -- mangling a name that was never
/// registered, and producing text the game never sent.
#[test]
fn a_short_name_does_not_rewrite_the_inside_of_a_longer_one() {
    let mut scrubber = Scrubber::default();
    scrubber.pseudonymise("Eon", "Zed");
    let eon = "Zed";

    let out = scrubber.scrub("Eon greets Eonake beside the Eonwood tree.\n");

    assert!(
        out.contains(eon),
        "the registered name must be replaced: {out:?}"
    );
    assert!(
        out.contains("Eonake"),
        "an unregistered longer name must survive intact -- rewriting its \
         inside produces a name the game never sent: {out:?}"
    );
    assert!(
        out.contains("Eonwood"),
        "and so must an ordinary word that happens to start the same: {out:?}"
    );
}

/// **Case must not let a name through.**
///
/// The wire capitalises a name at the start of a sentence and Lich lowercases
/// it in some command echoes, so a registered `Alderin` left every `alderin`
/// in place. The checker in `fixtures_are_scrubbed.rs` is case-sensitive too,
/// which is why nothing caught it.
#[test]
fn a_name_in_another_case_is_still_replaced() {
    let mut scrubber = Scrubber::default();
    scrubber.pseudonymise("Alderin", "Bracken");
    let pseudonym = "Bracken";

    let out = scrubber.scrub("alderin arrives. ALDERIN shouts. Alderin waves.\n");

    for leaked in ["alderin", "ALDERIN", "Alderin"] {
        assert!(
            !out.contains(leaked),
            "{leaked:?} survived scrubbing: {out:?}"
        );
    }
    assert!(
        out.to_lowercase()
            .matches(&pseudonym.to_lowercase())
            .count()
            == 3,
        "all three occurrences must be replaced: {out:?}"
    );
}

/// The pseudonym adopts the case of what it replaced.
///
/// Matching case-insensitively and emitting the pseudonym verbatim would turn
/// `alderin` into `Bob` mid-sentence -- a tell that the fixture was rewritten,
/// in files whose whole purpose is to look like wire traffic.
#[test]
fn the_pseudonym_wears_the_case_it_replaced() {
    let mut scrubber = Scrubber::default();
    scrubber.pseudonymise("Alderin", "Bracken");
    let pseudonym = "Bracken";

    let out = scrubber.scrub("alderin and ALDERIN and Alderin\n");
    assert!(
        out.contains(&pseudonym.to_lowercase()),
        "a lowercase occurrence stays lowercase: {out:?}"
    );
    assert!(
        out.contains(&pseudonym.to_uppercase()),
        "an uppercase occurrence stays uppercase: {out:?}"
    );
    assert!(
        out.contains(pseudonym),
        "a capitalised occurrence stays capitalised: {out:?}"
    );
}

/// **ESP is a private channel, and the doc always said so.**
///
/// Rule 3 of the module doc drops private channels rather than pseudonymising
/// them, because consent for a stranger's words is not obtainable. The marker
/// list covered whisper, society and bounty, and not `thoughts` -- the
/// ESP/telepathy channel (`reference/wiki_clean/Wrayth protocol.txt:61`),
/// whose own wiki example is a named player's sentence.
#[test]
fn esp_and_speech_lines_are_dropped_not_rewritten() {
    let scrubber = Scrubber::default();

    let out = scrubber.scrub(
        "keep me\n\
         <pushStream id=\"thoughts\"/>You hear the faint thoughts of Someone echo in your mind: a secret\n\
         keep me too\n\
         <pushStream id='speech'/>Someone says, \"a private thing\"\n\
         and me\n",
    );

    assert!(
        !out.contains("a secret"),
        "an ESP line must be DROPPED, not pseudonymised -- a pseudonym does \
         not make a stranger's sentence publishable: {out:?}"
    );
    assert!(
        !out.contains("a private thing"),
        "a speech line must be dropped too: {out:?}"
    );
    assert!(
        out.contains("keep me") && out.contains("keep me too") && out.contains("and me"),
        "ordinary lines either side must survive: {out:?}"
    );
}

/// `thoughts` reaching main as inline text is still ESP.
///
/// `thoughts` is an "Exclusive" stream (`Wrayth protocol.txt:76`): with the
/// window closed the text falls through into main wrapped in a style, so the
/// `pushStream` marker is absent and only the prose form identifies it.
#[test]
fn inline_thoughts_without_a_push_stream_are_still_dropped() {
    let scrubber = Scrubber::default();
    let out = scrubber.scrub(
        "keep me\n\
         <preset id='thought'>You hear the faint thoughts of Someone echo in your mind: hello</preset>\n\
         keep me too\n",
    );
    assert!(
        !out.contains("hello"),
        "an inline ESP line carries the same words as a streamed one and must \
         be dropped the same way: {out:?}"
    );
    assert!(
        out.contains("keep me") && out.contains("keep me too"),
        "ordinary lines must survive: {out:?}"
    );
}
