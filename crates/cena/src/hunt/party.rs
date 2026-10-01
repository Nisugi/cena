//! What every character's hunt shares ([`Party`]): each group's board, and
//! each character's seat, from which a leader starts its followers' hunts.
//!
//! A seat is kept by its session, and names the game instance and the
//! character: one name can be on two games, and a seat kept by the name
//! alone was taken by whichever sat last, and emptied when either left
//! (the crate review of 2026-09-28, R6). A leader's `hunt <name> with A B` finds A and B on
//! its own game -- a group is in one game -- whatever the case they are
//! typed in.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, PoisonError};

use cena_behavior::group::{Boards, Place};
use cena_behavior::hunt::{Command, Desk};
use cena_session::{Notice, NoticeKind, SessionHandle, SessionId, SessionObserver};

use super::start_placed;

/// What every character's hunt in this Hydra shares: each group's board, so
/// a leader's hunt and its followers' meet (`plan/39` §5), and each
/// character's seat, so a leader can start its followers' hunts.
///
/// **Owned by the session table** (`play.rs`) and handed to each character's
/// hunt as it opens. It was two process globals, which `plan/05` Rule 5.2
/// forbids ("No process globals. None.") and `every_static_is_allowlisted`
/// caught: shared state belongs to what owns the sessions, not to the process.
#[derive(Clone)]
pub(crate) struct Party {
    pub(super) boards: Arc<Boards>,
    pub(super) seats: Arc<Mutex<Seats>>,
    /// The window, when there is one: each hunt's reports go to its Hunt
    /// pane (`plan/47` step 8, `hunt/panel.rs`).
    pub(super) window: Option<cena_gui::Sessions>,
}

/// Each session's seat, and who sits in it.
pub(super) type Seats = BTreeMap<SessionId, (Sitter, Seat)>;

/// Who sits in a seat: the character's game instance, and its name.
#[derive(Clone, Debug)]
pub(super) struct Sitter {
    instance: String,
    name: String,
}

impl Sitter {
    /// Whether this is `name` on `instance`, whatever the case.
    fn is(&self, instance: &str, name: &str) -> bool {
        self.instance.eq_ignore_ascii_case(instance) && self.name.eq_ignore_ascii_case(name)
    }
}

/// The seat `member` sits in on the game `instance`, other than `leader`'s
/// own session, while its session is `live`.
///
/// A session that ended on its own (a refused login, the retry ladder's
/// cap) stays on the table, seat and all, until it is removed: started on
/// it, a follower's hunt failed on the dead session and the leader waited
/// for it up to `lost_wait`, told nothing (the crate review of 2026-10-01,
/// BI-C-6).
fn seated<'a, T>(
    seats: &'a BTreeMap<SessionId, (Sitter, T)>,
    leader: SessionId,
    instance: &str,
    member: &str,
    live: impl Fn(&T) -> bool,
) -> Option<&'a (Sitter, T)> {
    seats
        .iter()
        .find(|(session, (sitter, seat))| {
            **session != leader && sitter.is(instance, member) && live(seat)
        })
        .map(|(_, seat)| seat)
}

impl Party {
    /// No groups and no seats yet; hunts shown in `window`, when there is one.
    pub(crate) fn new(window: Option<cena_gui::Sessions>) -> Self {
        Self {
            boards: Boards::new(),
            seats: Arc::default(),
            window,
        }
    }

    /// Session `session` left the table: a leader can no longer start its
    /// hunt. Without this, a stopped character's seat stayed, and `hunt ...
    /// with` named it would have started a hunt on a session that was gone.
    pub(crate) fn unseat(&self, session: SessionId) {
        self.seats
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&session);
    }
}

/// A character's hunt desk and session: what a leader's `hunt <name> with`
/// starts a follower's hunt on.
#[derive(Clone)]
pub(super) struct Seat {
    desk: Arc<Desk>,
    handle: SessionHandle,
    observer: SessionObserver,
}

/// This character's seat -- `(instance, name)` is who it is -- for a
/// leader to start its hunt from.
pub(super) fn take_seat(
    party: &Party,
    (instance, name): &(String, String),
    desk: &Arc<Desk>,
    handle: &SessionHandle,
    observer: &SessionObserver,
) {
    let seat = Seat {
        desk: Arc::clone(desk),
        handle: handle.clone(),
        observer: observer.clone(),
    };
    party
        .seats
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .insert(
            handle.session(),
            (
                Sitter {
                    instance: instance.clone(),
                    name: name.clone(),
                },
                seat,
            ),
        );
}

/// `hunt <name> with A B`: each named character's own hunt on the profile
/// of that name, following `leader` -- its game instance and name; then
/// the leader's, waiting for them (`plan/39` §8, question 2). A name this
/// Hydra is not running on the leader's game is said, and hunted without.
pub(super) fn form(
    seats: &Mutex<Seats>,
    desk: &Arc<Desk>,
    handle: &SessionHandle,
    observer: &SessionObserver,
    (instance, leader): &(String, String),
    name: String,
    with: &[String],
) -> tokio::task::JoinHandle<()> {
    let seats = seats.lock().unwrap_or_else(PoisonError::into_inner).clone();
    let mut followers = Vec::new();
    for member in with {
        match seated(&seats, handle.session(), instance, member, |seat| {
            !seat.observer.has_ended()
        }) {
            Some((sitter, seat)) => {
                drop(start_placed(
                    &seat.desk,
                    &seat.handle,
                    &seat.observer,
                    Command::Run(name.clone()),
                    Place::Follow(leader.clone()),
                ));
                followers.push(sitter.name.clone());
            }
            None => handle.say(Notice::line(
                NoticeKind::Warn,
                format!(
                    "Hunt: {member} is not a character this Hydra is running; hunting without them."
                ),
            ).answering()),
        }
    }
    start_placed(
        desk,
        handle,
        observer,
        Command::Run(name),
        Place::Lead(followers),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sitter(instance: &str, name: &str) -> Sitter {
        Sitter {
            instance: instance.to_owned(),
            name: name.to_owned(),
        }
    }

    /// A member is found on the leader's game, by any case, never the
    /// leader's own session; one name on two games is two characters
    /// (the crate review of 2026-09-28, R6).
    #[test]
    fn a_member_is_found_on_the_leaders_game() {
        let seats = BTreeMap::from([
            (SessionId(1), (sitter("GS3", "Nisugi"), 1)),
            (SessionId(2), (sitter("GSF", "Baelor"), 2)),
            (SessionId(3), (sitter("GS3", "Baelor"), 3)),
        ]);
        let found = |leader, instance, member| {
            seated(&seats, SessionId(leader), instance, member, |_| true).map(|(_, seat)| *seat)
        };
        assert_eq!(found(1, "GS3", "baelor"), Some(3));
        assert_eq!(found(1, "GSF", "Baelor"), Some(2));
        assert_eq!(found(1, "GS3", "Nisugi"), None, "not the leader itself");
        assert_eq!(found(9, "GS3", "Nisugi"), Some(1));
        assert_eq!(found(1, "GS3", "Lorwyn"), None);
    }

    /// BI-C-6: a session that ended on its own keeps its seat until it is
    /// taken off the table; a leader does not count it as a follower.
    #[tokio::test]
    async fn a_seat_whose_session_ended_is_not_found() {
        let (source, _transcript) = cena_platform::AnsweringSource::logged_in(b"");
        let session = cena_session::Session::new(source);
        let observer = session.observer();
        let seats = BTreeMap::from([(SessionId(2), (sitter("GS3", "Baelor"), observer))]);
        let live = |observer: &SessionObserver| !observer.has_ended();
        assert!(
            seated(&seats, SessionId(1), "GS3", "Baelor", live).is_some(),
            "a session not yet ended is found"
        );
        drop(session);
        assert!(
            seated(&seats, SessionId(1), "GS3", "Baelor", live).is_none(),
            "an ended session's seat is passed by"
        );
    }
}
