//! When a character's facts reach disk.
//!
//! > **AUTHOR, 2026-09-20:** *"I say they are written to a queue that gets
//! > drained 5 minutes after it stops changing?"*
//!
//! These run on tokio's paused clock, so "five minutes" costs no wall time and
//! the boundary is exact rather than approximate.

use std::time::Duration;

use cena_model::state::character::snapshot::Group;
use cena_platform::ReplaySource;
use cena_session::dirty_groups::{DirtyGroups, IDLE_WINDOW};
use cena_session::{Session, character_store};

/// Who the character is, which the login burst sends and the store needs for
/// a filename.
pub(crate) const WHO: &str = concat!(
    r#"<app char="Nisugi" game="GS" title="[GSIV: Nisugi, the Ranger] (Prime)"/>"#,
    "\n",
);

/// A `society` report: one fact, in one chunk closed by a prompt.
pub(crate) const SOCIETY: &str = concat!(
    "<popBold/>   You are a Master of the Guardians of Sunfist.\n",
    "<prompt time=\"1\">&gt;</prompt>\n",
);

pub(crate) fn temp_dir(name: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("cena-persist-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    dir
}

mod the_queue {
    use super::{DirtyGroups, Group};

    #[test]
    fn a_group_marked_twice_is_one_entry() {
        // Why the queue holds GROUPS: train five PSM ranks and that is five
        // marks and ONE write. It cannot grow past six entries however many
        // facts arrive.
        let mut dirty = DirtyGroups::default();
        for _ in 0..5 {
            dirty.mark(Group::Psms);
        }
        assert_eq!(dirty.len(), 1);
    }

    #[test]
    fn draining_empties_it() {
        let mut dirty = DirtyGroups::default();
        dirty.mark_all([Group::Stats, Group::Standing]);
        assert_eq!(dirty.drain(), [Group::Stats, Group::Standing]);
        assert!(dirty.is_empty(), "a second drain would write nothing");
    }

    #[test]
    fn it_drains_in_a_stable_order() {
        // Criterion 7: a replay that iterated a `HashSet` would not be
        // deterministic.
        let mut dirty = DirtyGroups::default();
        dirty.mark_all([Group::Standing, Group::Stats, Group::Identity]);
        assert_eq!(
            dirty.drain(),
            [Group::Stats, Group::Identity, Group::Standing],
            "`Group::ALL` order, not insertion order"
        );
    }
}

#[test]
fn the_window_is_five_minutes() {
    // The author's number, in one place, so a change to it is a change to
    // this line rather than to a literal buried in the actor.
    assert_eq!(IDLE_WINDOW, Duration::from_mins(5));
}

// A test that a fact is NOT written before the window elapses is deliberately
// absent, and this says why rather than leaving a gap.
//
// It needs a session that is still running five minutes after learning
// something. `ReplaySource` hits EOF immediately, which ends the session
// cleanly and takes the shutdown flush -- so the file appears at once, for a
// reason that has nothing to do with the timer. `AnsweringSource` stays open
// but only speaks when spoken to, so the facts would have to be provoked by a
// command, which tests the queue rather than the clock.
//
// What the timer arm actually does is covered where it is cheap to be sure of:
// `the_window_is_five_minutes` pins the constant, `the_queue` pins the
// coalescing, and `a_clean_shutdown_flushes_without_waiting` proves the drain
// writes real values. The arm itself -- a guarded `sleep_until` copied line
// for line from the quit deadline beside it -- is **enforced by review**,
// exactly as `character_store::save`'s atomicity is, and for the same honest
// reason: the test that would prove it costs more than it is worth.

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_clean_shutdown_flushes_without_waiting() {
    // What makes the five-minute window SAFE: an ordinary logout never waits
    // on it, so the only way to lose facts is a crash -- and a crash loses
    // facts a sync re-teaches.
    //
    // The replay ends, which ends the session cleanly, which is this path.
    let dir = temp_dir("shutdown");
    let wire = format!("{WHO}{SOCIETY}");
    let session =
        Session::new(ReplaySource::from_bytes(wire.as_bytes())).with_character_store(dir.clone());
    let end = session.into_actor().run().await;

    assert!(
        end.state.character.standing.society.is_some(),
        "guard: the model learned it"
    );
    let stored = character_store::load(&dir, "GS", "Nisugi").expect("written on shutdown");
    assert_eq!(
        stored.standing.society_rank,
        Some(20),
        "and it is the real value, not an empty snapshot"
    );
    assert!(
        stored.is_known(Group::Standing),
        "the group is stamped, so the next login does not re-sync it"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_group_nothing_taught_is_not_stamped_fresh() {
    // The staleness chain depends on this: stamping every group on every save
    // would report a character as fully synced after learning one fact, and
    // the next login would skip the sync it needs.
    let dir = temp_dir("untaught");
    let wire = format!("{WHO}{SOCIETY}");
    let session =
        Session::new(ReplaySource::from_bytes(wire.as_bytes())).with_character_store(dir.clone());
    let _ = session.into_actor().run().await;

    let stored = character_store::load(&dir, "GS", "Nisugi").expect("load");
    assert!(
        stored.is_known(Group::Standing),
        "guard: this one was taught"
    );
    for group in [Group::Stats, Group::Skills, Group::Psms, Group::Enhancives] {
        assert!(
            !stored.is_known(group),
            "{group:?} was never taught and must still read as stale"
        );
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn a_session_with_no_store_still_learns_and_writes_nothing() {
    // Every existing test is in this state.
    let dir = temp_dir("unconfigured");
    let wire = format!("{WHO}{SOCIETY}");
    let session = Session::new(ReplaySource::from_bytes(wire.as_bytes()));
    let end = session.into_actor().run().await;

    assert!(end.state.character.standing.society.is_some());
    assert!(!dir.exists(), "no directory was touched");
}

#[tokio::test(flavor = "current_thread", start_paused = true)]
async fn the_login_burst_teaches_who_the_file_is_about() {
    // Without `<app>` there is no filename to write under. Rule 2.2a: the
    // parser has carried this since `Frame::AppInfo` was widened, and nothing
    // consumed it until now.
    let session = Session::new(ReplaySource::from_bytes(WHO.as_bytes()));
    let end = session.into_actor().run().await;
    assert_eq!(end.state.character.name.as_deref(), Some("Nisugi"));
    assert_eq!(end.state.character.instance.as_deref(), Some("GS"));
}

mod loading {
    use super::{SOCIETY, WHO, temp_dir};
    use cena_model::state::character::snapshot::Group;
    use cena_model::state::character::vocabulary::Society;
    use cena_platform::ReplaySource;
    use cena_session::{Session, character_store};

    /// One session learns and saves; the next reads it back.
    ///
    /// **The point of the whole feature.** Without this the store only ever
    /// writes, and every login starts blank -- so a character would re-sync
    /// sixteen commands every time regardless of what was on disk.
    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_second_session_starts_from_what_the_first_learned() {
        let dir = temp_dir("round-trip");

        let first = format!("{WHO}{SOCIETY}");
        let end = Session::new(ReplaySource::from_bytes(first.as_bytes()))
            .with_character_store(dir.clone())
            .into_actor()
            .run()
            .await;
        assert_eq!(
            end.state.character.standing.society_rank,
            Some(20),
            "guard: the first session learned it"
        );

        // The second session is told WHO it is and nothing else.
        let end = Session::new(ReplaySource::from_bytes(WHO.as_bytes()))
            .with_character_store(dir.clone())
            .into_actor()
            .run()
            .await;
        assert_eq!(
            end.state.character.standing.society,
            Some(Some(Society::GuardiansOfSunfist)),
            "restored from disk, not from the wire"
        );
        assert_eq!(end.state.character.standing.society_rank, Some(20));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_character_with_no_file_starts_blank_rather_than_failing() {
        // The ordinary case for a character nobody has synced. It must not be
        // an error: a session that refused to start would be worse than one
        // that syncs.
        let dir = temp_dir("no-file");
        let end = Session::new(ReplaySource::from_bytes(WHO.as_bytes()))
            .with_character_store(dir.clone())
            .into_actor()
            .run()
            .await;
        assert_eq!(end.state.character.standing.society, None);
        assert_eq!(end.state.character.name.as_deref(), Some("Nisugi"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn what_the_session_learns_outranks_what_was_stored() {
        // The load happens on `<app>`, which arrives BEFORE the game teaches
        // anything else -- so a fact from this session always lands on top of
        // the restored one rather than under it. A load that ran later, or
        // twice, would overwrite fresh facts with old ones.
        let dir = temp_dir("fresh-wins");

        // Store a Voln membership.
        let voln = format!(
            "{WHO}   You are a member in the Order of Voln at rank 5.\n<prompt time=\"1\">&gt;</prompt>\n"
        );
        let _ = Session::new(ReplaySource::from_bytes(voln.as_bytes()))
            .with_character_store(dir.clone())
            .into_actor()
            .run()
            .await;
        assert_eq!(
            character_store::load(&dir, "GS", "Nisugi")
                .expect("stored")
                .standing
                .society,
            Some(Some(Society::OrderOfVoln)),
            "guard: Voln is on disk"
        );

        // Now a session where the game says Sunfist.
        let end = Session::new(ReplaySource::from_bytes(
            format!("{WHO}{SOCIETY}").as_bytes(),
        ))
        .with_character_store(dir.clone())
        .into_actor()
        .run()
        .await;
        assert_eq!(
            end.state.character.standing.society,
            Some(Some(Society::GuardiansOfSunfist)),
            "the wire wins over the file"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn a_second_app_frame_does_not_reload_over_fresh_facts() {
        // MUTATION-FOUND. Removing the `loaded` guard left the whole suite
        // green, because `<app>` arrives once in a replay. It does NOT arrive
        // once in life: the login burst re-sends it on every reconnect
        // (`reconnect.rs` measures the burst), so without the guard a
        // reconnect mid-session would overwrite everything learned since with
        // the older values on disk.
        let dir = temp_dir("reload");

        // Voln on disk.
        let voln = format!(
            "{WHO}   You are a member in the Order of Voln at rank 5.
<prompt time=\"1\">&gt;</prompt>
"
        );
        let _ = Session::new(ReplaySource::from_bytes(voln.as_bytes()))
            .with_character_store(dir.clone())
            .into_actor()
            .run()
            .await;

        // Learn Sunfist, THEN receive `<app>` again as a reconnect would.
        let wire = format!("{WHO}{SOCIETY}{WHO}");
        let end = Session::new(ReplaySource::from_bytes(wire.as_bytes()))
            .with_character_store(dir.clone())
            .into_actor()
            .run()
            .await;
        assert_eq!(
            end.state.character.standing.society,
            Some(Some(Society::GuardiansOfSunfist)),
            "the second `<app>` must not restore Voln over it"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test(flavor = "current_thread", start_paused = true)]
    async fn the_restored_timestamps_survive_a_later_save() {
        // What makes the staleness chain work across sessions: a group taught
        // long ago keeps its stamp when a different group is written today.
        let dir = temp_dir("stamps");

        let _ = Session::new(ReplaySource::from_bytes(
            format!("{WHO}{SOCIETY}").as_bytes(),
        ))
        .with_character_store(dir.clone())
        .into_actor()
        .run()
        .await;
        let first = character_store::load(&dir, "GS", "Nisugi").expect("stored");
        let stamp = first
            .group_updated_at(Group::Standing)
            .expect("Standing stamped");

        // A second session that learns the SAME thing -- nothing changes, so
        // nothing is marked, so the stamp must not move.
        let _ = Session::new(ReplaySource::from_bytes(
            format!("{WHO}{SOCIETY}").as_bytes(),
        ))
        .with_character_store(dir.clone())
        .into_actor()
        .run()
        .await;
        let second = character_store::load(&dir, "GS", "Nisugi").expect("stored");
        assert_eq!(
            second.group_updated_at(Group::Standing),
            Some(stamp),
            "a re-sync that finds nothing new does not restamp"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }
}
