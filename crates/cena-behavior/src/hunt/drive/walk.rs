//! The hunt driver's walks: travel's own driver run under the hunt's
//! authority, with the hunt folding its own stream meanwhile (the module
//! doc of `drive.rs`, "A walk inside the hunt").

use cena_map::RoomId;
use cena_session::{CommandId, Event, Notice, NoticeKind, Snapshot, State};
use tokio::sync::broadcast::error::RecvError;

use super::{Driver, HuntEnd, fold_into};
use crate::error::BehaviorError;
use crate::hunt::engine::Hunt;
use crate::hunt::said::Phase;
use crate::travel::{Ended, TravelNotes, travel_holding};

impl<F: FnMut() -> CommandId, W: FnMut(&TravelNotes), L: FnMut(&[String])> Driver<'_, F, W, L> {
    /// Walk to `to` with travel's driver under this authority, folding this
    /// hunt's own stream meanwhile.
    pub(super) async fn walk(&mut self, to: RoomId) -> Result<(), HuntEnd> {
        let snapshot = Snapshot {
            session: self.session,
            state: self.state.clone(),
            lifecycle: self.lifecycle,
            generation: self.generation,
            cursor: self.cursor,
            retry: None,
        };
        let listener = self.events.resubscribe();
        let mut notes = std::mem::take(&mut self.notes);
        let mut dropped = false;
        let walk_cancel = self.cancel.child_token();
        let field_trip = self.machine.field_rest;
        let mut redirect_to_town = false;
        let travelled = {
            let handle = self.handle;
            let cancel = &walk_cancel;
            let token = self.token;
            let map = if self.machine.phase() == Phase::Hunting {
                self.hunting_map.as_ref().unwrap_or(self.map)
            } else {
                self.map
            };
            let next_id = &mut self.next_id;
            let mut walk = Box::pin(travel_holding(
                handle,
                cancel,
                next_id,
                token,
                (snapshot, listener),
                map,
                to,
                &mut notes,
                |_| {},
            ));
            // The walk holds `next_id`; this loop touches only the stream
            // and the state, so both borrows stand.
            let events = &mut self.events;
            let state = &mut self.state;
            let dropped = &mut dropped;
            loop {
                let event = tokio::select! {
                    biased;
                    travelled = &mut walk => break travelled,
                    event = events.recv() => event,
                };
                match event {
                    Ok(event) => {
                        if matches!(event, Event::StateChanged(State::Reconnecting)) {
                            *dropped = true;
                        }
                        if let Err(gone) = fold_into(state, &event) {
                            return Err(HuntEnd::Stopped(gone));
                        }
                        if field_trip && !Hunt::field_eligible(state) {
                            // Stop this walk through travel's cancellation path,
                            // preserving the hunt's authority and latest room.
                            // Its next tick selects town and drops field commands.
                            redirect_to_town = true;
                            walk_cancel.cancel();
                        }
                    }
                    Err(RecvError::Lagged(_)) => {}
                    Err(RecvError::Closed) => return Err(HuntEnd::Stopped(BehaviorError::Dead)),
                }
            }
        };
        self.notes = notes;
        (self.wrote)(&self.notes);
        if let Some(room) = travelled.last_room {
            self.last_room = Some(room);
        }
        if dropped || matches!(travelled.ended, Ended::Stopped(BehaviorError::Disconnected)) {
            // The walk stopped for the drop; the next tick walks again once
            // the session is back.
            self.link_lost();
            return Ok(());
        }
        match travelled.ended {
            Ended::Arrived => Ok(()),
            Ended::Stopped(BehaviorError::Cancelled)
                if redirect_to_town && !self.cancel.is_cancelled() =>
            {
                Ok(())
            }
            Ended::Stopped(why) => Err(HuntEnd::Stopped(why)),
            other => {
                self.handle.say(Notice::line(
                    NoticeKind::Warn,
                    format!("Hunt: could not walk to room {}: {other:?}.", to.0),
                ));
                match self.machine.walk_failed(to) {
                    Some(ending) => Err(HuntEnd::Finished(ending)),
                    None => Ok(()),
                }
            }
        }
    }
}
