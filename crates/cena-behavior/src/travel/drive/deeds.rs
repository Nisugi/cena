//! The deeds: what the trip hands the driver because it cannot spell them
//! as a command ([`Deed`]) -- the hands, a cast, a stance, a language, a key,
//! the followers.
//!
//! Moved down out of `drive.rs` (review, 2026-09-23), which the fixes to
//! these very deeds took past its line cap: `plan/05` Rule 4.1, *move code
//! down, do not raise the cap*. Nothing here decides where to walk; it does
//! what it was handed, to completion, and says whether it could.

use std::time::Duration;

use cena_session::{ChunkLine, CommandId, Notice, NoticeKind};
use tokio::time::Instant;

use super::super::hands::{cast_commands, store_commands, take_back};
use super::super::kept;
use super::super::routines::casting::{MANA_WAIT_MS, MANA_WAITS, MAX_HINDERED};
use super::super::{Deed, TravelNotes, Trip, walker_from};
use super::{Driver, Ended, FOLLOW_WAIT, Taken};

impl<N: FnMut() -> CommandId> Driver<'_, N> {
    pub(super) async fn deed(&mut self, trip: &mut Trip, deed: Deed) -> Result<(), Ended> {
        match deed {
            Deed::EmptyHands => {
                for (stored, command) in store_commands(&self.state) {
                    // Written down before it is sent: a stop between the two
                    // must still know what to take back.
                    self.stored.push(stored);
                    self.exchange(trip, &command).await?;
                }
            }
            Deed::FillHands => self.fill_hands(trip).await?,
            // `Spell#cast` returns false and the script goes on -- a sigil
            // before a climb that could not be afforded is the climb tried
            // without it -- so a plain cast that did not take does not end
            // the crossing. Hence no `could_not` here.
            Deed::Cast(spell) => {
                self.cast(trip, &spell, None).await?;
            }
            Deed::CastAt(spell, target) => {
                if let Err(why) = self.cast_to_be_carried(trip, &spell, &target).await? {
                    self.handle.say(Notice::line(
                        NoticeKind::Warn,
                        format!("Travel: {why} I will go another way if there is one."),
                    ));
                    trip.could_not();
                }
            }
            Deed::Stance(stance) => {
                // The first one replaced is the one to go back to.
                if self.stance_before.is_none() {
                    self.stance_before.clone_from(&self.state.character.stance);
                }
                self.exchange(trip, &format!("stance {stance}")).await?;
            }
            Deed::RestoreStance => {
                if let Some(before) = self.stance_before.take() {
                    self.exchange(trip, &format!("stance {before}")).await?;
                }
            }
            Deed::AwaitFollowers => self.await_followers(trip).await?,
            Deed::Speak(language) => {
                self.exchange(trip, "speak").await?;
                let speaking = kept::language_in(&self.answer);
                if !speaking
                    .as_deref()
                    .is_some_and(|is| kept::is_spoken(&language, is))
                {
                    // The first one replaced is the one to go back to.
                    if self.speech_before.is_none() {
                        self.speech_before = speaking;
                    }
                    self.exchange(trip, &format!("speak {language}")).await?;
                }
            }
            Deed::RestoreSpeech => {
                if let Some(before) = self.speech_before.take() {
                    self.exchange(trip, &format!("speak {before}")).await?;
                }
            }
            Deed::TakeOut(name) => {
                self.exchange(trip, &format!("get my {name}")).await?;
                self.taken = kept::taken_from(&self.answer).map(|(thing, container)| Taken {
                    thing,
                    container,
                    name,
                });
                if self.taken.is_none() {
                    trip.could_not();
                }
            }
            Deed::PutBack => {
                // Still recorded while the command is out: a stop in between
                // must still say where the key is.
                if let Some(taken) = &self.taken {
                    let command = format!("put #{} in #{}", taken.thing, taken.container);
                    self.exchange(trip, &command).await?;
                    self.taken = None;
                }
            }
            // Handled where the notes are: `walk`.
            Deed::Remember(..) | Deed::Forget(_) => {}
        }
        Ok(())
    }

    /// Take back what [`Deed::EmptyHands`] stored: last stored, first back,
    /// one command each (`hands`).
    ///
    /// **Nothing leaves [`Self::stored`] until it is seen back in a hand.**
    /// This took the whole list out first and put each back as it failed, so
    /// an error on the first -- a stop, a drop -- left every item after it off
    /// the record: [`Self::take_back_once`] never tried them, and neither
    /// `still_stored` nor the player's notice named them (review, 2026-09-23).
    async fn fill_hands(&mut self, trip: &mut Trip) -> Result<(), Ended> {
        let mut untried = std::mem::take(&mut self.stored);
        let mut missed = Vec::new();
        let mut asked = Ok(());
        while let Some(stored) = untried.pop() {
            let command = take_back(&self.state, &stored);
            asked = self.exchange(trip, &command).await;
            if self.state.hand_holding(&stored.id).is_none() {
                missed.push(stored);
            }
            if asked.is_err() {
                break;
            }
        }
        // In the order they were stored, so a later take-back is last-first too.
        missed.reverse();
        untried.extend(missed);
        self.stored = untried;
        asked
    }

    /// Cast `spell`, at `at` if there is one. Each command is sent by
    /// [`Self::put`], Lich's `fput`, which sends again when the game answers
    /// with roundtime: a cast refused for roundtime has not been cast, and
    /// this used to send it once, take the refusal for the answer, and count
    /// the deed done (review, 2026-09-23). Returns what the last command was
    /// answered with.
    async fn cast(
        &mut self,
        trip: &mut Trip,
        spell: &str,
        at: Option<&str>,
    ) -> Result<Vec<ChunkLine>, Ended> {
        let Some(commands) = cast_commands(spell, at) else {
            return Err(Ended::UnknownSpell);
        };
        let mut answer = Vec::new();
        for command in commands {
            answer = self.put(trip, &command).await?;
        }
        Ok(answer)
    }

    /// [`Deed::CastAt`]: Phase at an insignia. Upstream, from the map's own
    /// procs (`reference/mapdb/mapdb.json`, room 18926's way on):
    ///
    /// ```ruby
    /// loop { wait_until { Spell[704].affordable? }; result = cast(704, 'insignia');
    ///        break unless result =~ /Spell Hindrance/ }
    /// ```
    ///
    /// So: wait for the mana, cast, and cast again while armour hinders it.
    /// Both of upstream's waits are unbounded; these take the bounds
    /// `routines::casting` already gives the same loop -- [`MANA_WAITS`]
    /// waits of [`MANA_WAIT_MS`] (ten minutes), and [`MAX_HINDERED`] casts.
    /// `Err` says why it gave up, for the player, and the step is then failed
    /// at once rather than left waiting to be carried by a spell that never
    /// went off.
    ///
    /// A gauge the game has not stated counts as affordable, as it does in
    /// `casting`: one refused `prepare` at worst.
    async fn cast_to_be_carried(
        &mut self,
        trip: &mut Trip,
        spell: &str,
        at: &str,
    ) -> Result<Result<(), String>, Ended> {
        let name = cena_session::spell_named(spell).map_or(spell, |found| found.name.as_str());
        for _ in 0..MAX_HINDERED {
            let mut waits = 0;
            while !self.affords(name) {
                waits += 1;
                if waits > MANA_WAITS {
                    return Ok(Err(format!(
                        "I waited ten minutes for the mana to cast {name}, and it never came."
                    )));
                }
                let until = Instant::now() + Duration::from_millis(MANA_WAIT_MS);
                while Instant::now() < until {
                    self.hold(trip).await?;
                }
            }
            let answer = self.cast(trip, spell, Some(at)).await?;
            if !answer
                .iter()
                .any(|line| line.text().contains("[Spell Hindrance"))
            {
                return Ok(Ok(()));
            }
        }
        Ok(Err(format!(
            "your armour hindered {name} {MAX_HINDERED} times in a row."
        )))
    }

    /// Lich's `Spell#affordable?`, as the walker's facts have it. Not known
    /// is affordable (`casting`'s rule).
    fn affords(&self, name: &str) -> bool {
        let server = self.state.game_time_now().unwrap_or(0);
        walker_from(&self.state, &TravelNotes::default(), server)
            .affordable_spells
            .is_none_or(|affordable| affordable.contains(name))
    }

    /// Until everyone who set out with the walker has rejoined it, or
    /// [`FOLLOW_WAIT`].
    ///
    /// Upstream's loop, from the six crossings that have it (`keys.rs`,
    /// `with_company`): a ladder or a bridge does not carry a group, so each
    /// follower crosses alone and the leader waits for
    /// `X joins your group.` or `You reach out and hold X's hand.`, striking
    /// each name as it comes, until none is left.
    ///
    /// Who is waited for is **the group, as the model has it now** -- the
    /// port of Lich's `Group` (author, 2026-09-21: *"we don't use
    /// `$group_members` ... we use what we ported from the Group module"*).
    /// Upstream's list is a global nothing in any reference sets; the group
    /// is what it stood for. Asked here and not when the trip set out, so
    /// someone who joined on the road is waited for too.
    ///
    /// Upstream's only way out of a follower who never comes is typing `go`;
    /// here it is the clock, and a stop.
    ///
    /// Listening starts now, as upstream's `clear` has it: a rejoining heard
    /// before the crossing is not one after it.
    async fn await_followers(&mut self, trip: &mut Trip) -> Result<(), Ended> {
        let group = self.state.group.members();
        self.behind = group.iter().map(|member| member.id.clone()).collect();
        let until = Instant::now() + FOLLOW_WAIT;
        while !self.behind.is_empty() && Instant::now() < until {
            self.hold(trip).await?;
        }
        self.behind.clear();
        Ok(())
    }
}
