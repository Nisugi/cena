//! A behavior as an operation (`plan/35` §6, M7 step 3): the controls of one
//! run, and its ending in the words an operation's result is given in
//! ([`cena_session::operation::Ended`]).
//!
//! # One run's controls, never the next run's
//!
//! A desk's own stop (`hunt stop`, `go2 stop`) stops whatever it is running.
//! An agent steers the operation it started, which may have been replaced by
//! a hunt the player began since: its [`Steering`] holds that run's own
//! token, so a stop sent late stops nothing it did not start.
//!
//! # What a verdict is (issue #19, point 3)
//!
//! The work's verdict is the behavior's own, read off its typed ending:
//! `completed` means the machine met its stopping rule or the walk arrived,
//! and **never** that anything was killed or looted, which no ending records.
//! What a walk left undone -- an item still stored, one taken out and not
//! put back, a stance not restored -- is carried as `left`, apart from the
//! verdict, so a failed trip that tidied up is still a failed trip.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use cena_session::operation::{Ended, Progress, Work};
use tokio::sync::watch;
use tokio::task::JoinHandle;
use tokio_util::sync::CancellationToken;

use crate::error::BehaviorError;
use crate::hunt::{Ending, HuntEnd};
use crate::travel::{Ended as Walked, Travelled, Why};

/// A behavior started: its task, and the controls for that run alone.
#[derive(Debug)]
pub struct Underway<T> {
    /// Over when the run is, with how it ended.
    pub task: JoinHandle<T>,
    /// How the run is steered.
    pub steering: Steering,
}

/// The controls of one run. A hunt heeds all of them
/// (`crate::hunt::Hunt::heed`); a walk only its stop.
#[derive(Clone, Debug)]
pub struct Steering {
    stop: CancellationToken,
    hold: Arc<AtomicBool>,
    retreat: Arc<AtomicBool>,
    /// How the run is getting on, as it last said (`crate::hunt` progress).
    progress: Arc<watch::Sender<Option<Progress>>>,
}

impl Steering {
    /// The controls of the run `stop` cancels.
    #[must_use]
    pub fn new(stop: CancellationToken) -> Self {
        Self {
            stop,
            hold: Arc::default(),
            retreat: Arc::default(),
            progress: Arc::new(watch::Sender::new(None)),
        }
    }

    /// The run's progress now; whoever [`Self::progress`] watches hears of
    /// it only when it differs from the last.
    pub fn report(&self, progress: Progress) {
        self.progress.send_if_modified(|last| {
            let news = last.as_ref() != Some(&progress);
            if news {
                *last = Some(progress);
            }
            news
        });
    }

    /// Watch the run's progress.
    #[must_use]
    pub fn progress(&self) -> watch::Receiver<Option<Progress>> {
        self.progress.subscribe()
    }

    /// Stop this run, as the player's stop does. Stopping it again is
    /// nothing.
    pub fn stop(&self) {
        self.stop.cancel();
    }

    /// Hold: defend, and start nothing (`crate::hunt` steer's docs).
    pub fn hold(&self) {
        self.hold.store(true, Ordering::Relaxed);
    }

    /// Go on after a hold.
    pub fn resume(&self) {
        self.hold.store(false, Ordering::Relaxed);
    }

    /// Walk to the resting room and end there; a hold gives way to it.
    pub fn retreat(&self) {
        self.retreat.store(true, Ordering::Relaxed);
        self.hold.store(false, Ordering::Relaxed);
    }

    /// Whether the run is held.
    pub(crate) fn held(&self) -> bool {
        self.hold.load(Ordering::Relaxed)
    }

    /// Whether the run was asked to retreat.
    pub(crate) fn retreating(&self) -> bool {
        self.retreat.load(Ordering::Relaxed)
    }
}

/// A stop from outside, as an operation's result.
fn stopped(why: BehaviorError) -> Ended {
    match why {
        BehaviorError::Cancelled => Ended::plainly(Work::Interrupted, "stopped"),
        BehaviorError::Dead => Ended::plainly(Work::Failed, "dead"),
        BehaviorError::Disconnected => Ended::plainly(Work::Interrupted, "disconnected"),
        BehaviorError::AuthorityHeld => Ended::plainly(Work::NoOpportunity, "authority_held"),
        BehaviorError::Wedged => Ended::plainly(Work::Failed, "wedged"),
        BehaviorError::FellBehind => Ended::plainly(Work::Interrupted, "fell_behind"),
    }
}

impl HuntEnd {
    /// This ending as an operation's result.
    #[must_use]
    pub fn ended(&self) -> Ended {
        let ending = match self {
            Self::Stopped(why) => return stopped(*why),
            Self::Finished(ending) => ending,
        };
        let (work, reason) = match ending {
            Ending::Healed => (Work::Completed, "healed"),
            Ending::Stocked => (Work::Completed, "stocked"),
            Ending::Waggled => (Work::Completed, "waggled"),
            Ending::Sent => (Work::Completed, "sent"),
            Ending::Looted(_) => (Work::Completed, "looted"),
            Ending::Rested(_) => (Work::Completed, "rested"),
            Ending::Cleared => (Work::Completed, "cleared"),
            Ending::Bounty => (Work::Completed, "bounty"),
            Ending::Dead => (Work::Failed, "dead"),
            Ending::NoRestingRoom => (Work::Failed, "no_resting_room"),
            Ending::NoHuntingRoom => (Work::Failed, "no_hunting_room"),
            Ending::Unreachable(_) => (Work::Failed, "unreachable"),
            Ending::Injured => (Work::Failed, "injured"),
            Ending::NoWands => (Work::Failed, "no_wands"),
            Ending::Unblessed => (Work::Failed, "unblessed"),
            Ending::Disarmed => (Work::Failed, "disarmed"),
            Ending::NoEffect => (Work::Failed, "no_effect"),
            Ending::Trouble => (Work::Failed, "trouble"),
            Ending::Deader => (Work::Interrupted, "deader"),
            Ending::MemberDied => (Work::Interrupted, "member_died"),
            Ending::LeaderStopped => (Work::Interrupted, "leader_stopped"),
            // Arrived where it was sent, and the hunt was cut short to get
            // there: a safe return is not a finished hunt (issue #19).
            Ending::Retreated => (Work::Interrupted, "retreated"),
        };
        Ended::plainly(work, reason)
    }
}

impl Travelled {
    /// This walk's ending as an operation's result, with what it left undone.
    #[must_use]
    pub fn ended(&self) -> Ended {
        let mut ended = match &self.ended {
            Walked::Arrived => Ended::plainly(Work::Completed, "arrived"),
            Walked::Failed(Why::NoRoute) => Ended::plainly(Work::Failed, "no_route"),
            Walked::Failed(Why::OffTheMap) => Ended::plainly(Work::Failed, "off_the_map"),
            Walked::Failed(Why::TooManyReplans) => Ended::plainly(Work::Failed, "too_many_replans"),
            Walked::UnknownSpell => Ended::plainly(Work::Failed, "unknown_spell"),
            Walked::Stopped(why) => stopped(*why),
            Walked::Halted => Ended::plainly(Work::Interrupted, "halted"),
        };
        if let Some(halted) = &self.halted {
            ended
                .left
                .push(format!("a routine stopped the walk: {halted}"));
        }
        for stored in &self.still_stored {
            ended.left.push(format!("still stored: {stored:?}"));
        }
        if let Some(out) = &self.still_out {
            ended
                .left
                .push(format!("taken out and not put back: {out}"));
        }
        if let Some(stance) = &self.stance_before {
            ended
                .left
                .push(format!("stance not put back: it was {stance}"));
        }
        ended
    }
}
