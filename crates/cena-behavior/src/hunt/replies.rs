//! What the game answered a line the hunt sent, and what the hunt does
//! about it (`inventory/12` §2: attack, injury and cast refusals).
//!
//! The write-time gate ([`cena_session::Gate::Act`]) refuses a step whose
//! target is gone before the bytes leave, and cannot see a kill landing in
//! flight, a sanctuary, an arm too hurt to swing or an arrow that does
//! nothing. The game says each of those in words, after the line is sent.
//! The driver hands the reply's lines here ([`Hunt::replied`]); the next
//! tick decides with them.
//!
//! | Reply | Source | The hunt |
//! |---|---|---|
//! | [`Reply::NoTarget`] | `bigshot.lic:5541`, `combo.lic:245`, `spell.rb:40,57` | the target is gone: choose again |
//! | [`Reply::Injured`] | `spell.rb:26-33`, `health.lic:63-85` | rest to heal; a second time after resting for it, the hunt ends |
//! | [`Reply::NoEffect`] | `bigshot.lic:6398` | the hunt ends: the weapon or ammunition cannot hurt what is here |
//! | [`Reply::Sanctuary`] | `spell.rb:42-43` | no fighting in this room: wander on |
//! | [`Reply::Unwilling`] | `bigshot.lic:5537` | wait, and try again |
//! | [`Reply::Rooted`] | `bigshot.lic:5534-5536` | wait, and try again |
//! | [`Reply::NoMana`] | `spell.rb:29,36` | rest for mana |
//!
//! bigshot rests on an attack with no effect (`$bigshot_should_rest`);
//! here it ends the hunt, because a rest does not bless an arrow and the
//! hunt would walk back to the same refusal.

use std::collections::BTreeSet;

use super::engine::Hunt;
use super::said::{Ending, Why};

/// Seconds to wait after a refusal the next moment may clear.
const PAUSE: u32 = 3;

/// One reply the hunt acts on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reply {
    /// Nothing to hit: killed by someone else, gone, or never targeted.
    NoTarget,
    /// An injury stops the action: a hand, an arm, the head, the throat.
    Injured,
    /// The attack landed and did nothing: the wrong weapon or ammunition.
    NoEffect,
    /// A sanctuary: no spells of war here.
    Sanctuary,
    /// Calmed: `You are unable to muster the will to attack anything.`
    Unwilling,
    /// Rooted in place: `You don't seem to be able to move to do that.`
    Rooted,
    /// `But you don't have any mana!`
    NoMana,
    /// `You spy a ... and recover it!`: a disarmed weapon is back.
    Recovered,
    /// `You have deducted 500 experience points from your field
    /// experience`: a Long-Term Experience Boost spent.
    Boosted,
    /// `You do not have any Long-Term Experience Boosts to redeem.`
    NoBoosts,
    /// The part aimed at cannot be hit: `You cannot aim that high!`, `does
    /// not have a head!`, `is already missing that!` (`bigshot.lic:6560`).
    BadAim,
}

/// Read one line. `None`: nothing the hunt acts on.
#[must_use]
pub fn read(line: &str) -> Option<Reply> {
    let text = line.trim();
    let starts = |prefix: &str| text.starts_with(prefix);
    if starts("You currently have no valid target.")
        || starts("You do not currently have a target.")
        || starts("It looks like somebody already did the job for you.")
        || starts("What were you referring to?")
        || starts("You spin about but don't see anything to hit!")
        || starts("Cast at what?")
    {
        return Some(Reply::NoTarget);
    }
    if starts("You can't make that dextrous of a move!")
        || starts("You are too injured to make that dextrous of a movement")
        || starts("You can't think clearly enough to prepare a spell!")
        || starts("The searing pain in your throat makes that impossible")
        || starts("All you manage to do is cough up some blood.")
        || starts("You're not in any condition to be searching around!")
    {
        return Some(Reply::Injured);
    }
    if starts("You cannot aim that high!")
        || text.contains("is already missing that!")
        || (text.contains(" does not have a") && text.ends_with('!'))
    {
        return Some(Reply::BadAim);
    }
    if text.contains("You have deducted 500 experience points from your field experience") {
        return Some(Reply::Boosted);
    }
    if starts("You do not have any Long-Term Experience Boosts to redeem.") {
        return Some(Reply::NoBoosts);
    }
    if starts("You spy ") && text.ends_with("and recover it!") {
        return Some(Reply::Recovered);
    }
    if text.contains("but it has no effect") {
        return Some(Reply::NoEffect);
    }
    if starts("Be at peace my child, there is no need for spells of war in here.")
        || text.contains("Spells of War cannot be cast")
    {
        return Some(Reply::Sanctuary);
    }
    if starts("You are unable to muster the will to attack anything.") {
        return Some(Reply::Unwilling);
    }
    if starts("You don't seem to be able to move to do that.")
        || starts("You don't seem to be able to move your legs to do that.")
    {
        return Some(Reply::Rooted);
    }
    if starts("But you don't have any mana!") {
        return Some(Reply::NoMana);
    }
    None
}

/// What replies taught the hunt, beyond this tick.
#[derive(Debug, Default)]
pub(super) struct Heard {
    /// Rooms the game said are sanctuaries, by the game's number.
    pub(super) sanctuaries: BTreeSet<String>,
    /// A rest for an injury refusal finished, and nothing has died since.
    pub(super) rested_for_injury: bool,
    /// No attack before this game second.
    pub(super) paused_until: Option<u32>,
    /// The hunt is over, said at the next tick.
    pub(super) ending: Option<Ending>,
    /// The game said a `flee.messages` phrase: leave at the next tick.
    pub(super) flee_said: bool,
}

impl Hunt {
    /// The lines the game answered the last line with, at game second
    /// `now`. What they say is acted on at the next tick.
    pub fn replied<'a>(&mut self, lines: impl IntoIterator<Item = &'a str>, now: Option<u32>) {
        let lines: Vec<&str> = lines.into_iter().collect();
        self.wand_replied(&lines);
        self.ammo_replied(&lines);
        self.boons_replied(&lines);
        self.force_replied(&lines);
        self.resend_replied(&lines);
        self.bounty_replied(&lines);
        let replies: Vec<Reply> = lines.iter().copied().filter_map(read).collect();
        // A boost the game answered with neither of its lines is not tried
        // again: treated as none left.
        if std::mem::take(&mut self.boosts.1)
            && !replies
                .iter()
                .any(|r| matches!(r, Reply::Boosted | Reply::NoBoosts))
        {
            self.boosts.0 = self.profile.rest.lte_boost;
        }
        for reply in replies {
            match reply {
                Reply::NoTarget => self.target_gone(),
                Reply::Injured if self.heard.rested_for_injury => {
                    self.heard.ending = Some(Ending::Injured);
                }
                Reply::Injured => self.must_rest = Some(Why::Injured),
                Reply::NoEffect => self.heard.ending = Some(Ending::NoEffect),
                Reply::Sanctuary => {
                    if let Some(room) = self.room.clone() {
                        self.notes
                            .push(format!("room {room} is a sanctuary: moving on."));
                        self.heard.sanctuaries.insert(room);
                    }
                    self.target_gone();
                }
                Reply::Unwilling | Reply::Rooted => {
                    self.heard.paused_until = now.map(|now| now + PAUSE);
                }
                Reply::NoMana => self.must_rest = Some(Why::Mana),
                Reply::Recovered => self.recovered(),
                Reply::BadAim => self.aiming.refused(),
                Reply::Boosted => {
                    self.boosts.0 += 1;
                    self.fried_kills = 0;
                }
                Reply::NoBoosts => self.boosts.0 = self.profile.rest.lte_boost,
            }
        }
        // Last: a reply that also forgot the target (`What were you
        // referring to?`) has cleared the lines queued, and the answer's line
        // goes after that (`hunt/follow.rs`).
        self.follow_replied(&lines);
    }

    /// Whether the room the hunt is in is one the game said is a sanctuary.
    pub(super) fn in_sanctuary(&self) -> bool {
        self.room
            .as_ref()
            .is_some_and(|room| self.heard.sanctuaries.contains(room))
    }

    /// Whether a refusal asked for a moment before the next attack.
    pub(super) fn paused(&self, now: Option<u32>) -> bool {
        self.heard
            .paused_until
            .zip(now)
            .is_some_and(|(until, now)| now < until)
    }
}
