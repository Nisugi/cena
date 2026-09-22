//! Matching a line against the bestiary: arrival, flee, death, decay.
//!
//! MEASURED before writing any of this: `creature_messages.tsv` holds 3,863
//! lines and `Creature::messages_of` had **no caller**. The table was loaded
//! and entirely unread, which is why `plan/20`'s third hiding condition --
//! "not seen to leave" -- could not be written.
//!
//! The sweep at the bottom is the real test: every templated line in the whole
//! table, rendered with each of its placeholder's forms, must match itself.

use cena_model::state::creature::{self, MessageKind};
use cena_model::state::creature_message::{classify, match_template};

#[test]
fn a_literal_line_matches_itself_and_nothing_else() {
    let t = "An Agresh bear lumbers in!";
    assert_eq!(match_template(t, t), Some(None), "no direction in it");
    assert_eq!(match_template(t, "An Agresh bear lumbers out!"), None);
    assert_eq!(match_template(t, "You see nothing unusual."), None);
}

#[test]
fn a_direction_placeholder_captures_the_direction() {
    let t = "An Agresh bear lumbers {direction}.";
    assert_eq!(
        match_template(t, "An Agresh bear lumbers north."),
        Some(Some("north".to_owned()))
    );
    // `out` is a direction in Lich's list and is not a compass point. Its
    // absence once made every such line unmatchable (`creature.rb:852`).
    assert_eq!(
        match_template(t, "An Agresh bear lumbers out."),
        Some(Some("out".to_owned()))
    );
    // A word that is not a direction must not match.
    assert_eq!(match_template(t, "An Agresh bear lumbers away."), None);
}

#[test]
fn the_longest_direction_wins() {
    // `northeast` must not be read as `north` with `east.` left over -- which
    // is exactly what a first-match scan would do.
    let t = "A troll runs {direction}.";
    assert_eq!(
        match_template(t, "A troll runs northeast."),
        Some(Some("northeast".to_owned()))
    );
    assert_eq!(
        match_template(t, "A troll runs southwest."),
        Some(Some("southwest".to_owned()))
    );
}

#[test]
fn a_pronoun_placeholder_matches_every_form() {
    let t = "An Agresh bear slowly backs away, {pronoun} teeth bared.";
    for form in ["his", "her", "its", "their"] {
        assert_eq!(
            match_template(
                t,
                &format!("An Agresh bear slowly backs away, {form} teeth bared.")
            ),
            Some(None),
            "form: {form}"
        );
    }
    assert_eq!(
        match_template(t, "An Agresh bear slowly backs away, the teeth bared."),
        None
    );
}

#[test]
fn two_placeholders_in_one_line_both_bind() {
    let t = "An Agresh bear lumbers {direction}, flecks of drool flinging with each of {pronoun} strides.";
    assert_eq!(
        match_template(
            t,
            "An Agresh bear lumbers west, flecks of drool flinging with each of its strides."
        ),
        Some(Some("west".to_owned()))
    );
    // The tail must still match: a line that stops early is not this message.
    assert_eq!(
        match_template(t, "An Agresh bear lumbers west, flecks of drool."),
        None
    );
}

#[test]
fn a_wildcard_stops_at_the_next_literal_run() {
    // `{weapon}` is Lich's `RAW:.+?`, lazy. `{target}` is absent from Lich's
    // map entirely, so its 24 lines are unmatchable there; we treat it as a
    // wildcard, which is a deliberate divergence.
    let t = "A hunch-backed dogmatist trots in with {pronoun} {weapon}!";
    assert_eq!(
        match_template(
            t,
            "A hunch-backed dogmatist trots in with its rusty scimitar!"
        ),
        Some(None)
    );
    // A wildcard requires at least one character, as `.+?` does.
    assert_eq!(
        match_template(t, "A hunch-backed dogmatist trots in with its !"),
        None
    );
}

#[test]
fn classify_names_the_kind_and_the_direction() {
    let bear = creature::creature("agresh_bear").expect("in the bestiary");
    let found = classify(
        bear,
        "An Agresh bear lumbers north.",
        &[MessageKind::Death, MessageKind::Flee],
    )
    .expect("a flee line");
    assert_eq!(found.kind, MessageKind::Flee);
    assert_eq!(found.direction.as_deref(), Some("north"));
}

#[test]
fn a_line_from_another_creature_does_not_match_this_one() {
    let bear = creature::creature("agresh_bear").expect("in the bestiary");
    assert!(
        classify(
            bear,
            "An Agresh troll chieftain runs north.",
            &[MessageKind::Flee]
        )
        .is_none()
    );
}

#[test]
fn an_arrival_is_told_from_a_flee() {
    // The pair that matters for the hiding inference: one means it is here,
    // the other that it is gone, and both name the same creature.
    let bear = creature::creature("agresh_bear").expect("in the bestiary");
    let kinds = [MessageKind::Arrival, MessageKind::Flee];
    assert_eq!(
        classify(bear, "An Agresh bear lumbers in!", &kinds).map(|m| m.kind),
        Some(MessageKind::Arrival)
    );
    assert_eq!(
        classify(bear, "An Agresh bear lumbers north.", &kinds).map(|m| m.kind),
        Some(MessageKind::Flee)
    );
}

/// **The sweep: every templated line in the table must match its own
/// renderings.**
///
/// This is what makes the placeholder vocabularies honest. A form missing from
/// `PRONOUN` or `DIRECTION` makes an otherwise-correct message unmatchable, and
/// Lich's own comment says that happened to it -- so the test renders each
/// template with *every* form of *every* placeholder it carries, and requires
/// all of them back.
#[test]
fn every_templated_message_in_the_table_matches_its_own_renderings() {
    const PRONOUN: &[&str] = &[
        "he",
        "she",
        "it",
        "his",
        "her",
        "its",
        "their",
        "him",
        "them",
        "himself",
        "herself",
        "itself",
        "themselves",
    ];
    const REFLEXIVE: &[&str] = &["himself", "herself", "itself", "themselves"];
    const DIRECTION: &[&str] = &[
        "north",
        "south",
        "east",
        "west",
        "up",
        "down",
        "out",
        "northeast",
        "northwest",
        "southeast",
        "southwest",
    ];

    let mut checked = 0_usize;
    let mut templated = 0_usize;
    let mut failures = Vec::new();

    for creature in creature::creatures() {
        for kind in [
            MessageKind::Arrival,
            MessageKind::Flee,
            MessageKind::Death,
            MessageKind::Decay,
        ] {
            for template in creature.messages_of(kind) {
                if !template.contains('{') {
                    // A literal line must match itself exactly.
                    checked += 1;
                    if match_template(template, template).is_none() {
                        failures.push(format!("literal did not match itself: {template}"));
                    }
                    continue;
                }
                templated += 1;
                // **Each placeholder gets ITS OWN vocabulary.** A first
                // version substituted one word everywhere and produced
                // nonsense like "lumbers north, ... each of north strides",
                // which the matcher correctly refused -- 550 of 16,342. The
                // renderer was wrong, not the matcher.
                //
                // The lists are walked in step so that every form of every
                // vocabulary is exercised across the sweep; the longest list
                // sets the count.
                let steps = DIRECTION.len().max(PRONOUN.len());
                for step in 0..steps {
                    let rendered = template
                        .replace("{direction}", DIRECTION[step % DIRECTION.len()])
                        .replace("{pronoun}", PRONOUN[step % PRONOUN.len()])
                        .replace("{reflexive}", REFLEXIVE[step % REFLEXIVE.len()])
                        .replace("{target}", "the warrior")
                        .replace("{weapon}", "a rusty blade");
                    if rendered.contains('{') {
                        continue; // an unknown placeholder; the guards below catch a drift
                    }
                    checked += 1;
                    if match_template(template, &rendered).is_none() {
                        failures.push(format!("{template}\n  did not match: {rendered}"));
                    }
                }
            }
        }
    }

    assert!(
        templated > 1_000,
        "guard: the table should carry over a thousand templated lines, saw {templated}"
    );
    assert!(
        checked > 10_000,
        "guard: the sweep should check ten thousand renderings, saw {checked}"
    );
    assert!(
        failures.is_empty(),
        "{} of {checked} renderings did not match:\n{}",
        failures.len(),
        failures
            .iter()
            .take(12)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
