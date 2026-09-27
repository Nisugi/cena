//! The trigger matcher (`crates/cena-model/src/trigger/matcher.rs`): which
//! triggers hit a finished line, where, and in what order.

use cena_model::trigger::{Hit, Matcher, Pattern, Rule, Trigger};

/// A squelch on `pattern`: what it does is not the matcher's business.
fn trigger(name: &str, pattern: Pattern) -> Trigger {
    Trigger {
        name: name.into(),
        rule: Rule {
            pattern: Some(pattern),
            squelch: true,
            ..Rule::default()
        },
    }
}

fn words(name: &str, text: &str) -> Trigger {
    trigger(
        name,
        Pattern::Literal {
            text: text.into(),
            whole_word: true,
        },
    )
}

fn inside_words(name: &str, text: &str) -> Trigger {
    trigger(
        name,
        Pattern::Literal {
            text: text.into(),
            whole_word: false,
        },
    )
}

fn regex(name: &str, source: &str) -> Trigger {
    trigger(name, Pattern::Regex(source.into()))
}

/// Each hit on the main stream as `name@start..end`.
fn hits(triggers: Vec<Trigger>, text: &str) -> Option<Vec<String>> {
    hits_on(triggers, "", text)
}

fn hits_on(triggers: Vec<Trigger>, stream: &str, text: &str) -> Option<Vec<String>> {
    let matcher = Matcher::new(triggers).ok()?;
    Some(
        matcher
            .hits(stream, text)
            .iter()
            .map(|hit: &Hit| {
                let name = &matcher.triggers()[hit.trigger].name;
                format!("{name}@{}..{}", hit.span.start, hit.span.end)
            })
            .collect(),
    )
}

#[test]
fn a_literal_matches_whole_words_unless_told_otherwise() {
    assert_eq!(
        hits(vec![words("stun", "stun")], "You are stunned.").unwrap(),
        [] as [&str; 0]
    );
    assert_eq!(
        hits(vec![words("stun", "stun")], "You stun_ it, stun it.").unwrap(),
        ["stun@14..18"],
        "`_` is part of a word; `,` and a space are not"
    );
    assert_eq!(
        hits(vec![words("stun", "stun")], "The restun fails.").unwrap(),
        [] as [&str; 0],
        "joined on the left"
    );
    assert_eq!(
        hits(vec![inside_words("stun", "stun")], "You are stunned.").unwrap(),
        ["stun@8..12"]
    );
}

/// Only an edge that is a letter, a digit or `_` needs a boundary. Wizard
/// FE's own example (`reference/wiki_clean/Wizard _front end_.txt:571`),
/// which there misses a GM's name until "Not on Word Boundary" is ticked,
/// and a Saga player's `[DemsDen] `, whose trailing space is there to refuse
/// `[DemsDen]No Space` (`reference/discord/saga-thread.txt:21114-21116`).
#[test]
fn an_edge_that_is_not_a_letter_needs_no_boundary() {
    let gm = "SEND[BigWizard] hi there..may I help you?";
    assert_eq!(
        hits(vec![words("send", "SEND[")], gm).unwrap(),
        ["send@0..5"]
    );
    assert_eq!(
        hits(vec![words("send", "SEND")], "SENDER waves.").unwrap(),
        [] as [&str; 0],
        "a letter at the edge still needs one"
    );
    assert_eq!(
        hits(vec![words("eyes", "'s eyes glow")], "Nisugi's eyes glow.").unwrap(),
        ["eyes@6..18"],
        "an apostrophe at the edge may follow a letter"
    );
    let den = || vec![words("den", "[DemsDen] ")];
    assert_eq!(
        hits(den(), "[DemsDen] Nisugi says hello.").unwrap(),
        ["den@0..10"]
    );
    assert_eq!(
        hits(den(), "[DemsDen]No Space See?").unwrap(),
        [] as [&str; 0]
    );
}

#[test]
fn case_is_ignored_unless_the_trigger_says_it_matters() {
    let line = "you see John. You see John.";
    let mut exact = words("exact", "You see");
    exact.rule.case_sensitive = true;
    assert_eq!(
        hits(vec![words("loose", "You see"), exact], line).unwrap(),
        ["loose@0..7", "loose@14..21", "exact@14..21"]
    );
    let mut exact_re = regex("exact-re", "You see");
    exact_re.rule.case_sensitive = true;
    assert_eq!(
        hits(vec![regex("loose-re", "you SEE"), exact_re], line).unwrap(),
        ["loose-re@0..7", "loose-re@14..21", "exact-re@14..21"]
    );
}

/// Two triggers over the same words both hit; an automaton searched for the
/// leftmost match only would report one of them.
#[test]
fn every_trigger_hits_every_place_even_where_they_overlap() {
    assert_eq!(
        hits(
            vec![words("stun", "stun"), words("you-stun", "you stun")],
            "You stun the rat. The rat stuns you. You stun it."
        )
        .unwrap(),
        [
            "stun@4..8",
            "stun@41..45",
            "you-stun@0..8",
            "you-stun@37..45"
        ]
    );
    // One trigger's own places do not overlap: the earlier is kept.
    assert_eq!(
        hits(vec![inside_words("aa", "aa")], "aaaa").unwrap(),
        ["aa@0..2", "aa@2..4"]
    );
}

#[test]
fn a_regex_reports_its_groups() {
    let matcher = Matcher::new(vec![regex("whisper", r"^(\w+) whispers(, (.*))?$")]).unwrap();
    let hits = matcher.hits("", "Dicate whispers");
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].span, 0..15);
    assert_eq!(hits[0].groups, [Some(0..6), None, None]);
}

#[test]
fn a_trigger_limited_to_a_stream_sees_only_that_stream() {
    let mut thoughts = words("thoughts", "Nisugi");
    thoughts.rule.stream = Some("thoughts".into());
    let mut main = words("main", "Nisugi");
    main.rule.stream = Some(String::new());
    let everywhere = words("everywhere", "Nisugi");
    let all = || vec![thoughts.clone(), main.clone(), everywhere.clone()];
    assert_eq!(
        hits_on(all(), "thoughts", "Nisugi thinks.").unwrap(),
        ["thoughts@0..6", "everywhere@0..6"]
    );
    assert_eq!(
        hits_on(all(), "main", "Nisugi waves.").unwrap(),
        ["main@0..6", "everywhere@0..6"],
        "the session may name the main stream `main` or `\"\"`"
    );
    assert_eq!(
        hits_on(all(), "", "Nisugi waves.").unwrap(),
        ["main@0..6", "everywhere@0..6"]
    );
}

/// Priority first, highest first; then the order given, which the file
/// makes category then name.
#[test]
fn triggers_rank_by_priority_then_the_order_given() {
    let mut urgent = words("urgent", "rat");
    urgent.rule.priority = 5;
    let mut ignored = words("ignored", "rat");
    ignored.rule.priority = -1;
    let matcher = Matcher::new(vec![
        ignored,
        words("first", "rat"),
        urgent,
        words("second", "rat"),
    ])
    .unwrap();
    let ranked: Vec<&str> = matcher
        .triggers()
        .iter()
        .map(|trigger| trigger.name.as_str())
        .collect();
    assert_eq!(ranked, ["urgent", "first", "second", "ignored"]);
    let order: Vec<usize> = matcher
        .hits("", "a rat")
        .iter()
        .map(|hit| hit.trigger)
        .collect();
    assert_eq!(order, [0, 1, 2, 3]);
}

/// A literal, a regex and a squelch of blank lines, in one matcher: each
/// kind is found, and a regex that matches nothing costs nothing.
#[test]
fn literals_and_regexes_together() {
    assert_eq!(
        hits(
            vec![
                words("rat", "rat"),
                regex("never", "^Zzz$"),
                regex("blank", r"^\s*$"),
                regex("roundtime", r"Roundtime: (\d+) sec"),
            ],
            "You hit the rat.  Roundtime: 3 sec."
        )
        .unwrap(),
        ["rat@12..15", "roundtime@18..34"]
    );
    assert_eq!(
        hits(vec![regex("blank", r"^\s*$")], "   ").unwrap(),
        ["blank@0..3"]
    );
}

#[test]
fn a_regex_that_cannot_be_built_is_refused_by_name() {
    let error = Matcher::new(vec![regex("bad", "a(?=b)")]).unwrap_err();
    assert!(error.starts_with("`bad`:"), "{error}");
}

#[test]
fn no_triggers_hit_nothing() {
    let matcher = Matcher::new(Vec::new()).unwrap();
    assert!(matcher.hits("", "anything").is_empty());
}
