//! Rule 2.1: nothing above `cena-protocol` ever sees a raw byte or an
//! unparsed string -- and Rule 2.2's one mandated exception to it.
//!
//! Split out of `file_rules.rs` under the rule that file enforces
//! (`plan/05:352-353`): move code down, do not raise the cap. Adding one cap
//! exception put it at 804 of 800.
//!
//! That file's own history is the precedent -- it has gone red four times and
//! split four times rather than raise its cap once, into `src/harness.rs`,
//! `src/lexical.rs`, `src/plan_rules.rs`, `tests/architecture.rs` and
//! `tests/ratchet.rs`. This is the fifth, and Rule 2.1 is a whole rule with
//! its own allowlist, which makes it the natural seam.

use cena_arch_tests::harness::{relative, workspace_sources};
use cena_arch_tests::lexical::{collapse_whitespace, items};

// ---------------------------------------------------------------------------
// Rule 2.1 — Nothing above `cena-protocol` ever sees a raw byte or an
// unparsed string. (plan/05:270-274)
//
// "Enforced by: `cena-protocol` exposes no `String`-of-wire-text type in its
// public API; architecture test." (plan/05:273-274)
//
// This rule was DEFERRED until `cena-protocol` had a public API to name. It
// now does, so the deferral is spent and the test is written -- Rule 9.3
// (:515-517) asks for the test when the rule is adopted, and "the types exist
// now" was the stated unblocking condition.
//
// # Rule 2.2 mandates the one escape 2.1 forbids
//
// The deferral entry said to write the two as a cross-referenced pair, and
// that is what RAW_TEXT_ESCAPES is. Rule 2.2 (:276-283) REQUIRES an unmodelled
// tag to reach the user as text, which is by definition raw wire text in the
// public API. So the test cannot ban the shape outright; it bans every
// instance except the ones 2.2 compels, each named with its reason.
//
// # What this enforces, and what it does not
//
// It scans `cena-protocol`'s public items for fields whose NAME says they
// carry unparsed wire text (`raw`, `xml`, `markup`, `inner_xml`). It is a
// lexical scan over field names, so it cannot see that a field called
// `value` holds markup -- which is exactly Vellum's violation
// (`src/parser.rs:128-131`, `Component { id, value }` where `value` is the raw
// inner XML). That specific case is covered by a golden instead
// (`room_components_arrive_parsed_rather_than_as_markup`), which asserts a
// component body contains no `<`.
//
// So: this test catches a field that ANNOUNCES itself as raw, and the golden
// catches the one that does not. Neither is a proof; both name what they do.
// ---------------------------------------------------------------------------

/// A public field in `cena-protocol` that may carry raw wire text, and the
/// rule that compels it.
///
/// Two of the three are Rule 2.2's. The third, `Structural`, is the author's
/// drop-nothing rule, which is the same shape of obligation from a different
/// direction: 2.2 says an unmodelled tag must reach the user, drop-nothing
/// says an unremarkable one must too.
const RAW_TEXT_ESCAPES: &[(&str, &str)] = &[
    (
        "UnknownTag",
        "Rule 2.2 (:276-283) mandates it: an unmodelled tag reaches the user as text and a log. \
         The raw bytes ARE the diagnostic -- a reader has to see what the game actually sent.",
    ),
    (
        "MalformedTag",
        "A tag with no closing `>`. Same rule: it renders and is logged rather than being \
         smuggled into prose, which is the silent desync the reference has at \
         reference/VellumFE/src/parser.rs:733-736.",
    ),
    (
        "Structural",
        "Compelled by the author's drop-nothing rule of 2026-09-18 rather than by 2.2:          \"Cena shouldn't drop anything that comes in.\" Three arms in cena-protocol          returned without emitting -- close_tag's `_ if tags::is_known(name)` and          markup_tag's two `_ => {}` -- so 133 tag/form combinations produced NO frame and          were invisible to every consumer (measured by          crates/cena-protocol/tests/every_tag_is_observable.rs before the fix: 123 close          forms plus 10 self-closing). `raw` is the same diagnostic as UnknownTag's and for          the same reason -- without it a consumer knows a tag arrived but not which bytes,          and the rule is that nothing is unrecoverable. It is NOT a licence to stop          modelling: a tag that gains a real handler moves out of Structural, and the          enforcement test counts these so the move is visible in a diff.",
    ),
];

#[test]
fn wire_text_reaches_the_public_api_only_through_rule_2_2() {
    // Field DECLARATIONS, not construction sites: `raw: String` declares the
    // hole, while `raw: tag.to_owned()` merely fills one that already exists
    // and is already allowlisted. A first draft matched both and reported four
    // violations that were all just the two escapes being built.
    let needles = [
        "raw: String",
        "xml: String",
        "markup: String",
        "inner_xml: String",
        "raw_text: String",
        "raw: Vec<u8>",
        "bytes: Vec<u8>",
    ];
    let mut violations = Vec::new();

    for (path, text) in workspace_sources() {
        let rel = relative(&path);
        if !rel.starts_with("crates/cena-protocol/src/") {
            continue;
        }
        for item in items(&text) {
            if item.in_test_module {
                continue;
            }
            let code = collapse_whitespace(&item.code);
            if !needles.iter().any(|n| code.contains(n)) {
                continue;
            }
            // Allowed only inside a variant that Rule 2.2 compels.
            let excused = RAW_TEXT_ESCAPES
                .iter()
                .any(|(variant, _)| code.contains(variant));
            if !excused {
                violations.push(format!("{rel}:{}: {code}", item.line));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Nothing above cena-protocol ever sees a raw byte or an unparsed \
         string (plan/05 Rule 2.1, :270-274). A public field named `raw`, \
         `xml` or `markup` hands wire text to the layer above and makes it \
         re-parse.\n\n\
         The ONLY exceptions are the ones Rule 2.2 (:276-283) mandates -- \
         Frame::UnknownTag and Frame::MalformedTag -- because 2.2 requires an \
         unmodelled tag to reach the user as text. Those are listed in \
         RAW_TEXT_ESCAPES with their reasons. If a new escape is genuinely \
         compelled, add it there and say which rule compels it.\n\n\
         NOTE this scan reads field NAMES. A field called `value` that holds \
         markup is invisible to it -- that is Vellum's own violation at \
         src/parser.rs:128-131, and it is covered by the golden \
         `room_components_arrive_parsed_rather_than_as_markup` instead.\n{}",
        violations.join("\n")
    );
}

#[test]
fn every_raw_text_escape_names_the_rule_that_compels_it() {
    // A bare allowlist would let a future escape be added with no argument.
    // Rule 2.1 is only meaningful if each hole in it is justified in writing.
    for (variant, reason) in RAW_TEXT_ESCAPES {
        assert!(
            reason.len() > 80 && reason.contains(':'),
            "the RAW_TEXT_ESCAPES entry for {variant} must cite the rule that \
             compels it (plan/05 §-2), not {reason:?}"
        );
    }
}

// ---------------------------------------------------------------------------
// Rule 3.4 — Game-specific code lives under a game namespace. (plan/05:332-336)
//
// "Enforced by: architecture test bans game-name identifiers outside the game
// modules." (plan/05:336)
//
// This is the DragonRealms-deferral guard: CLAUDE.md and plan/12 §9d forbid a
// `GameAdapter` abstraction, so the only thing stopping `if game == ...` from
// spreading through shared modules is this test.
//
// The rule's words are "shared modules", not "cena-model". The first draft
// scanned `cena-model` only, which left the banned branch legal in the six
// other crates — including `cena-session`, where an M1 multi-session dispatch
// branch would most plausibly appear, and `cena-protocol`, where a GS-vs-DR
// wire divergence would. It now scans the workspace; the `/src/gemstone/` and
// `/src/dragonrealms/` exclusions already work workspace-wide because
// `relative()` emits full paths.
//
// `GS4` is a needle (the game is GemStone IV, CLAUDE.md:3). Bare `DR` is NOT:
// it would match `DRY`, `ADDR`, `DROP` and every hex literal.
//
// # THIS TEST FLAGS; IT DOES NOT PROVE. The name says `flags`, not `bans`.
//
// VERIFIED defeated by `concat!("Gem", "Stone")`, and equally by
// `"Gem\u{53}tone"`, `"GEMSTONE".to_lowercase()`, or a const assembled from
// bytes. No lexical scan can close that class: the set of expressions
// evaluating to "GemStone" is unbounded. Stripping `concat!` arguments would
// close one spelling and leave the rest, which is `plan/05` §-1's "simplest
// thing that works" applied to the wrong problem — the work does not reduce
// the residue.
//
// So this is a speed bump against **drift**, not a proof against intent, and
// the test is named for what it does. The real enforcement for a deliberate
// DragonRealms branch is review plus `plan/12` §9d; this catches the ordinary
// case where someone types the literal without thinking about it.
//
// FLAG: plan/05:335 justifies the rule with "that is what the `GameAdapter`
// trait is for", but CLAUDE.md bans `GameAdapter` and plan/05:533 itself lists
// it as over-engineered. The rule stands; its stated rationale is stale and
// should be amended per plan/05 §10.
//
// AMENDED 2026-09-19 — the rule forbade its own remedy. See
// `is_module_plumbing`. In short: `"gemstone"` is a needle, so the
// `mod gemstone;` and `use crate::gemstone::...` lines that reach the exempt
// directory were themselves flagged, and Rule 3.4 could not be followed at all.
// Found by `cena-platform/src/gemstone/endpoint.rs`, the first code to need the
// namespace. The exemption covers `mod`/`use` statements only.
//
// It also decides WHERE the namespace goes: the exclusion is the literal
// `/src/gemstone/`, so it must sit directly under a crate's `src`, exactly as
// plan/05:333 writes it (`cena-model/src/gemstone/`). A first attempt at
// `src/eaccess/gemstone/` -- beside its only caller -- was still flagged.
