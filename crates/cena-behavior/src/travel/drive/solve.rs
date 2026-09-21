//! The one loop that runs every named routine (`super::super::routines`):
//! show the solver what the walker sees, do what it asks, and show it again.
//!
//! **A routine's moves go back through the trip** (`Trip::aside`), so `go
//! door` inside a puzzle has the same ladder of remedies as a plain exit, and
//! whatever its steps change is owed back by the trip like any crossing's.
//! `Script.run('go2', …)` is a trip inside the trip: the same driver walks
//! it, with the same hands and the same stop.

use std::time::Duration;

use cena_map::{Action, RoomId, Routine, Step};
use cena_session::{ChunkLine, CommandId, MoveFeedback, movement};
use tokio::time::Instant;

use super::super::mover::wait_ms;
use super::super::routines::{Next, Seen, solver_for};
use super::super::{Trip, walker_from};
use super::{Cx, Driver, Ended, Turn};

/// How many things one routine may ask for: the driver's stop, above any
/// solver's own. The longest upstream loop is the Confluence's, unbounded.
const MAX_ASKS: u32 = 2000;
/// How often a command refused for roundtime is sent again.
const MAX_WAITS: u32 = 3;

impl<N: FnMut() -> CommandId> Driver<'_, N> {
    /// Run a routine to its end. `false`: it could not cross.
    pub(super) async fn solve(
        &mut self,
        cx: &mut Cx<'_>,
        routine: &Routine,
    ) -> Result<bool, Ended> {
        let (Some(mut solver), Some(goal)) = (solver_for(routine), cx.trip.routine_to()) else {
            return Ok(false);
        };
        let mut ok = true;
        let mut answer: Vec<ChunkLine> = Vec::new();
        for _ in 0..MAX_ASKS {
            self.drain(cx.trip).map_err(Ended::Stopped)?;
            let here = self.locate(cx.map);
            let server = self.state.game_time_now().unwrap_or(0);
            let walker = walker_from(&self.state, cx.notes, server);
            let next = solver.next(&Seen {
                here,
                goal,
                walker: &walker,
                state: &self.state,
                answer: &answer,
                ok,
                random: cx.trip.draw(),
            });
            answer.clear();
            ok = true;
            match next {
                Next::Put(command) => answer = self.put(cx.trip, &command).await?,
                Next::Go(command) => {
                    let step = Step {
                        action: Action::Move(command),
                        when: None,
                    };
                    ok = self.aside(cx, vec![step]).await?;
                }
                Next::Steps(steps) => ok = self.aside(cx, steps).await?,
                Next::WalkTo(room) => ok = self.walk_to(cx, room).await?,
                Next::Await(lines, ms) => {
                    ok = self.await_line(cx.trip, &lines, ms).await?;
                    answer.clone_from(&self.answer);
                }
                Next::Pause(ms) => {
                    let until = Instant::now() + Duration::from_millis(ms);
                    while Instant::now() < until {
                        self.hold(cx.trip).await?;
                    }
                }
                Next::Done => return Ok(true),
                Next::Failed => return Ok(false),
            }
        }
        Ok(false)
    }

    /// `fput`: send, take the answer, and send again if it was roundtime.
    async fn put(&mut self, trip: &mut Trip, command: &str) -> Result<Vec<ChunkLine>, Ended> {
        for _ in 0..MAX_WAITS {
            self.exchange(trip, command).await?;
            let wait = self
                .answer
                .iter()
                .find_map(|line| match movement::classify(&line.text()) {
                    Some(MoveFeedback::Wait(seconds)) => Some(seconds),
                    _ => None,
                });
            let Some(seconds) = wait else { break };
            let until = Instant::now() + Duration::from_millis(wait_ms(seconds));
            while Instant::now() < until {
                self.hold(trip).await?;
            }
        }
        Ok(self.answer.clone())
    }

    /// Steps run beside the plan, to their end.
    async fn aside(&mut self, cx: &mut Cx<'_>, steps: Vec<Step>) -> Result<bool, Ended> {
        cx.trip.aside(steps, None);
        loop {
            match Box::pin(self.turn(cx)).await? {
                Turn::Aside(worked) => return Ok(worked),
                Turn::Arrived => return Ok(true),
                Turn::On => {}
            }
        }
    }

    /// A trip inside the trip. `false`: there is no way there.
    async fn walk_to(&mut self, cx: &mut Cx<'_>, room: RoomId) -> Result<bool, Ended> {
        let mut trip = Trip::seeded(room, cx.trip.draw());
        let mut inner = Cx {
            trip: &mut trip,
            map: cx.map,
            notes: &mut *cx.notes,
            wrote: &mut *cx.wrote,
        };
        match Box::pin(self.walk(&mut inner)).await {
            Ended::Arrived => Ok(true),
            Ended::Failed(_) => Ok(false),
            stopped => Err(stopped),
        }
    }

    /// Until the game says one of these. `false`: it never did.
    async fn await_line(
        &mut self,
        trip: &mut Trip,
        lines: &[String],
        ms: u64,
    ) -> Result<bool, Ended> {
        self.answer.clear();
        let until = Instant::now() + Duration::from_millis(ms);
        loop {
            let said = self.answer.iter().any(|line| {
                let text = line.text();
                lines.iter().any(|wanted| text.contains(wanted.as_str()))
            });
            if said || Instant::now() >= until {
                return Ok(said);
            }
            self.hold(trip).await?;
        }
    }
}
