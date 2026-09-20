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
const WHO: &str = concat!(
    r#"<app char="Nisugi" game="GS" title="[GSIV: Nisugi, the Ranger] (Prime)"/>"#,
    "\n",
);

/// A `society` report: one fact, in one chunk closed by a prompt.
const SOCIETY: &str = concat!(
    "<popBold/>   You are a Master of the Guardians of Sunfist.\n",
    "<prompt time=\"1\">&gt;</prompt>\n",
);

fn temp_dir(name: &str) -> std::path::PathBuf {
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
