//! `flare_patterns.rb`'s flares, family `flare_mirror`: the table loads whole,
//! every message it knows is read as a flare, and the flares it adds carry
//! their damage through the state machine.
//!
//! The counts are the extractor's:
//!
//! ```text
//! $ ruby crates/cena-model/tools/extract_flare_mirror.rb \
//!       reference/lich_repo_mirror/lib/flare_patterns.rb crates/cena-model/data
//! combat_flare_mirror.tsv: 277 rows, 458 message alternatives
//!   under a shipped flare's name: 75; new names: 202
//!   damaging: 148
//!   placeholders skipped: 17 (...)
//!   not ported, part of a shipped flare's proc: 6 (...)
//! ```
//!
//! Of the source's 465 alternatives, one is `Infusion_GS`'s non-damaging
//! pattern, which the two scripts that load the file never run: both merge
//! the two hashes, and the damaging hash's placeholder replaces it. Six are a
//! line of a proc a shipped flare already counts -- the spirit animal's
//! prefix, ensorcell's benefit -- found by the table reading Lich's own replay
//! fixtures (`the_replay_fixtures_gain_three_flares`, and
//! `tools/extract_flare_mirror.rb`'s `NOT_PORTED`).
//!
//! # The samples are built from the patterns
//!
//! Every alternative becomes one line: `.*?` filled with a word, `(s)?`
//! dropped, escapes undone. That line is what the pattern promises to match,
//! so the test is whether the CLASSIFIER reaches the row -- past the shipped
//! flares, past earlier mirror rows, past the attack-line rule -- and names
//! it. Lines for the state machine are written out, the creature as the bold
//! link the game sends.

mod fsm_harness;

use cena_model::state::chunks::ChunkLine;
use cena_model::state::combat::defs::{Def, defs};
use cena_model::{AttackLine, DamageLine, FlareLine, GameState};
use cena_protocol::Parser;
use fsm_harness::{bolded, dmg, flares, parse};

/// One wire line, through the parser, as the chunk would hold it.
fn line_of(wire: &str) -> ChunkLine {
    let mut parser = Parser::new();
    let mut state = GameState::default();
    for frame in parser.push_bytes(format!("{wire}\n").as_bytes()) {
        state.apply(&frame);
    }
    state
        .open_chunk()
        .lines()
        .first()
        .cloned()
        .unwrap_or_default()
}

/// A pattern's top-level alternatives. The table has no groups but `(s)?`, so
/// every unescaped `|` is top level.
fn alternatives(pattern: &str) -> Vec<String> {
    let mut out = vec![String::new()];
    let mut chars = pattern.chars();
    while let Some(c) = chars.next() {
        if c == '|' {
            out.push(String::new());
            continue;
        }
        let Some(cur) = out.last_mut() else {
            continue;
        };
        cur.push(c);
        if c == '\\'
            && let Some(n) = chars.next()
        {
            cur.push(n);
        }
    }
    out
}

/// A line the alternative matches. Only the syntax the table uses: a leading
/// `^`, `.*?`, `(s)?`, ` *?` (one row) and `\` escapes.
fn sample(alt: &str) -> String {
    let alt = alt.strip_prefix('^').unwrap_or(alt).replace("(s)?", "");
    let mut out = String::new();
    let mut chars = alt.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' => out.extend(chars.next()),
            '.' if chars.peek() == Some(&'*') => {
                chars.next();
                chars.next_if_eq(&'?');
                out.push_str("kobold");
            }
            '*' if chars.peek() == Some(&'?') => {
                chars.next();
                while out.ends_with(' ') {
                    out.pop();
                }
            }
            _ => out.push(c),
        }
    }
    out
}

fn mirror() -> &'static [Def] {
    defs().family("flare_mirror")
}

/// The rows whose sample the shipped attack grammar reads, and as what.
const ATTACK_LINES: &[(&str, &str)] = &[
    ("Wither_LoreBenefit", "wither"),
    ("FAura1706", "flaming_aura"),
];

/// The table is `flare_patterns.rb` whole: every row compiled, every message.
#[test]
fn the_table_loads_whole() {
    let rows = mirror();
    assert_eq!(rows.len(), 277, "the extractor wrote 277 rows");
    let alts: usize = rows.iter().map(|d| alternatives(&d.pattern).len()).sum();
    assert_eq!(
        alts, 458,
        "465 alternatives less Infusion_GS's unreachable one and six not ported"
    );
    for d in rows {
        assert!(d.regex.is_some(), "{} did not compile", d.name);
        assert!(d.extra("key").is_some(), "{}: no source key", d.name);
        assert!(!d.pattern.contains("place holder"), "{}", d.name);
    }
    assert_eq!(
        rows.iter().filter(|d| d.flag("damaging")).count(),
        148,
        "the damaging hash's keys"
    );
}

/// **A shipped name keeps its flags** (`supplements.rb:778-798`): a custom
/// acid text must not make `acid` a buff. A new name has the supplement
/// default, not single-target or spawning.
#[test]
fn a_reused_name_keeps_the_shipped_flags() {
    let shipped = defs().family("flare");
    let mut reused = 0;
    for d in mirror() {
        let flags = |x: &Def| ["damaging", "aoe", "spawns"].map(|k| x.flag(k));
        match shipped.iter().find(|s| s.name == d.name) {
            Some(s) => {
                reused += 1;
                assert_eq!(flags(d), flags(s), "{} changed a shipped flag", d.name);
            }
            None => assert!(!d.flag("aoe") && !d.flag("spawns"), "{}", d.name),
        }
    }
    assert_eq!(reused, 75);
    // the two that matter most to the state machine
    let named = |key: &str| mirror().iter().find(|d| d.extra("key") == Some(key));
    assert!(named("Blink").is_some_and(|d| d.name == "blink" && d.flag("spawns")));
    assert!(named("MirrorImage_GS").is_some_and(|d| d.name == "mirror_image"));
}

/// **Every message the table knows is read as its flare** -- all 458, less
/// the two lines Lich's attack grammar owns.
///
/// Also the gap, measured by the classifier rather than by pattern text:
/// how many of the samples the shipped flares alone read, and how many are
/// read now.
#[test]
fn every_alternative_is_read_as_its_flare() {
    let (mut total, mut shipped, mut read) = (0, 0, 0);
    let mut attack_lines = Vec::new();
    for d in mirror() {
        let key = d.extra("key").unwrap_or_default();
        for alt in alternatives(&d.pattern) {
            total += 1;
            let line = line_of(&sample(&alt));
            shipped += usize::from(defs().first_match("flare", &line.text()).is_some());
            let got = FlareLine::classify(&line).map(|f| f.name);
            if let Some(attack) = AttackLine::classify(&line) {
                assert_eq!(got, None, "{key}: an attack line read as a flare");
                attack_lines.push((key, attack.name));
                continue;
            }
            assert_eq!(got.as_deref(), Some(d.name.as_str()), "{key}: {alt}");
            read += 1;
        }
    }
    println!("{total} alternatives: the shipped flares read {shipped}, now {read}");
    assert_eq!(total, 458);
    assert_eq!(read, 456);
    let expected: Vec<(&str, String)> = ATTACK_LINES
        .iter()
        .map(|(k, a)| (*k, (*a).to_owned()))
        .collect();
    assert_eq!(attack_lines, expected);
}

/// The measured collisions, whole: a row matches, and the rule, not the
/// table, keeps the line what the shipped grammar says it is.
#[test]
fn a_mirror_row_never_reads_an_attack_or_a_damage_line() {
    let orc = bolded(4242, "orc", "a greater orc");
    let wither = line_of(&format!(
        "A nebulous haze shimmers into view around {orc}, plunging inward in a dizzying spiral to envelop {orc} completely."
    ));
    assert_eq!(
        AttackLine::classify(&wither).map(|a| a.name).as_deref(),
        Some("wither")
    );
    assert!(defs().first_match("flare_mirror", &wither.text()).is_some());
    assert_eq!(FlareLine::classify(&wither), None);

    // `damage.rb:41`'s bleed tick, which Sanguine Sacrifice's row also reads
    let tick = line_of(&format!("{orc} suffers an additional 3 damage!"));
    assert_eq!(DamageLine::classify(&tick).map(|d| d.amount), Some(3));
    let text = tick.text();
    let row = defs().first_match("flare_mirror", &text);
    assert_eq!(row.map(|(d, _)| d.name.as_str()), Some("sanguinesacrifice"));
    assert_eq!(FlareLine::classify(&tick), None);
}

/// **The shipped flare wins a line both tables read**, with its own flags:
/// the mirror's broader `Terror_Flare` and `Death_Flare` would rename them.
#[test]
fn the_shipped_flare_wins_a_line_both_tables_read() {
    for (wire, shipped) in [
        (
            "** Your longsword releases a distorted black shadow! **",
            "terror_weapon",
        ),
        (
            "** Your longsword emits an ominous black-green glow! **",
            "xazkruvrixis",
        ),
    ] {
        let line = line_of(wire);
        assert!(defs().first_match("flare_mirror", &line.text()).is_some());
        let f = FlareLine::classify(&line).map(|f| f.name);
        assert_eq!(f.as_deref(), Some(shipped), "{wire}");
    }
}

/// **Weapon flares in either verb number** (`inventory/12` pile 2 §8): the
/// shipped magma flare is plural with a target, the shipped fire flare
/// singular.
#[test]
fn a_weapon_flare_is_read_in_either_verb_number() {
    for wire in [
        "** Your spiked gauntlets expel a glob of molten magma! **",
        "** Your longsword expels a glob of molten magma! **",
        "** Your longsword expels a glob of molten magma at the orc! **",
    ] {
        let f = FlareLine::classify(&line_of(wire));
        assert_eq!(f.as_ref().map(|f| f.name.as_str()), Some("magma"), "{wire}");
        assert!(f.is_some_and(|f| f.damaging && f.is_ours()), "{wire}");
    }
    for wire in [
        "** Your spiked gauntlets flare with a burst of flame! **",
        "** Your longsword flares with a burst of flame! **",
    ] {
        let f = FlareLine::classify(&line_of(wire)).map(|f| f.name);
        assert_eq!(f.as_deref(), Some("fire"), "{wire}");
    }
}

/// **One ensorcell proc is one flare.** The benefit line after the shipped
/// `ensorcell` line is the same proc (`attack.txt`, and three more replay
/// fixtures), so the table's `Ensorcell_*` rows are not ported.
#[test]
fn an_ensorcell_proc_is_one_flare() {
    let orc = bolded(4242, "orc", "a greater orc");
    let f = parse(&[
        &format!("You swing a mithril war-hammer at {orc}!"),
        "  AS: +400 vs DS: +200 with AvD: +30 + d100 roll: +50 = +280",
        "   ... and hits for 30 points of damage!",
        " ** Necrotic energy from your mithril war-hammer overflows into you! **",
        "   You feel energized!",
    ]);
    assert_eq!(f.events.len(), 1);
    assert_eq!(flares(&f.events[0]), ["ensorcell"]);
    for benefit in ["healed", "empowered", "rejuvenated", "reinvigorated"] {
        let line = line_of(&format!("You feel {benefit}!"));
        assert_eq!(FlareLine::classify(&line), None, "{benefit}");
    }
}

/// **A custom flare text owns its damage.** Before this table the second
/// damage line had no flare to land on and was added to the swing's.
#[test]
fn a_custom_flare_owns_the_damage_after_it() {
    let orc = bolded(4242, "orc", "a greater orc");
    let f = parse(&[
        &format!("You swing a slim short sword at {orc}!"),
        "  AS: +400 vs DS: +200 with AvD: +30 + d100 roll: +50 = +280",
        "   ... and hits for 30 points of damage!",
        &format!(
            "Hundreds of razor-sharp daggers discharge from your slim short sword in a virulent barrage to mutilate {orc}!"
        ),
        "   ... 12 points of damage!",
    ]);
    assert_eq!(f.events.len(), 1);
    let e = &f.events[0];
    assert_eq!(dmg(e), [30], "the swing keeps its own damage only");
    assert_eq!(flares(e), ["fatalafflares"], "the key, lowercased");
    let flare = &e.flares[0];
    let hits: Vec<u32> = flare.hits.iter().map(|h| h.damage).collect();
    assert_eq!(hits, [12]);
    assert_eq!(flare.target.as_ref().and_then(|a| a.id), Some(4242));
}

/// **A custom text for a shipped flare is that flare**, and a purified
/// metal's two flares are two, each with its damage.
#[test]
fn a_custom_acid_text_and_a_purified_metal_are_named() {
    let orc = bolded(4242, "orc", "a greater orc");
    let f = parse(&[
        &format!("You swing a drakar longsword at {orc}!"),
        "  AS: +400 vs DS: +200 with AvD: +30 + d100 roll: +50 = +280",
        "   ... and hits for 30 points of damage!",
        &format!(
            "Luminescent green droplets dribble from your drakar longsword and puddle mid-air, creating a large globule of sizzling liquid that bursts over {orc}!"
        ),
        "   ... 8 points of damage!",
        &format!(
            "** A scorching blast of golden fire blazes forth from your drakar longsword, bathing {orc} in flame! **"
        ),
        "   ... 15 points of damage!",
        &format!(
            "** Sparks swirl about {orc}, spiraling inward to ignite the air in a roaring firestorm! **"
        ),
        "   ... 9 points of damage!",
    ]);
    assert_eq!(f.events.len(), 1);
    let e = &f.events[0];
    assert_eq!(dmg(e), [30]);
    assert_eq!(flares(e), ["acid", "pure_drakar", "pure_drakar_2ndflare"]);
    let hits: Vec<Vec<u32>> = e
        .flares
        .iter()
        .map(|x| x.hits.iter().map(|h| h.damage).collect())
        .collect();
    assert_eq!(hits, [vec![8], vec![15], vec![9]]);
}

/// A Covert Arts poison without the `**` the shipped pattern wants is the
/// shipped `weapon_poison`, and ours.
#[test]
fn a_covert_arts_poison_is_the_shipped_weapon_poison() {
    let orc = bolded(4242, "orc", "a greater orc");
    let f = FlareLine::classify(&line_of(&format!(
        "Afflicted by your dagger, {orc} reels as the crimson-swirled poison does its work!"
    )));
    assert_eq!(f.as_ref().map(|f| f.name.as_str()), Some("weapon_poison"));
    assert!(f.is_some_and(|f| f.is_ours() && !f.damaging));
}

/// **A proc the creature's attack sets off stays on that attack.** Bark
/// absorbing the orc's blow once resumed our interrupted swing, and the orc's
/// 28 damage to us was recorded as our swing's second hit on the orc.
#[test]
fn a_defensive_proc_leaves_the_creatures_damage_on_its_attack() {
    let orc = bolded(4242, "orc", "a greater orc");
    let f = parse(&[
        &format!("You swing a slim short sword at {orc}!"),
        "  AS: +400 vs DS: +200 with AvD: +30 + d100 roll: +50 = +280",
        "   ... and hits for 30 points of damage!",
        &format!("{orc} swings a cudgel at you!"),
        "  AS: +176 vs DS: +76 with AvD: +20 + d100 roll: +64 = +184",
        "The layer of bark on you hardens and absorbs some of the damage.",
        "   ... and hits for 28 points of damage!",
    ]);
    let ours: Vec<_> = f.events.iter().filter(|e| !e.inbound).collect();
    let theirs: Vec<_> = f.events.iter().filter(|e| e.inbound).collect();
    assert_eq!((ours.len(), theirs.len()), (1, 1));
    assert_eq!(dmg(ours[0]), [30]);
    assert!(ours[0].flares.is_empty());
    assert_eq!(dmg(theirs[0]), [28]);
    assert_eq!(flares(theirs[0]), ["barkskin605"]);
}

/// **On Lich's own replay fixtures the table reads three more lines**, and
/// which: a luck talisman on a shot, a spirit ward on a creature's tackle, a
/// minor fire tick. It read nine before `NOT_PORTED` -- the other six were the
/// spirit animal's prefix and ensorcell's benefit, each one proc already
/// counted by a shipped flare on the next or previous line.
#[test]
fn the_replay_fixtures_gain_three_flares() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/combat");
    let mut paths: Vec<_> = std::fs::read_dir(&dir)
        .map(|rd| rd.filter_map(Result::ok).map(|e| e.path()).collect())
        .unwrap_or_default();
    paths.sort();
    assert!(
        paths.len() > 60,
        "the fixtures are where the other tests find them"
    );
    let mut read = Vec::new();
    for path in paths {
        let file = path
            .file_name()
            .map(|f| f.to_string_lossy().into_owned())
            .unwrap_or_default();
        let text = std::fs::read_to_string(&path).unwrap_or_default();
        let mut wire = String::new();
        for l in text.lines() {
            if !l.starts_with('#') && !l.trim().is_empty() {
                wire.push_str(l);
                wire.push('\n');
            }
        }
        let mut parser = Parser::new();
        let mut state = GameState::default();
        for frame in parser.push_bytes(wire.as_bytes()) {
            state.apply(&frame);
        }
        for line in state.open_chunk().lines() {
            if let Some(f) = FlareLine::classify(line).filter(|f| !f.shipped) {
                read.push(format!("{file} {}", f.name));
            }
        }
    }
    assert_eq!(
        read,
        [
            "fire.txt lucktalisman_offensive",
            "tackle.txt sward319",
            "wand.txt minorfire906_burn",
        ]
    );
}
