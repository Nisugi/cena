//! The combat recorder: sessions and the attack payload.
//!
//! Ported from `spec/lib/gemstone/combat/recorder_spec.rb` and
//! `recorder_schema_spec.rb`. The receipt, migration, BLOB-typing and mutex
//! cases have no counterpart here (see the module doc's "NOT ported"); every
//! other case under *session lifecycle*, *idle auto-sessioning*,
//! *`record_attack`*, *spawn-tree links*, *guardian redirect*, *ownership* and
//! *rollback safety* is below. The status stream is
//! `combat_recorder_statuses.rs`.

mod recorder_kit;

use cena_model::GameState;
use cena_model::state::combat::event::{
    Confidence, EventTarget, FlareEvent, ParentFlare, Redirect,
};
use cena_model::{Actor, StatusAction, StatusName};
use cena_session::combat_recorder::CombatRecorder;
use recorder_kit::{attack, bolded, chunk, column, count, dead, facts, feed, lizard, one, status};

fn explicit() -> rusqlite::Result<CombatRecorder> {
    CombatRecorder::in_memory(Some("Tester"), "spec", None)
}

fn auto() -> rusqlite::Result<CombatRecorder> {
    CombatRecorder::in_memory(Some("Tester"), "live", Some(300.0))
}

mod session_lifecycle {
    use super::*;

    #[test]
    fn opens_records_into_and_closes_a_session() {
        let mut rec = explicit().expect("opens");
        let id = rec.start_session(1_000.0).expect("starts");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 1_001.0)
            .expect("records");
        assert_eq!(rec.finish_session(1_060.0).expect("finishes"), Some(id));
        assert_eq!(count(&rec, "attacks").expect("q"), 1);
        let (character, source, started, ended): (String, String, f64, f64) = rec
            .connection()
            .query_row(
                "SELECT character, source, started_at, ended_at FROM sessions",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .expect("q");
        assert_eq!((character.as_str(), source.as_str()), ("Tester", "spec"));
        assert!((started - 1_000.0).abs() < f64::EPSILON);
        assert!((ended - 1_060.0).abs() < f64::EPSILON);
        assert_eq!(rec.drain_finished_sessions(), vec![id]);
        assert!(rec.drain_finished_sessions().is_empty());
    }

    #[test]
    fn starting_a_new_session_finishes_the_previous_one() {
        let mut rec = explicit().expect("opens");
        rec.start_session(1_000.0).expect("starts");
        rec.start_session(2_000.0).expect("starts");
        let ended: Vec<Option<f64>> =
            column(&rec, "SELECT ended_at FROM sessions ORDER BY id").expect("q");
        assert_eq!(ended, vec![Some(2_000.0), None]);
    }

    #[test]
    fn does_not_record_when_no_session_is_open() {
        let mut rec = explicit().expect("opens");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 1.0)
            .expect("a no-op, not an error");
        assert_eq!(count(&rec, "attacks").expect("q"), 0);
        assert_eq!(count(&rec, "sessions").expect("q"), 0);
    }

    #[test]
    fn a_file_database_survives_close_and_reopens_with_its_schema() {
        let dir = std::env::temp_dir().join(format!("cena-recorder-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("dir");
        let path = dir.join("combat_stats.db");
        let _ = std::fs::remove_file(&path);
        let mut rec =
            CombatRecorder::open(&path, Some("Tester"), "live", Some(300.0)).expect("opens");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 10.0)
            .expect("records");
        rec.close(20.0).expect("closes");

        let rec = CombatRecorder::open(&path, None, "live", None).expect("reopens");
        assert_eq!(count(&rec, "attacks").expect("q"), 1);
        // closed at the last event, not at close time
        assert_eq!(
            one::<i64>(&rec, "SELECT CAST(ended_at AS INTEGER) FROM sessions").expect("q"),
            10
        );
        assert_eq!(one::<i64>(&rec, "PRAGMA user_version").expect("q"), 1);
        drop(rec);
        let _ = std::fs::remove_dir_all(&dir);
    }
}

mod idle_auto_sessioning {
    use super::*;

    #[test]
    fn opens_a_session_lazily_on_the_first_event() {
        let mut rec = auto().expect("opens");
        assert_eq!(rec.session_id(), None);
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 5.0)
            .expect("records");
        assert!(rec.session_id().is_some());
        assert_eq!(
            one::<i64>(&rec, "SELECT CAST(started_at AS INTEGER) FROM sessions").expect("q"),
            5
        );
    }

    #[test]
    fn a_nearby_players_attack_neither_opens_a_session_nor_keeps_one_alive() {
        let mut rec = auto().expect("opens");
        let mut foreign = attack(&lizard(), 40);
        foreign.foreign_caster = true;
        rec.record_chunk(&chunk(vec![foreign.clone()]), 1_000.0)
            .expect("records");
        assert_eq!(count(&rec, "sessions").expect("q"), 0, "it opened nothing");

        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 2_000.0)
            .expect("records");
        // inside the hunt it IS recorded, for context
        rec.record_chunk(&chunk(vec![foreign.clone()]), 2_100.0)
            .expect("records");
        assert_eq!(count(&rec, "attacks").expect("q"), 2);
        // but it did not keep the hunt alive: 301s after OUR last swing
        rec.record_chunk(&chunk(vec![foreign]), 2_301.0)
            .expect("records");
        assert_eq!(rec.session_id(), None);
        assert_eq!(
            one::<i64>(&rec, "SELECT CAST(ended_at AS INTEGER) FROM sessions").expect("q"),
            2_000
        );
        assert_eq!(count(&rec, "attacks").expect("q"), 2);
    }

    #[test]
    fn an_inbound_attack_is_part_of_our_hunt() {
        let mut rec = auto().expect("opens");
        let mut inbound = attack(&lizard(), 0);
        inbound.inbound = true;
        inbound.target = EventTarget::None;
        rec.record_chunk(&chunk(vec![inbound]), 50.0)
            .expect("records");
        assert_eq!(count(&rec, "sessions").expect("q"), 1);
    }

    #[test]
    fn a_delayed_death_lands_in_the_session_that_fought_the_creature() {
        let mut rec = auto().expect("opens");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 3_000_000.0)
            .expect("records");
        // the room feed confirms the kill well past the idle gap
        rec.record_chunk(&facts(vec![dead(&lizard())]), 3_000_301.0)
            .expect("records");
        assert_eq!(
            count(&rec, "sessions").expect("q"),
            1,
            "no spurious second session"
        );
        assert_eq!(
            column::<String>(&rec, "SELECT status FROM statuses").expect("q"),
            vec!["dead"]
        );
        // on the SAME creature row the attack created
        assert_eq!(count(&rec, "creatures").expect("q"), 1);
        assert_eq!(
            one::<i64>(&rec, "SELECT CAST(killed_at AS INTEGER) FROM creatures").expect("q"),
            3_000_301
        );
    }

    #[test]
    fn a_trailing_fact_neither_extends_the_hunt_nor_resurrects_it_for_a_stranger() {
        let mut rec = auto().expect("opens");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 3_000_000.0)
            .expect("records");
        let sprite = Actor {
            id: Some(999),
            noun: Some("sprite".to_owned()),
            name: "a passing sprite".to_owned(),
        };
        rec.record_chunk(
            &facts(vec![status(
                &sprite,
                StatusName::Stunned,
                StatusAction::Add,
            )]),
            3_000_301.0,
        )
        .expect("records");
        assert_eq!(count(&rec, "statuses").expect("q"), 0, "a stranger's fact");

        rec.record_chunk(&facts(vec![dead(&lizard())]), 3_000_301.0)
            .expect("records");
        assert_eq!(rec.check_idle(3_000_900.0).expect("checks"), Some(1));
        assert_eq!(count(&rec, "sessions").expect("q"), 1);
        // still the last real attack: town time never pads a hunt
        assert_eq!(
            one::<i64>(&rec, "SELECT CAST(ended_at AS INTEGER) FROM sessions").expect("q"),
            3_000_000
        );
        // the re-opened session closed twice, and says so
        assert_eq!(rec.drain_finished_sessions(), vec![1, 1]);
    }

    #[test]
    fn closes_after_the_idle_gap_stamped_at_the_last_event() {
        let mut rec = auto().expect("opens");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 2_000_000.0)
            .expect("records");
        assert_eq!(rec.check_idle(2_000_299.0).expect("checks"), None, "guard");
        assert_eq!(rec.check_idle(2_000_400.0).expect("checks"), Some(1));
        assert_eq!(
            one::<i64>(&rec, "SELECT CAST(ended_at AS INTEGER) FROM sessions").expect("q"),
            2_000_000
        );
    }

    #[test]
    fn the_next_events_own_idle_check_closes_the_previous_session() {
        let mut rec = auto().expect("opens");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 1_000.0)
            .expect("records");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 1_400.0)
            .expect("records");
        assert_eq!(count(&rec, "sessions").expect("q"), 2);
        assert_eq!(rec.drain_finished_sessions(), vec![1]);
        // a new session restarts seq and re-registers the creature
        assert_eq!(
            column::<i64>(&rec, "SELECT seq FROM attacks ORDER BY id").expect("q"),
            vec![1, 1]
        );
        assert_eq!(count(&rec, "creatures").expect("q"), 2);
    }
}

mod the_attack_payload {
    use super::*;

    /// Real feed text, through the parser, the chunk and the state machine.
    fn swing() -> cena_model::state::combat::ChunkFacts {
        let liz = bolded(101, "lizard", "a cave lizard");
        feed(
            &mut GameState::default(),
            &[
                &format!("You swing a broadsword at {liz}!"),
                "  AS: +300 vs DS: +100 with AvD: +30 + d100 roll: +50 = +280",
                "   ... and hit for 40 points of damage!",
                "   Hit on the leg chars the skin and eats into the underlying muscles.",
            ],
        )
    }

    #[test]
    fn writes_attack_resolution_and_hit_rows_and_registers_the_creature() {
        let mut rec = auto().expect("opens");
        rec.record_chunk(&swing(), 100.0).expect("records");
        let (name, kind, weapon, ours, root, id): (String, String, String, bool, i64, i64) = rec
            .connection()
            .query_row(
                "SELECT name, target_kind, weapon, ours, root_attack_id, id FROM attacks",
                [],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                    ))
                },
            )
            .expect("q");
        assert_eq!((name.as_str(), kind.as_str()), ("attack", "creature"));
        assert_eq!(weapon, "broadsword");
        assert!(ours);
        assert_eq!(root, id, "a lone root points at itself");
        let (noun, cname, exist): (String, String, i64) = rec
            .connection()
            .query_row("SELECT noun, name, exist_id FROM creatures", [], |r| {
                Ok((r.get(0)?, r.get(1)?, r.get(2)?))
            })
            .expect("q");
        assert_eq!(
            (noun.as_str(), cname.as_str(), exist),
            ("lizard", "a cave lizard", 101)
        );
        let (rtype, a, d, m, roll, result): (String, i64, i64, i64, i64, i64) = rec
            .connection()
            .query_row(
                "SELECT type, attacker_stat, defender_stat, modifier, roll, result FROM resolutions",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?, r.get(4)?, r.get(5)?)),
            )
            .expect("q");
        assert_eq!(
            (rtype.as_str(), a, d, m, roll, result),
            ("as_ds", 300, 100, 30, 50, 280)
        );
    }

    #[test]
    fn a_hit_carries_its_crit_in_lichs_spellings_with_the_ranks_apart() {
        let mut rec = auto().expect("opens");
        let facts = swing();
        let crit = facts.events[0].hits[0]
            .crit
            .clone()
            .expect("guard: the crit resolved");
        rec.record_chunk(&facts, 100.0).expect("records");
        let (damage, location, part, ctype, rank, wound, line): (
            i64,
            String,
            String,
            String,
            i64,
            i64,
            i64,
        ) = rec
            .connection()
            .query_row(
                "SELECT damage, location, body_part, crit_type, crit_rank, wound_rank, line_seq FROM hits",
                [],
                |r| {
                    Ok((
                        r.get(0)?,
                        r.get(1)?,
                        r.get(2)?,
                        r.get(3)?,
                        r.get(4)?,
                        r.get(5)?,
                        r.get(6)?,
                    ))
                },
            )
            .expect("q");
        assert_eq!(damage, 40);
        // `left leg` / `leftLeg`, never `left_leg`: the answer-key database
        assert!(!location.contains('_'), "{location}");
        assert!(location.ends_with("leg"), "{location}");
        assert!(part.ends_with("Leg"), "{part}");
        assert_eq!(
            Some(ctype),
            crit.damage_type.map(|t| t.as_str().replace('_', "-"))
        );
        assert_eq!(Some(rank), crit.rank.map(i64::from));
        assert_eq!(Some(wound), crit.wound_rank.map(i64::from));
        assert_eq!(line, 2, "the damage line's position in the chunk");
    }

    #[test]
    fn classifies_inbound_foreign_target_foreign_caster_and_unowned() {
        let mut rec = explicit().expect("opens");
        rec.start_session(0.0).expect("starts");
        let mut inbound = attack(&lizard(), 0);
        inbound.inbound = true;
        inbound.target = EventTarget::None;
        inbound.attacker = Some(lizard());
        let mut third_party = attack(&lizard(), 0);
        third_party.target = EventTarget::Foreign("Tijay".to_owned());
        let mut bystander = attack(&lizard(), 10);
        bystander.foreign_caster = true;
        let mut tick = attack(&lizard(), 5);
        tick.unowned = true;
        let mut orphan = attack(&lizard(), 5);
        orphan.orphan = true;
        orphan.target = EventTarget::None;
        rec.record_chunk(
            &chunk(vec![
                attack(&lizard(), 1),
                inbound,
                third_party,
                bystander,
                tick,
                orphan,
            ]),
            1.0,
        )
        .expect("records");
        let kinds: Vec<String> =
            column(&rec, "SELECT target_kind FROM attacks ORDER BY id").expect("q");
        assert_eq!(
            kinds,
            [
                "creature", "self", "foreign", "creature", "creature", "none"
            ]
        );
        let ours: Vec<bool> = column(&rec, "SELECT ours FROM attacks ORDER BY id").expect("q");
        assert_eq!(ours, [true, false, false, false, false, false]);
        assert_eq!(
            one::<i64>(
                &rec,
                "SELECT attacker_exist_id FROM attacks WHERE inbound = 1"
            )
            .expect("q"),
            101
        );
    }

    #[test]
    fn splits_spawned_cast_lineage_into_parent_and_parent_weapon() {
        let mut rec = explicit().expect("opens");
        rec.start_session(0.0).expect("starts");
        let mut child = attack(&lizard(), 20);
        "blink".clone_into(&mut child.name);
        child.via_cast = true;
        child.parent_flare = Some(ParentFlare {
            flare: "weapon_cast".to_owned(),
            weapon: Some(Actor::unlinked("a  faewood longbow ")),
        });
        rec.record_chunk(&chunk(vec![child]), 1.0).expect("records");
        let (parent, weapon, via, conf): (String, String, String, String) = rec
            .connection()
            .query_row(
                "SELECT parent, parent_weapon, via, parent_confidence FROM attacks",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)),
            )
            .expect("q");
        assert_eq!(parent, "weapon_cast");
        assert_eq!(weapon, "a faewood longbow", "trimmed and squeezed");
        assert_eq!(via, "cast");
        // flare-spawned, and no spawner row could be asserted
        assert_eq!(conf, "unbound");
    }

    #[test]
    fn records_flares_with_their_own_hits_and_creature() {
        let zerk = bolded(121_654_846, "berserker", "a tattooed gigas berserker");
        let masto = bolded(121_678_494, "mastodon", "a heavily armored battle mastodon");
        let facts = feed(
            &mut GameState::default(),
            &[
                &format!("You fire a faewood arrow at {zerk}!"),
                "   ... and hit for 188 points of damage!",
                &format!(
                    " ** A bloom of spectral light blossoms around {masto}, engulfing it in searing brilliance! **"
                ),
                "   ... 5 points of damage!",
                "   Smack to the eye bursts blood vessels.",
            ],
        );
        let mut rec = auto().expect("opens");
        rec.record_chunk(&facts, 9.0).expect("records");
        assert_eq!(count(&rec, "creatures").expect("q"), 2);
        let rows: Vec<(Option<i64>, i64, i64)> = {
            let mut stmt = rec
                .connection()
                .prepare(
                    "SELECT h.flare_id, h.damage, c.exist_id FROM hits h \
                     JOIN creatures c ON c.id = h.creature_id ORDER BY h.seq",
                )
                .expect("q");
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)))
                .expect("q")
                .collect::<Result<_, _>>()
                .expect("q")
        };
        let flare_id: i64 = one(&rec, "SELECT id FROM flares").expect("one flare");
        assert_eq!(
            rows,
            [(None, 188, 121_654_846), (Some(flare_id), 5, 121_678_494)]
        );
        // the flare's crit stun rode the flare, and is filed on it
        let (kind, fid, source): (String, i64, String) = rec
            .connection()
            .query_row(
                "SELECT kind, flare_id, source FROM statuses WHERE kind = 'stun'",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("the stun row");
        assert_eq!(
            (kind.as_str(), fid, source.as_str()),
            ("stun", flare_id, "event")
        );
    }

    #[test]
    fn flares_ours_is_decided_once_two_person_even_on_an_inbound_row() {
        let mut rec = explicit().expect("opens");
        rec.start_session(0.0).expect("starts");
        let flare = |attacker: Option<Actor>| {
            let mut f: FlareEvent = blank_flare();
            f.name = "shield_spike".to_owned();
            f.target = None;
            f.attacker = attacker;
            f
        };
        let mut inbound = attack(&lizard(), 0);
        inbound.inbound = true;
        inbound.flares = vec![flare(None), flare(Some(lizard()))];
        let mut bystander = attack(&lizard(), 0);
        bystander.foreign_caster = true;
        bystander.flares = vec![flare(None)];
        rec.record_chunk(&chunk(vec![inbound, bystander]), 1.0)
            .expect("records");
        let ours: Vec<bool> = column(&rec, "SELECT ours FROM flares ORDER BY id").expect("q");
        assert_eq!(ours, [true, false, false]);
    }

    /// A flare with nothing on it, built through the classifier's own type
    /// so the private bookkeeping fields take their defaults.
    fn blank_flare() -> FlareEvent {
        let facts = feed(
            &mut GameState::default(),
            &[
                &format!("You swing a broadsword at {}!", bolded(1, "rat", "a rat")),
                "   ... and hit for 1 point of damage!",
                &format!(
                    " ** A bloom of spectral light blossoms around {}, engulfing it in searing brilliance! **",
                    bolded(1, "rat", "a rat")
                ),
            ],
        );
        facts.events[0].flares[0].clone()
    }
}

mod spawn_tree_links {
    use super::*;

    fn blink_pair() -> Vec<cena_model::state::combat::AttackEvent> {
        let mut child = attack(&lizard(), 20);
        "blink".clone_into(&mut child.name);
        child.root = Some(0);
        child.parent = Some(0);
        child.parent_confidence = Some(Confidence::Bracket);
        vec![attack(&lizard(), 40), child]
    }

    #[test]
    fn a_blink_child_resolves_to_the_root_row_it_shares_a_chunk_with() {
        let mut rec = explicit().expect("opens");
        rec.start_session(0.0).expect("starts");
        rec.record_chunk(&chunk(blink_pair()), 1.0)
            .expect("records");
        let rows: Vec<(i64, i64, Option<i64>, Option<String>)> = {
            let mut stmt = rec
                .connection()
                .prepare("SELECT id, root_attack_id, parent_attack_id, parent_confidence FROM attacks ORDER BY id")
                .expect("q");
            stmt.query_map([], |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?)))
                .expect("q")
                .collect::<Result<_, _>>()
                .expect("q")
        };
        assert_eq!(
            rows,
            [
                (1, 1, None, None),
                (2, 1, Some(1), Some("bracket".to_owned()))
            ]
        );
    }

    #[test]
    fn indices_never_cross_link_between_chunks() {
        let mut rec = explicit().expect("opens");
        rec.start_session(0.0).expect("starts");
        rec.record_chunk(&chunk(blink_pair()), 1.0)
            .expect("records");
        rec.record_chunk(&chunk(blink_pair()), 1.0)
            .expect("records");
        let roots: Vec<i64> =
            column(&rec, "SELECT root_attack_id FROM attacks ORDER BY id").expect("q");
        assert_eq!(roots, [1, 1, 3, 3]);
        // two chunks inside one whole second still order
        let seqs: Vec<i64> = column(&rec, "SELECT chunk_seq FROM attacks ORDER BY id").expect("q");
        assert_eq!(seqs, [1, 1, 2, 2]);
    }

    #[test]
    fn never_links_across_a_session_boundary_when_a_foreign_event_held_index_0() {
        let mut rec = auto().expect("opens");
        rec.record_chunk(&chunk(blink_pair()), 1_000.0)
            .expect("records");
        // next hunt: index 0 is a bystander's cast that records nothing
        // (no session is open yet), and the child names index 0 as its root
        let mut pair = blink_pair();
        pair[0].foreign_caster = true;
        rec.record_chunk(&chunk(pair), 2_000.0).expect("records");
        let (root, parent, id): (i64, Option<i64>, i64) = rec
            .connection()
            .query_row(
                "SELECT root_attack_id, parent_attack_id, id FROM attacks ORDER BY id DESC LIMIT 1",
                [],
                |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
            )
            .expect("q");
        assert_eq!(
            (root, parent),
            (id, None),
            "degraded to a self-root, not the old hunt's row"
        );
    }

    #[test]
    fn an_ambiguous_echo_is_rooted_but_parentless() {
        let mut rec = explicit().expect("opens");
        rec.start_session(0.0).expect("starts");
        let mut pair = blink_pair();
        pair[1].parent = None;
        pair[1].parent_confidence = None;
        rec.record_chunk(&chunk(pair), 1.0).expect("records");
        let (root, parent): (i64, Option<i64>) = rec
            .connection()
            .query_row(
                "SELECT root_attack_id, parent_attack_id FROM attacks WHERE id = 2",
                [],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .expect("q");
        assert_eq!((root, parent), (1, None));
    }
}

#[test]
fn a_redirect_stores_the_intended_victim_only_when_honored() {
    let mut rec = explicit().expect("opens");
    rec.start_session(0.0).expect("starts");
    let redirect = |honored| {
        Some(Redirect {
            interceptor: lizard(),
            intended: "shaper".to_owned(),
            honored,
        })
    };
    let mut honored = attack(&lizard(), 10);
    honored.redirect = redirect(true);
    let mut announced = attack(&lizard(), 10);
    announced.redirect = redirect(false);
    rec.record_chunk(&chunk(vec![honored, announced]), 1.0)
        .expect("records");
    let from: Vec<Option<String>> =
        column(&rec, "SELECT redirected_from FROM attacks ORDER BY id").expect("q");
    assert_eq!(from, [Some("shaper".to_owned()), None]);
}

mod rollback_safety {
    use super::*;

    /// Make the NEXT hit insert fail, after the creature and attack rows of
    /// the same chunk have been written.
    fn sabotage(rec: &CombatRecorder) -> rusqlite::Result<()> {
        rec.connection().execute_batch(
            "CREATE TRIGGER boom BEFORE INSERT ON hits BEGIN SELECT RAISE(ABORT, 'boom'); END;",
        )
    }

    #[test]
    fn a_failed_chunk_leaves_no_rows_and_no_cached_ids() {
        let mut rec = auto().expect("opens");
        sabotage(&rec).expect("trigger");
        assert!(
            rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 1.0)
                .is_err()
        );
        for table in ["sessions", "creatures", "attacks", "hits"] {
            assert_eq!(count(&rec, table).expect("q"), 0, "{table}");
        }
        assert_eq!(
            rec.session_id(),
            None,
            "the session it opened rolled back too"
        );

        // the recorder is usable, and does not hand out a rolled-back row id
        rec.connection()
            .execute_batch("DROP TRIGGER boom;")
            .expect("drop");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 2.0)
            .expect("records");
        let orphans: i64 = one(
            &rec,
            "SELECT count(*) FROM attacks a LEFT JOIN creatures c ON c.id = a.creature_id \
             WHERE c.id IS NULL",
        )
        .expect("q");
        assert_eq!(orphans, 0);
        assert_eq!(one::<i64>(&rec, "SELECT seq FROM attacks").expect("q"), 1);
    }

    #[test]
    fn a_failed_chunk_does_not_disturb_the_open_session() {
        let mut rec = auto().expect("opens");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 1.0)
            .expect("records");
        sabotage(&rec).expect("trigger");
        assert!(
            rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 2.0)
                .is_err()
        );
        rec.connection()
            .execute_batch("DROP TRIGGER boom;")
            .expect("drop");
        rec.record_chunk(&chunk(vec![attack(&lizard(), 40)]), 3.0)
            .expect("records");
        assert_eq!(
            column::<i64>(&rec, "SELECT seq FROM attacks ORDER BY id").expect("q"),
            [1, 2],
            "the failed attack's seq was given back"
        );
        assert_eq!(count(&rec, "sessions").expect("q"), 1);
    }
}
