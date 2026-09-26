//! The interaction monitor (`hunt/monitor.rs`, `bigshot.lic:6797-6826`).

use cena_behavior::hunt::{Hunt, Profile, import};

fn hunt(monitor: &str) -> Result<Hunt, String> {
    Ok(Hunt::new(
        Profile::parse(&format!("[monitor]\n{monitor}\n"))?,
        1,
    ))
}

#[test]
fn a_watched_line_is_raised_unless_a_safe_pattern_covers_it() {
    let mut h =
        hunt("interaction = true\nstrings = [\"taps you\", \"whispers,\"]\nsafe = [\"Private\"]")
            .unwrap();
    h.watched("Somebody taps you on the shoulder.");
    h.watched("A GM WHISPERS, \"Are you there?\"");
    h.watched("[Private]-GSIV:Somebody: \"whispers, hi\"");
    h.watched("A kobold swings a club at you!");
    assert_eq!(
        h.take_alerts(),
        [
            "Somebody taps you on the shoulder.",
            "A GM WHISPERS, \"Are you there?\""
        ],
        "case does not matter; the safe line and the fight are not raised"
    );
    assert!(h.take_alerts().is_empty(), "taken once");
}

#[test]
fn off_nothing_is_raised_and_on_without_strings_it_is_bigshots_list() {
    let mut off = hunt("strings = [\"taps you\"]").unwrap();
    off.watched("Somebody taps you on the shoulder.");
    assert!(off.take_alerts().is_empty());
    let mut default = hunt("interaction = true").unwrap();
    default.watched("Someone is speaking to you.");
    default.watched("R E P O R T this");
    assert_eq!(default.take_alerts().len(), 2, "bigshot.lic:556's list");
}

#[test]
fn a_pattern_that_does_not_read_turns_the_monitor_off_and_says_so() {
    // A lookahead, which the regex crate does not read (inside brackets the
    // same characters are a plain class, and read).
    let mut h = hunt("interaction = true\nsafe = [\"(?!Private)GS\"]").unwrap();
    h.watched("Somebody taps you on the shoulder.");
    assert!(h.take_alerts().is_empty());
    let notes = h.take_notes();
    assert!(
        notes
            .iter()
            .any(|n| n.contains("interaction monitor is off")),
        "{notes:?}"
    );
}

#[test]
fn bigshots_joined_lists_are_split() {
    let brought = import(
        "t",
        "monitor_interaction: true\nmonitor_strings: taps you||whispers,\nmonitor_safe_strings: Private\n",
    )
    .unwrap();
    let m = &brought.profile.monitor;
    assert!(m.interaction);
    assert_eq!(m.strings, ["taps you", "whispers,"]);
    assert_eq!(m.safe, ["Private"]);
}
