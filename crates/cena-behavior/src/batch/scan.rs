//! `;foreach`'s first half: looking in each target and gathering what is
//! there (`reference/scripts/scripts/foreach.lic:1771-1886`, and
//! `ItemMatcher#ingest_from_name`, `:524-575`).
//!
//! A named target is looked in -- `look in <name>`, or `on`, `under`,
//! `behind` -- quietly, and its answer read for the container's id; what is
//! in it is what the game then listed for that id. The ground needs no
//! look: the room already says what is on it. `loot` looks in each thing on
//! the ground, and a thing that is not a container gives nothing, silently,
//! as Lich's scan of the room did (`ingest_loot`, `:463-466`).

use cena_session::NoticeKind;

use super::drive::{Driver, Halt};
use super::foreach::{Foreach, Position, Target};
use super::pick::{Candidate, Group, Looked, Place, contents, looked};

/// What to do when a target is closed, or not there.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Missing {
    /// Stop the run: the player named it.
    Stop,
    /// Say it was skipped: the player named it with a `?`.
    Skip,
    /// Skip it and say nothing: one of `loot`'s things.
    Quiet,
}

impl Driver<'_> {
    /// Every target's items, in order, before any filter.
    pub(super) async fn scan(&mut self, foreach: &Foreach) -> Result<Vec<Group>, Halt> {
        let mut groups = Vec::new();
        for target in &foreach.targets {
            match target {
                Target::Ground => groups.push(Group {
                    place: Place::Ground,
                    items: self.state.room.objects.iter().map(Candidate::of).collect(),
                }),
                Target::Loot => {
                    let things: Vec<String> = self
                        .state
                        .room
                        .objects
                        .iter()
                        .map(|o| o.id.clone())
                        .collect();
                    for id in things {
                        let found = self
                            .look_into(&format!("#{id}"), Position::In, Missing::Quiet)
                            .await?;
                        groups.extend(found);
                    }
                }
                Target::Named { name, optional } => {
                    let missing = if *optional {
                        Missing::Skip
                    } else {
                        Missing::Stop
                    };
                    let found = self.look_into(name, foreach.position, missing).await?;
                    groups.extend(found);
                }
            }
        }
        Ok(groups)
    }

    /// Look in one target and gather what the game listed for it.
    async fn look_into(
        &mut self,
        target: &str,
        position: Position,
        missing: Missing,
    ) -> Result<Option<Group>, Halt> {
        let answer = self
            .look(&format!("look {} {target}", position.word()))
            .await?;
        let why = match looked(&answer) {
            Looked::Container { id, name } => {
                if let Some(items) = contents(&self.state, &id) {
                    return Ok(Some(Group {
                        items: items.iter().map(Candidate::of).collect(),
                        place: Place::Container { id, name },
                    }));
                }
                "the game listed nothing for it"
            }
            Looked::Empty => {
                if missing != Missing::Quiet {
                    self.say(NoticeKind::Info, &format!("'{target}' is empty."));
                }
                return Ok(None);
            }
            Looked::Closed => "it is closed",
            Looked::NotFound => "it was not found",
            Looked::Unread => "the game's answer to the look was not one foreach knows",
        };
        match missing {
            Missing::Stop => Err(Halt::Failed(format!(
                "'{target}': {why}. Nothing was done; `{target}?` skips it."
            ))),
            Missing::Skip => {
                self.say(NoticeKind::Warn, &format!("skipping '{target}?': {why}."));
                Ok(None)
            }
            Missing::Quiet => Ok(None),
        }
    }
}
