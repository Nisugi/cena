//! `Errand::SwordInTheGorge`: the gorge's rim, reached through a pool by
//! prying a gap with a short sword.
//!
//! Upstream (`errand_sword_gorge.rb`): empty the hands; find a `short sword`
//! in the walker's containers and `_drag` it to the right hand, or -- with
//! `$go2_get_silvers` on -- buy one (`withdraw 200`, `weaponshop`, `order 3`)
//! and come back to the room the errand began in. Cast Sigil of Resolve if it
//! is known, affordable and not running. Then, until the pry lands the walker
//! somewhere that is not the pool's floor (room 22229): `go pool`, `swim
//! down`, `kneel`, wait half a second, `pry gap`; a sword that "tumbles out
//! of your hand" is got back with `recover item` before prying again; `swim
//! up`, `go shore`, and round again. Out: stand, put the sword back where it
//! came from (else the stow container, else each container in turn but a
//! locket), close what was opened, fill the hands. Upstream sets
//! `$go2_restart`; here that is [`Next::Done`], as for every routine.
//!
//! Deviations:
//! - **The dives are bounded** at [`MAX_DIVES`] and the stands at
//!   [`MAX_STANDS`]; upstream's loops are not. Past the bound the sword is
//!   still put away and the exit given up.
//! - Upstream kills the `disarmed` script for the crossing. There is no such
//!   script here.
//! - The sword put away is whichever a hand holds by the noun `sword`: the
//!   hands were emptied, so it is the one. Upstream tracks an id it never
//!   sets for a bought sword and falls back to the same thing.
//! - The withdrawal is checked, and a sword not in hand after `buy` stops the
//!   trip; upstream walks on to pry with nothing. See `shopping`.

use cena_map::{Action, Cond, RoomId, Step};

use super::shopping::{BankRun, Found, Purchase, Ran, Search, held, may_buy, step};
use super::{Next, Seen, Solver};

/// The pool's floor: prying has worked when the walker is anywhere else.
const POOL_FLOOR: RoomId = RoomId(22229);
/// Upstream's room for the exit, for a walker the map could not place.
const GORGE: RoomId = RoomId(6955);
/// Dives before the exit is given up: the walker's stop, not an estimate.
pub(super) const MAX_DIVES: u32 = 20;
const MAX_STANDS: u32 = 5;
const RESOLVE: &str = "Sigil of Resolve";

const NO_SWORD: &str = "You have no short sword to pry the gap with. Buy one, or let Travel \
                        fetch silver and buy it by turning the get_silvers setting on.";
const POOR: &str = "Too poor to cross the gorge: the bank would not give 200 silvers for a \
                    short sword.";
const NOT_SOLD: &str = "The weaponshop did not sell a short sword: there is none in hand.";

const SWORD: Purchase = Purchase {
    withdraw: "withdraw 200",
    shop: "weaponshop",
    order: "order 3",
    sold: None,
    poor: POOR,
    laden: NOT_SOLD,
};

pub(super) struct SwordGorge {
    at: At,
    search: Search,
    /// Where the sword came from, to go back to.
    home: Option<String>,
    dives: u32,
    recovering: bool,
    crossed: bool,
}

enum At {
    Begin,
    Searching,
    Buying(BankRun),
    Bought,
    Resolve,
    Pool,
    SwimDown,
    Kneel,
    Settle,
    Pry,
    Pried,
    Recovered,
    SwimUp,
    Shore,
    Stand(u32),
    /// Putting the sword away: the next of the search's containers to try.
    Stow(usize),
    Close,
    End,
}

impl SwordGorge {
    pub fn new() -> Self {
        SwordGorge {
            at: At::Begin,
            search: Search::new(vec!["short sword"]),
            home: None,
            dives: 0,
            recovering: false,
            crossed: false,
        }
    }

    fn put(&mut self, command: &str, then: At) -> Next {
        self.at = then;
        Next::Put(command.to_owned())
    }

    fn searching(&mut self, seen: &Seen<'_>) -> Next {
        match self.search.next(seen) {
            Found::Ask(next) => next,
            Found::Item(sword) => {
                self.home = sword.within;
                self.put(&format!("_drag #{} right", sword.id), At::Resolve)
            }
            Found::Nothing if may_buy(seen) => {
                let back = seen.here.unwrap_or(GORGE);
                self.at = At::Buying(BankRun::new(SWORD, back));
                self.next(seen)
            }
            Found::Nothing => {
                self.at = At::Close;
                self.next(seen)
            }
        }
    }

    /// One turn of upstream's loop, from the answer to `pry gap` on.
    fn pried(&mut self, seen: &Seen<'_>) -> Next {
        if seen.answered("tumbles out of your hand") {
            self.recovering = true;
        }
        if seen.here != Some(POOL_FLOOR) {
            self.crossed = true;
            self.at = At::Stand(0);
            return self.next(seen);
        }
        self.recover_or_rise()
    }

    fn recover_or_rise(&mut self) -> Next {
        if self.recovering {
            return self.put("recover item", At::Recovered);
        }
        self.put("swim up", At::SwimUp)
    }

    fn stow(&mut self, seen: &Seen<'_>, tried: usize) -> Next {
        let Some(sword) = held(seen, "sword") else {
            self.at = At::Close;
            return self.next(seen);
        };
        let stow = seen
            .state
            .containers
            .stow(cena_session::containers::StowSlot::Default)
            .map(|item| item.id.clone());
        // The place it came from; else the stow container, then the rest.
        let into = match &self.home {
            Some(home) => (tried == 0).then(|| home.clone()),
            None => stow
                .into_iter()
                .chain(
                    self.search
                        .containers
                        .iter()
                        .filter(|(_, noun)| noun != "locket")
                        .map(|(id, _)| id.clone()),
                )
                .nth(tried),
        };
        if let Some(into) = into {
            return self.put(&format!("_drag #{sword} #{into}"), At::Stow(tried + 1));
        }
        self.at = At::Close;
        self.next(seen)
    }
}

impl Solver for SwordGorge {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        match &mut self.at {
            At::Begin => {
                self.at = At::Searching;
                Next::Steps(vec![step(Action::EmptyHands)])
            }
            At::Searching => self.searching(seen),
            At::Buying(run) => match run.next(seen) {
                Ran::Ask(next) => next,
                Ran::Finished => {
                    self.at = At::Bought;
                    self.next(seen)
                }
            },
            At::Bought if held(seen, "sword").is_none() => Next::Stop(NOT_SOLD.to_owned()),
            At::Bought | At::Resolve => {
                self.at = At::Pool;
                let resolve = || RESOLVE.to_owned();
                let can_cast = Cond::All(vec![
                    Cond::SpellKnown(resolve()),
                    Cond::SpellAffordable(resolve()),
                    Cond::Not(Box::new(Cond::SpellActive(resolve()))),
                ]);
                Next::Steps(vec![Step {
                    action: Action::Cast(resolve()),
                    when: Some(can_cast),
                }])
            }
            At::Pool if self.dives >= MAX_DIVES => {
                self.at = At::Stand(0);
                self.next(seen)
            }
            At::Pool => {
                self.dives += 1;
                self.at = At::SwimDown;
                Next::Go("go pool".to_owned())
            }
            At::SwimDown => self.put("swim down", At::Kneel),
            At::Kneel => self.put("kneel", At::Settle),
            At::Settle => {
                self.at = At::Pry;
                Next::Pause(500)
            }
            At::Pry if self.recovering => self.recover_or_rise(),
            At::Pry => self.put("pry gap", At::Pried),
            At::Pried => self.pried(seen),
            At::Recovered => {
                if seen.answered("and recover it") {
                    self.recovering = false;
                }
                self.put("swim up", At::SwimUp)
            }
            At::SwimUp => self.put("go shore", At::Shore),
            At::Shore => {
                self.at = At::Pool;
                self.next(seen)
            }
            At::Stand(stood) => {
                let stood = *stood;
                let down = seen
                    .walker
                    .posture
                    .as_deref()
                    .is_some_and(|is| is != "standing");
                if down && stood < MAX_STANDS {
                    return self.put("stand", At::Stand(stood + 1));
                }
                self.stow(seen, 0)
            }
            At::Stow(tried) => {
                let tried = *tried;
                self.stow(seen, tried)
            }
            At::Close => {
                if let Some(close) = self.search.close() {
                    return close;
                }
                self.at = At::End;
                Next::Steps(vec![step(Action::FillHands)])
            }
            At::End if self.crossed => Next::Done,
            At::End if self.dives == 0 => Next::Stop(NO_SWORD.to_owned()),
            At::End => Next::Failed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::shopping::scenes::{buying, holding, line};
    use super::super::testing::Scene;
    use super::*;

    fn put(command: &str) -> Next {
        Next::Put(command.into())
    }

    fn fill() -> Next {
        Next::Steps(vec![step(Action::FillHands)])
    }

    fn shore() -> Scene {
        Scene::at(6955, 1)
    }

    fn floor() -> Scene {
        Scene::at(22229, 1)
    }

    /// A sword found in pack 11, dragged out, and the sigil asked for.
    fn armed() -> SwordGorge {
        let mut gorge = SwordGorge::new();
        assert_eq!(
            shore().ask(&mut gorge),
            Next::Steps(vec![step(Action::EmptyHands)])
        );
        assert_eq!(shore().ask(&mut gorge), put("inventory containers"));
        let mut listed = shore();
        listed.answer = vec![line(
            "You are wearing ",
            &[("9", "locket", "locket"), ("11", "pack", "pack")],
        )];
        assert_eq!(listed.ask(&mut gorge), put("look in #9"));
        assert_eq!(shore().ask(&mut gorge), put("look in #11"));
        let mut inside = shore();
        inside.answer = vec![line("In the pack: ", &[("5", "sword", "short sword")])];
        assert_eq!(inside.ask(&mut gorge), put("_drag #5 right"));
        let resolve = || "Sigil of Resolve".to_owned();
        let when = Cond::All(vec![
            Cond::SpellKnown(resolve()),
            Cond::SpellAffordable(resolve()),
            Cond::Not(Box::new(Cond::SpellActive(resolve()))),
        ]);
        assert_eq!(
            shore().ask(&mut gorge),
            Next::Steps(vec![Step {
                action: Action::Cast(resolve()),
                when: Some(when),
            }])
        );
        gorge
    }

    /// From the shore to the pry.
    fn dive(gorge: &mut SwordGorge) {
        assert_eq!(shore().ask(gorge), Next::Go("go pool".into()));
        assert_eq!(shore().ask(gorge), put("swim down"));
        assert_eq!(floor().ask(gorge), put("kneel"));
        assert_eq!(floor().ask(gorge), Next::Pause(500));
    }

    #[test]
    fn a_sword_already_carried_pries_the_gap_and_goes_home() {
        let mut gorge = armed();
        dive(&mut gorge);
        assert_eq!(floor().ask(&mut gorge), put("pry gap"));
        // Spat out on the rim, kneeling, sword in hand.
        let mut rim = holding(Scene::at(1, 1), "5", "sword", "short sword");
        rim.walker.posture = Some("kneeling".into());
        let rim = rim.answered(&["you are spit out"]);
        assert_eq!(rim.ask(&mut gorge), put("stand"));
        let rim = || holding(Scene::at(1, 1), "5", "sword", "short sword");
        assert_eq!(rim().ask(&mut gorge), put("_drag #5 #11"));
        // Put away: nothing more is tried.
        assert_eq!(Scene::at(1, 1).ask(&mut gorge), fill());
        assert_eq!(Scene::at(1, 1).ask(&mut gorge), Next::Done);
    }

    #[test]
    fn a_pry_that_does_not_take_goes_up_and_round_again() {
        let mut gorge = armed();
        dive(&mut gorge);
        assert_eq!(floor().ask(&mut gorge), put("pry gap"));
        let weak = floor().answered(&["You can't get good enough leverage."]);
        assert_eq!(weak.ask(&mut gorge), put("swim up"));
        assert_eq!(shore().ask(&mut gorge), put("go shore"));
        assert_eq!(shore().ask(&mut gorge), Next::Go("go pool".into()));
    }

    #[test]
    fn a_dropped_sword_is_recovered_before_it_is_pried_with_again() {
        let mut gorge = armed();
        dive(&mut gorge);
        assert_eq!(floor().ask(&mut gorge), put("pry gap"));
        let dropped = floor().answered(&["The sword tumbles out of your hand!"]);
        assert_eq!(dropped.ask(&mut gorge), put("recover item"));
        let nothing = floor().answered(&["There is nothing recoverable here."]);
        assert_eq!(nothing.ask(&mut gorge), put("swim up"));
        assert_eq!(shore().ask(&mut gorge), put("go shore"));
        // Still without it: down again, and no pry.
        dive(&mut gorge);
        assert_eq!(floor().ask(&mut gorge), put("recover item"));
        let got = floor().answered(&["You search around and recover it."]);
        assert_eq!(got.ask(&mut gorge), put("swim up"));
        assert_eq!(shore().ask(&mut gorge), put("go shore"));
        dive(&mut gorge);
        assert_eq!(floor().ask(&mut gorge), put("pry gap"));
    }

    /// To the bank and the shop, up to the answer to `buy`.
    fn shop(gorge: &mut SwordGorge, here: impl Fn() -> Scene) {
        assert_eq!(
            here().ask(gorge),
            Next::Steps(vec![step(Action::EmptyHands)])
        );
        assert_eq!(here().ask(gorge), put("inventory containers"));
        assert_eq!(here().ask(gorge), Next::WalkToTag("bank".into()));
        assert_eq!(here().ask(gorge), put("withdraw 200"));
        let paid = here().answered(&["The teller hands you 200 silvers."]);
        assert_eq!(paid.ask(gorge), Next::WalkToTag("weaponshop".into()));
        assert_eq!(here().ask(gorge), put("order 3"));
        assert_eq!(here().ask(gorge), put("buy"));
        assert_eq!(here().ask(gorge), Next::WalkToTag("bank".into()));
        assert_eq!(here().ask(gorge), put("deposit all"));
    }

    #[test]
    fn with_no_sword_one_is_bought_and_the_walker_comes_back_to_where_it_began() {
        let mut gorge = SwordGorge::new();
        shop(&mut gorge, || buying(Scene::at(6900, 1)));
        assert_eq!(
            buying(Scene::at(6900, 1)).ask(&mut gorge),
            Next::WalkTo(RoomId(6900))
        );
        let back = holding(Scene::at(6900, 1), "8", "sword", "short sword");
        assert!(matches!(back.ask(&mut gorge), Next::Steps(_)));
        assert_eq!(shore().ask(&mut gorge), Next::Go("go pool".into()));
    }

    #[test]
    fn a_shop_that_sold_nothing_stops_the_trip() {
        let mut gorge = SwordGorge::new();
        shop(&mut gorge, || buying(shore()));
        buying(shore()).ask(&mut gorge);
        assert_eq!(shore().ask(&mut gorge), Next::Stop(NOT_SOLD.into()));
    }

    #[test]
    fn short_of_silver_stops_the_trip() {
        let mut gorge = SwordGorge::new();
        let here = || buying(shore());
        here().ask(&mut gorge);
        here().ask(&mut gorge);
        here().ask(&mut gorge);
        assert_eq!(here().ask(&mut gorge), put("withdraw 200"));
        let poor = here().answered(&["You don't seem to have that much."]);
        assert_eq!(poor.ask(&mut gorge), Next::Stop(POOR.into()));
    }

    #[test]
    fn a_walker_not_allowed_to_buy_takes_its_hands_back_and_stops() {
        let mut gorge = SwordGorge::new();
        shore().ask(&mut gorge);
        shore().ask(&mut gorge);
        assert_eq!(shore().ask(&mut gorge), fill());
        assert_eq!(shore().ask(&mut gorge), Next::Stop(NO_SWORD.into()));
    }

    #[test]
    fn a_sword_from_nowhere_is_tried_in_each_container_but_a_locket() {
        let mut gorge = armed();
        gorge.home = None;
        dive(&mut gorge);
        floor().ask(&mut gorge);
        let rim = || holding(Scene::at(1, 1), "5", "sword", "short sword");
        // No stow container known: the listed ones, but never the locket.
        assert_eq!(rim().ask(&mut gorge), put("_drag #5 #11"));
        assert_eq!(rim().ask(&mut gorge), fill());
    }

    #[test]
    fn the_dives_are_bounded_and_the_exit_given_up() {
        let mut gorge = armed();
        let mut pools = 0;
        let mut next = shore().ask(&mut gorge);
        for _ in 0..MAX_DIVES * 10 {
            if next == Next::Go("go pool".into()) {
                pools += 1;
            }
            if matches!(next, Next::Failed | Next::Done) {
                break;
            }
            next = floor().ask(&mut gorge);
        }
        assert_eq!(pools, MAX_DIVES);
        assert_eq!(next, Next::Failed);
    }
}
