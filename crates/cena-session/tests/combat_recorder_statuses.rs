//! The combat recorder: the status stream, the stun pair, and kill credit.
//!
//! Ported from `recorder_spec.rb` (*status stream + attack-window
//! attribution*, *crit-status*, *ucs*, *room-feed death*),
//! `recorder_schema_spec.rb` (*kill credit*, *stun pair*, *stun
//! re-application*) and `recorder_combat_order_spec.rb`, whose chunks are
//! real feed text and go through the real parser and state machine here too.

mod recorder_kit;

use cena_model::state::combat::event::{Crit, Subject, UcsKind};
use cena_model::state::combat::{AttackEvent, ChunkFacts, Fact};
use cena_model::{Actor, GameState, PositionTier, StatusAction, StatusName, UcsAttack};
use cena_session::combat_recorder::CombatRecorder;
use recorder_kit::{attack, chunk, column, count, dead, facts, feed, lizard, one, status};

fn explicit() -> rusqlite::Result<CombatRecorder> {
    let mut rec = CombatRecorder::in_memory(Some("Tester"), "spec", None)?;
    rec.start_session(1_000_000.0)?;
    Ok(rec)
}

fn auto() -> rusqlite::Result<CombatRecorder> {
    CombatRecorder::in_memory(Some("Tester"), "live", Some(60.0))
}

fn named(name: &str, target: &Actor) -> AttackEvent {
    let mut e = attack(target, 10);
    name.clone_into(&mut e.name);
    e
}

fn rolton() -> Actor {
    Actor {
        id: Some(102),
        noun: Some("rolton".to_owned()),
        name: "a rolton".to_owned(),
    }
}

fn stun(who: &Actor, rounds: u16, event: usize) -> Fact {
    Fact::Stun {
        creature: who.clone(),
        rounds,
        event,
        flare_seq: None,
    }
}

/// `(kind, action, value, attack name)` of one stunned row.
type StunRow = (String, String, Option<i64>, Option<String>);

/// Every stunned row, in row order.
fn stunned_rows(rec: &CombatRecorder) -> rusqlite::Result<Vec<StunRow>> {
    let mut stmt = rec.connection().prepare(
        "SELECT s.kind, s.action, s.value, a.name FROM statuses s \
         LEFT JOIN attacks a ON a.id = s.attack_id WHERE s.status = 'stunned' ORDER BY s.id",
    )?;
    stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))?
        .collect()
}

fn row(kind: &str, action: &str, value: Option<i64>, atk: &str) -> StunRow {
    (
        kind.to_owned(),
        action.to_owned(),
        value,
        Some(atk.to_owned()),
    )
}

mod attribution {
    use super::*;

    #[test]
    fn a_status_naming_a_touched_creature_falls_in_the_open_attacks_window() {
        let mut rec = explicit().expect("opens");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 1.0)
            .expect("records");
        // a later chunk, no event named: the window
        rec.record_chunk(
            &facts(vec![status(
                &lizard(),
                StatusName::Prone,
                StatusAction::Add,
            )]),
            2.0,
        )
        .expect("records");
        let (attack_id, source, subject): (i64, String, String) = rec
            .connection()
            .query_row("SELECT attack_id, source, subject FROM statuses", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .expect("q");
        assert_eq!(
            (attack_id, source.as_str(), subject.as_str()),
            (1, "window", "a cave lizard")
        );
    }

    #[test]
    fn the_event_the_state_machine_names_outranks_the_last_attack_recorded() {
        let mut rec = explicit().expect("opens");
        // our swing, then a creature's cast that interrupted it: the swing's
        // blind must not fall off the swing (hunt log 2026-09-07 23:50)
        let mut interrupt = named("cast", &lizard());
        interrupt.inbound = true;
        let mut c = chunk(vec![named("swing", &lizard()), interrupt]);
        c.facts.push(Fact::Status {
            subject: Subject::Creature(lizard()),
            status: StatusName::Blind,
            action: StatusAction::Add,
            event: Some(0),
            flare_seq: None,
            line: 1,
        });
        rec.record_chunk(&c, 1.0).expect("records");
        let (name, source): (String, String) = rec
            .connection()
            .query_row(
                "SELECT a.name, s.source FROM statuses s JOIN attacks a ON a.id = s.attack_id",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("q");
        assert_eq!((name.as_str(), source.as_str()), ("swing", "event"));
    }

    #[test]
    fn a_status_on_us_falls_in_an_inbound_attacks_window_only() {
        let mut rec = explicit().expect("opens");
        let us = |status| Fact::Status {
            subject: Subject::Us,
            status,
            action: StatusAction::Add,
            event: None,
            flare_seq: None,
            line: 0,
        };
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 1.0)
            .expect("records");
        rec.record_chunk(&facts(vec![us(StatusName::Stunned)]), 1.0)
            .expect("records");
        let mut inbound = attack(&lizard(), 0);
        inbound.inbound = true;
        rec.record_chunk(&chunk(vec![inbound]), 2.0)
            .expect("records");
        rec.record_chunk(&facts(vec![us(StatusName::Prone)]), 2.0)
            .expect("records");
        let rows: Vec<(String, Option<i64>, String)> = {
            let mut stmt = rec
                .connection()
                .prepare("SELECT subject, attack_id, source FROM statuses ORDER BY id")
                .expect("q");
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .expect("q")
                .collect::<Result<_, _>>()
                .expect("q")
        };
        assert_eq!(
            rows,
            [
                ("self".to_owned(), None, "direct".to_owned()),
                ("self".to_owned(), Some(2), "window".to_owned())
            ]
        );
        assert_eq!(
            count(&rec, "creatures").expect("q"),
            1,
            "we are not a creature"
        );
    }

    #[test]
    fn a_status_with_no_matching_attack_is_direct() {
        let mut rec = explicit().expect("opens");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 1.0)
            .expect("records");
        rec.record_chunk(
            &facts(vec![status(
                &rolton(),
                StatusName::Prone,
                StatusAction::Add,
            )]),
            2.0,
        )
        .expect("records");
        let (attack_id, source): (Option<i64>, String) = rec
            .connection()
            .query_row("SELECT attack_id, source FROM statuses", [], |r| {
                Ok((r.get(0)?, r.get(1)?))
            })
            .expect("q");
        assert_eq!((attack_id, source.as_str()), (None, "direct"));
    }

    #[test]
    fn a_status_first_creatures_noun_is_backfilled_by_a_later_attack() {
        let mut rec = explicit().expect("opens");
        let unnamed = Actor {
            noun: None,
            ..lizard()
        };
        rec.record_chunk(
            &facts(vec![status(&unnamed, StatusName::Prone, StatusAction::Add)]),
            1.0,
        )
        .expect("records");
        assert_eq!(
            one::<Option<String>>(&rec, "SELECT noun FROM creatures").expect("q"),
            None,
            "guard"
        );
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 2.0)
            .expect("records");
        assert_eq!(count(&rec, "creatures").expect("q"), 1);
        assert_eq!(
            one::<Option<String>>(&rec, "SELECT noun FROM creatures").expect("q"),
            Some("lizard".to_owned())
        );
    }
}

mod payloads {
    use super::*;

    #[test]
    fn stun_and_roundtime_keep_their_numbers() {
        let mut rec = explicit().expect("opens");
        let mut c = chunk(vec![attack(&lizard(), 40)]);
        c.facts.push(stun(&lizard(), 3, 0));
        c.facts.push(Fact::Roundtime {
            creature: lizard(),
            seconds: 7,
            event: 0,
            flare_seq: None,
        });
        rec.record_chunk(&c, 1.0).expect("records");
        let rows: Vec<(String, String, i64)> = {
            let mut stmt = rec
                .connection()
                .prepare("SELECT kind, status, value FROM statuses ORDER BY id")
                .expect("q");
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .expect("q")
                .collect::<Result<_, _>>()
                .expect("q")
        };
        assert_eq!(
            rows,
            [
                ("stun".to_owned(), "stunned".to_owned(), 3),
                ("roundtime".to_owned(), "roundtime".to_owned(), 7)
            ]
        );
    }

    /// `(status, value, spell_name, action)` of one `ucs` row.
    type UcsRow = (String, Option<i64>, Option<String>, Option<String>);

    #[test]
    fn ucs_tiers_are_their_ordinal_and_a_tierup_keeps_its_followup() {
        let mut rec = explicit().expect("opens");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 1.0)
            .expect("records");
        let ucs = |kind| Fact::Ucs {
            creature: lizard(),
            kind,
        };
        rec.record_chunk(
            &facts(vec![
                ucs(UcsKind::Position(PositionTier::Excellent)),
                ucs(UcsKind::PositionInbound(PositionTier::Decent)),
                ucs(UcsKind::Tierup(UcsAttack::Kick)),
                ucs(UcsKind::SmiteOn),
            ]),
            1.0,
        )
        .expect("records");
        let rows: Vec<UcsRow> = {
            let mut stmt = rec
                .connection()
                .prepare("SELECT status, value, spell_name, action FROM statuses WHERE kind = 'ucs' ORDER BY id")
                .expect("q");
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                .expect("q")
                .collect::<Result<_, _>>()
                .expect("q")
        };
        assert_eq!(
            rows,
            [
                ("position".to_owned(), Some(3), None, None),
                ("position_inbound".to_owned(), Some(1), None, None),
                ("tierup".to_owned(), None, Some("kick".to_owned()), None),
                ("smite_on".to_owned(), None, None, None),
            ]
        );
    }

    #[test]
    fn a_spell_loss_carries_its_spell_and_cause() {
        let mut rec = explicit().expect("opens");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 1.0)
            .expect("records");
        rec.record_chunk(
            &facts(vec![Fact::SpellLoss {
                subject: lizard(),
                spell: Some(1712),
                spell_name: "Cloak of Shadows".to_owned(),
                cause: Some(cena_model::state::combat::event::LossCause::Dispel),
            }]),
            1.0,
        )
        .expect("records");
        let got: (String, String, i64, String, String) = rec
            .connection()
            .query_row(
                "SELECT kind, action, spell, spell_name, cause FROM statuses",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?)),
            )
            .expect("q");
        assert_eq!(
            got,
            (
                "spell_loss".to_owned(),
                "remove".to_owned(),
                1712,
                "Cloak of Shadows".to_owned(),
                "dispel".to_owned()
            )
        );
    }
}

mod stun_pair {
    use super::*;

    #[test]
    fn the_table_stun_and_the_message_merge_into_one_row_in_either_order() {
        let mut rec = explicit().expect("opens");
        let mut first = chunk(vec![attack(&lizard(), 40)]);
        first.facts = vec![
            status(&lizard(), StatusName::Stunned, StatusAction::Add),
            stun(&lizard(), 3, 0),
        ];
        let mut second = chunk(vec![attack(&rolton(), 40)]);
        second.facts = vec![
            stun(&rolton(), 2, 0),
            status(&rolton(), StatusName::Stunned, StatusAction::Add),
        ];
        rec.record_chunk(&first, 1.0).expect("records");
        rec.record_chunk(&second, 1.0).expect("records");
        let rows: Vec<(i64, String, i64)> = {
            let mut stmt = rec
                .connection()
                .prepare(
                    "SELECT c.exist_id, s.kind, s.value FROM statuses s \
                     JOIN creatures c ON c.id = s.creature_id WHERE s.status = 'stunned' ORDER BY s.id",
                )
                .expect("q");
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .expect("q")
                .collect::<Result<_, _>>()
                .expect("q")
        };
        assert_eq!(
            rows,
            [(101, "stun".to_owned(), 3), (102, "stun".to_owned(), 2)]
        );
    }

    #[test]
    fn a_second_swings_stun_is_not_folded_into_a_pair_that_already_merged() {
        let mut rec = explicit().expect("opens");
        let mut first = chunk(vec![named("first", &lizard())]);
        first.facts = vec![
            stun(&lizard(), 3, 0),
            status(&lizard(), StatusName::Stunned, StatusAction::Add),
            status(&lizard(), StatusName::Stunned, StatusAction::Remove),
        ];
        let mut second = chunk(vec![named("second", &lizard())]);
        second.facts = vec![status(&lizard(), StatusName::Stunned, StatusAction::Add)];
        // all inside the two-second pair window
        rec.record_chunk(&first, 1.0).expect("records");
        rec.record_chunk(&second, 1.0).expect("records");
        assert_eq!(
            stunned_rows(&rec).expect("q"),
            [
                row("stun", "add", Some(3), "first"),
                row("status", "remove", None, "first"),
                row("status", "add", None, "second"),
            ]
        );
    }

    #[test]
    fn a_pair_a_removal_ended_takes_no_later_stun() {
        let mut rec = explicit().expect("opens");
        let mut first = chunk(vec![named("first", &lizard())]);
        first.facts = vec![
            status(&lizard(), StatusName::Stunned, StatusAction::Add),
            stun(&lizard(), 3, 0),
            status(&lizard(), StatusName::Stunned, StatusAction::Remove),
        ];
        let mut second = chunk(vec![named("second", &lizard())]);
        second.facts = vec![stun(&lizard(), 5, 0)];
        rec.record_chunk(&first, 1.0).expect("records");
        rec.record_chunk(&second, 1.0).expect("records");
        assert_eq!(
            stunned_rows(&rec).expect("q"),
            [
                row("stun", "add", Some(3), "first"),
                row("status", "remove", None, "first"),
                row("stun", "add", Some(5), "second"),
            ]
        );
    }
}

/// The two guards overlap whenever an attack is known, so each is pinned
/// where only it can act: facts that ride no attack at all.
mod stun_pair_without_an_attack {
    use super::*;

    fn bare_stun_add() -> ChunkFacts {
        facts(vec![status(
            &rolton(),
            StatusName::Stunned,
            StatusAction::Add,
        )])
    }

    /// `(kind, action, value)` of every stunned row.
    fn rows(rec: &CombatRecorder) -> Vec<(String, String, Option<i64>)> {
        stunned_rows(rec)
            .unwrap_or_default()
            .into_iter()
            .map(|(kind, action, value, _)| (kind, action, value))
            .collect()
    }

    #[test]
    fn a_row_that_absorbed_its_twin_takes_no_second_one() {
        let mut rec = explicit().expect("opens");
        // the lizard's attack: the rolton's facts fall in no window
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 1.0)
            .expect("records");
        rec.record_chunk(&bare_stun_add(), 1.0).expect("records");
        let mut swing = chunk(vec![attack(&rolton(), 40)]);
        swing.facts = vec![stun(&rolton(), 3, 0)];
        rec.record_chunk(&swing, 1.0).expect("records");
        // a NEW application, no removal between, still inside two seconds
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 1.0)
            .expect("records");
        rec.record_chunk(&bare_stun_add(), 1.0).expect("records");
        assert_eq!(
            rows(&rec),
            [
                ("stun".to_owned(), "add".to_owned(), Some(3)),
                ("status".to_owned(), "add".to_owned(), None)
            ]
        );
    }

    #[test]
    fn a_removal_ends_a_pair_that_never_merged() {
        let mut rec = explicit().expect("opens");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 1.0)
            .expect("records");
        rec.record_chunk(&bare_stun_add(), 1.0).expect("records");
        rec.record_chunk(
            &facts(vec![status(
                &rolton(),
                StatusName::Stunned,
                StatusAction::Remove,
            )]),
            1.0,
        )
        .expect("records");
        let mut swing = chunk(vec![attack(&rolton(), 40)]);
        swing.facts = vec![stun(&rolton(), 5, 0)];
        rec.record_chunk(&swing, 1.0).expect("records");
        assert_eq!(
            rows(&rec),
            [
                ("status".to_owned(), "add".to_owned(), None),
                ("status".to_owned(), "remove".to_owned(), None),
                ("stun".to_owned(), "add".to_owned(), Some(5))
            ]
        );
    }
}

mod kill_credit {
    use super::*;

    /// `(killed_at, killed_by_attack_id, kill_credit)`; times are whole seconds.
    type Credit = (Option<i64>, Option<i64>, Option<String>);

    fn credit(rec: &CombatRecorder) -> rusqlite::Result<Credit> {
        rec.connection().query_row(
            "SELECT CAST(killed_at AS INTEGER), killed_by_attack_id, kill_credit FROM creatures",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
    }

    fn fatal(target: &Actor) -> AttackEvent {
        let mut e = attack(target, 60);
        e.hits[0].crit = Some(Crit::coup_de_grace(cena_model::crit::Location::Neck, 1));
        e
    }

    #[test]
    fn a_fatal_crit_is_ground_truth() {
        let mut rec = explicit().expect("opens");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 1.0)
            .expect("records");
        assert_eq!(credit(&rec).expect("q"), (None, None, None), "guard");
        rec.record_chunk(&chunk(vec![fatal(&lizard())]), 5.0)
            .expect("records");
        assert_eq!(
            credit(&rec).expect("q"),
            (Some(5), Some(2), Some("crit".to_owned()))
        );
        assert_eq!(
            one::<String>(&rec, "SELECT crit_type FROM hits WHERE fatal = 1").expect("q"),
            "coup_de_grace"
        );
    }

    #[test]
    fn a_room_feed_death_inside_the_window_is_window() {
        let mut rec = explicit().expect("opens");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 1.0)
            .expect("records");
        rec.record_chunk(&facts(vec![dead(&lizard())]), 4.0)
            .expect("records");
        assert_eq!(
            credit(&rec).expect("q"),
            (Some(4), Some(1), Some("window".to_owned()))
        );
    }

    #[test]
    fn a_fatal_crit_takes_the_credit_from_an_earlier_room_feed_stamp_but_not_the_time() {
        let mut rec = explicit().expect("opens");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 1.0)
            .expect("records");
        rec.record_chunk(&facts(vec![dead(&lizard())]), 4.0)
            .expect("records");
        rec.record_chunk(&chunk(vec![fatal(&lizard())]), 9.0)
            .expect("records");
        assert_eq!(
            credit(&rec).expect("q"),
            (Some(4), Some(2), Some("crit".to_owned()))
        );
    }

    #[test]
    fn a_room_feed_death_never_overwrites_a_fatal_crits_credit() {
        let mut rec = explicit().expect("opens");
        rec.record_chunk(&chunk(vec![fatal(&lizard())]), 1.0)
            .expect("records");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 5)]), 2.0)
            .expect("records");
        rec.record_chunk(&facts(vec![dead(&lizard())]), 3.0)
            .expect("records");
        assert_eq!(
            credit(&rec).expect("q"),
            (Some(1), Some(1), Some("crit".to_owned()))
        );
    }

    #[test]
    fn with_no_window_the_last_damaging_attack_is_credited_and_says_whose() {
        let mut rec = auto().expect("opens");
        let mut theirs = named("theirs", &lizard());
        theirs.foreign_caster = true;
        rec.record_chunk(&chunk(vec![named("mine", &lizard())]), 1_000.0)
            .expect("records");
        rec.finish_session(1_100.0).expect("finishes");
        rec.record_chunk(&facts(vec![dead(&lizard())]), 1_100.0)
            .expect("records");
        assert_eq!(credit(&rec).expect("q").2.as_deref(), Some("last_own_hit"));

        // another player's LATER hit outranks our earlier one
        let mut rec = auto().expect("opens");
        rec.record_chunk(&chunk(vec![named("mine", &lizard())]), 1_000.0)
            .expect("records");
        rec.record_chunk(&chunk(vec![theirs]), 1_001.0)
            .expect("records");
        rec.finish_session(1_100.0).expect("finishes");
        rec.record_chunk(&facts(vec![dead(&lizard())]), 1_100.0)
            .expect("records");
        assert_eq!(
            credit(&rec).expect("q"),
            (Some(1_100), Some(2), Some("last_hit".to_owned()))
        );
    }

    #[test]
    fn two_chunks_in_one_second_order_by_chunk_not_by_line() {
        let mut rec = auto().expect("opens");
        let mut mine = named("mine", &lizard());
        mine.hits[0].line = 30; // late in the earlier chunk
        let mut theirs = named("theirs", &lizard());
        theirs.foreign_caster = true;
        theirs.hits[0].line = 2; // early in the later chunk
        rec.record_chunk(&chunk(vec![mine]), 1_000_000.0)
            .expect("records");
        rec.record_chunk(&chunk(vec![theirs]), 1_000_000.0)
            .expect("records");
        rec.finish_session(1_000_100.0).expect("finishes");
        rec.record_chunk(&facts(vec![dead(&lizard())]), 1_000_100.0)
            .expect("records");
        assert_eq!(credit(&rec).expect("q").1, Some(2));
        assert_eq!(credit(&rec).expect("q").2.as_deref(), Some("last_hit"));
    }
}

/// `recorder_combat_order_spec.rb`: a released spell is emitted AFTER the
/// swing that released it, for lineage, so row order is not combat order.
mod combat_order {
    use super::*;

    const TROLL: &str = r#"<pushBold/>a <a exist="212657781" noun="troll">bog troll</a><popBold/>"#;
    const MACE: &str = r#"<a exist="212333157" noun="mace">mithril mace</a>"#;

    fn release() -> String {
        format!(
            "As you attempt to strike with your {MACE}, it sends a surge of power through you that quickly leaps out at {TROLL}!"
        )
    }
    fn verdict() -> Vec<String> {
        vec![
            format!("Violet flames erupt from beneath {TROLL}."),
            "  CS: +149 - TD: +120 + CvA: +17 + d100: +85 == +131".to_owned(),
            "  Warding failed!".to_owned(),
            format!("A column of seething violet flame envelops {TROLL} in its searing embrace!"),
            "   ... 20 points of damage!".to_owned(),
        ]
    }
    fn swing_roll() -> Vec<String> {
        vec![
            "  AS: +273 vs DS: +80 with AvD: +35 + d100 roll: +2 = +230".to_owned(),
            "   ... and hit for 79 points of damage!".to_owned(),
        ]
    }
    fn run(lines: &[String]) -> ChunkFacts {
        let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        feed(&mut GameState::default(), &refs)
    }
    fn names(f: &ChunkFacts) -> Vec<&str> {
        f.events.iter().map(|e| e.name.as_str()).collect()
    }
    fn killer(rec: &CombatRecorder) -> rusqlite::Result<(String, String)> {
        rec.connection().query_row(
            "SELECT a.name, c.kill_credit FROM creatures c \
             JOIN attacks a ON a.id = c.killed_by_attack_id",
            [],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
    }
    fn troll() -> Actor {
        Actor {
            id: Some(212_657_781),
            noun: None,
            name: "bog troll".to_owned(),
        }
    }

    #[test]
    fn credits_the_swing_when_the_spell_it_released_landed_first() {
        let mut lines = vec![release()];
        lines.extend(verdict());
        lines.push(format!("You swing a perfect {MACE} at {TROLL}!"));
        lines.extend(swing_roll());
        let chunk = run(&lines);
        assert_eq!(
            names(&chunk),
            ["attack", "templars_verdict"],
            "guard: cast re-emitted after its swing"
        );
        let mut rec = auto().expect("opens");
        rec.record_chunk(&chunk, 1_000_000.0).expect("records");
        rec.finish_session(1_000_100.0).expect("finishes");
        rec.record_chunk(&facts(vec![dead(&troll())]), 1_000_100.0)
            .expect("records");
        assert_eq!(
            killer(&rec).expect("q"),
            ("attack".to_owned(), "last_own_hit".to_owned())
        );

        // and the feed position of every hit is on its row
        let order: Vec<String> = column(
            &rec,
            "SELECT a.name FROM hits h JOIN attacks a ON a.id = h.attack_id ORDER BY h.line_seq",
        )
        .expect("q");
        assert_eq!(order, ["templars_verdict", "attack"]);
    }

    #[test]
    fn credits_the_pummel_when_its_damage_lands_after_the_spell_it_released() {
        let mut lines = vec![
            format!(
                "You take a menacing step toward {TROLL}, sweeping your {MACE} out low to your side in your advance."
            ),
            "[SMR result: 165 (Open d100: 43, Bonus: 65)]".to_owned(),
            format!("With deliberate brutality, you bring your {MACE} around to pummel {TROLL}!"),
            release(),
        ];
        lines.extend(verdict());
        lines.extend(swing_roll());
        let chunk = run(&lines);
        assert_eq!(names(&chunk), ["pummel", "templars_verdict"], "guard");
        let mut rec = auto().expect("opens");
        rec.record_chunk(&chunk, 1_000_000.0).expect("records");
        rec.finish_session(1_000_100.0).expect("finishes");
        rec.record_chunk(&facts(vec![dead(&troll())]), 1_000_100.0)
            .expect("records");
        assert_eq!(
            killer(&rec).expect("q"),
            ("pummel".to_owned(), "last_own_hit".to_owned())
        );
    }
}
