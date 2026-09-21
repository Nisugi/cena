//! `Errand::GiantToRiversRest` and `Errand::GiantFromRiversRest`: the giant
//! who throws the walker between the kobold village and River's Rest.
//!
//! Upstream (`errand_giant_to_rr.rb`, `errand_giant_from_rr.rb`, which differ
//! only in the table below): empty the hands; find a `crystal amulet` -- or,
//! for a citizen of River's Rest, a `piece of sun-washed driftwood`, which is
//! preferred -- in the walker's containers and `_drag` it to the right hand.
//! With neither, and `$go2_get_silvers` on, buy one (`shopping::BankRun`) and
//! come back; with it off, tell the player and stop. Then `put my … in` the
//! giant's boot or the log, `move` into it, and fill the hands.
//!
//! | | to River's Rest | from River's Rest |
//! |---|---|---|
//! | room | 6274 | 11032 |
//! | put in, and go | `giant's boot` | `log` |
//! | a citizen buys | amulet: 2000, `alchemist`, `order 10` | driftwood: 600, `general store`, `order 11` |
//! | anyone else buys | the same amulet | amulet: 2000, `alchemist`, `order 15` |
//!
//! Deviations:
//! - **The walk back is to the room the errand began in**, and to upstream's
//!   fixed room only when the map could not say where that was. They are the
//!   same room whenever the exit is used as the map has it.
//! - From River's Rest upstream sends `buy` a second time after the amulet is
//!   in hand (`:122`). It buys nothing or a second amulet; it is not sent.
//! - Containers the search opened are closed on the way out as well as on
//!   the refusal; upstream closes them only when it refuses.
//! - How things are found: see `shopping`.

use cena_map::{Action, RoomId};

use super::shopping::{BankRun, Found, Purchase, Ran, Search, may_buy, step};
use super::{Next, Seen, Solver};

const DRIFTWOOD: &str = "piece of sun-washed driftwood";
const AMULET: &str = "crystal amulet";
const NO_AMULET: &str = "You have no crystal amulet for the giant. Buy one, or let Travel \
                         fetch silver and buy it by turning the get_silvers setting on.";

/// Which way the giant throws.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Way {
    ToRiversRest,
    FromRiversRest,
}

struct Table {
    room: RoomId,
    into: &'static str,
    citizen_buys: (Gift, Purchase),
    other_buys: (Gift, Purchase),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gift {
    Driftwood,
    Amulet,
}

const fn amulet(order: &'static str, poor: &'static str, laden: &'static str) -> Purchase {
    Purchase {
        withdraw: "withdraw 2000 silvers",
        shop: "alchemist",
        order,
        sold: Some("hands you a crystal amulet"),
        poor,
        laden,
    }
}

const POOR_TO: &str = "Too poor to go to River's Rest: the bank would not give 2000 silvers.";
const LADEN_TO: &str = "Too encumbered to go to River's Rest: the amulet could not be bought.";
const POOR_FROM: &str = "Too poor to go to Wehnimer's Landing: the bank would not give the silver.";
const LADEN_FROM: &str = "Too encumbered to go to Wehnimer's Landing: the giant's gift could not \
                          be bought.";

fn table(way: Way) -> Table {
    match way {
        Way::ToRiversRest => Table {
            room: RoomId(6274),
            into: "giant's boot",
            citizen_buys: (Gift::Amulet, amulet("order 10", POOR_TO, LADEN_TO)),
            other_buys: (Gift::Amulet, amulet("order 10", POOR_TO, LADEN_TO)),
        },
        Way::FromRiversRest => Table {
            room: RoomId(11032),
            into: "log",
            citizen_buys: (
                Gift::Driftwood,
                Purchase {
                    withdraw: "withdraw 600 silvers",
                    shop: "general store",
                    order: "order 11",
                    sold: Some("hands you a piece of sun-washed driftwood"),
                    poor: POOR_FROM,
                    laden: LADEN_FROM,
                },
            ),
            other_buys: (Gift::Amulet, amulet("order 15", POOR_FROM, LADEN_FROM)),
        },
    }
}

pub(super) struct Giant {
    way: Way,
    at: At,
    search: Option<Search>,
    citizen: bool,
    began: Option<RoomId>,
    gift: Gift,
}

enum At {
    Begin,
    Searching,
    Buying(BankRun),
    Give,
    Enter,
    /// Closing what was opened, then the hands; `true` when it ends in a
    /// refusal rather than a crossing.
    Leaving(bool),
    Ended(bool),
}

impl Giant {
    pub fn new(way: Way) -> Self {
        Giant {
            way,
            at: At::Begin,
            search: None,
            citizen: false,
            began: None,
            gift: Gift::Amulet,
        }
    }

    fn searching(&mut self, seen: &Seen<'_>) -> Next {
        let found = match self.search.as_mut() {
            Some(search) => search.next(seen),
            None => Found::Nothing,
        };
        match found {
            Found::Ask(next) => next,
            Found::Item(item) => {
                // Driftwood is only ever asked for first, and only of a citizen.
                self.gift = match (self.citizen, item.which) {
                    (true, 0) => Gift::Driftwood,
                    _ => Gift::Amulet,
                };
                self.at = At::Give;
                Next::Put(format!("_drag #{} right", item.id))
            }
            Found::Nothing if may_buy(seen) => {
                let table = table(self.way);
                let (gift, purchase) = if self.citizen {
                    table.citizen_buys
                } else {
                    table.other_buys
                };
                self.gift = gift;
                self.at = At::Buying(BankRun::new(purchase, self.began.unwrap_or(table.room)));
                self.next(seen)
            }
            Found::Nothing => {
                self.at = At::Leaving(true);
                self.next(seen)
            }
        }
    }
}

impl Solver for Giant {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        match &mut self.at {
            At::Begin => {
                self.began = seen.here;
                self.citizen = seen.walker.citizenship.as_deref() == Some("River's Rest");
                let mut names = vec![AMULET];
                if self.citizen {
                    names.insert(0, DRIFTWOOD);
                }
                self.search = Some(Search::new(names));
                self.at = At::Searching;
                Next::Steps(vec![step(Action::EmptyHands)])
            }
            At::Searching => self.searching(seen),
            At::Buying(run) => match run.next(seen) {
                Ran::Ask(next) => next,
                Ran::Finished => {
                    self.at = At::Give;
                    self.next(seen)
                }
            },
            At::Give => {
                self.at = At::Enter;
                let mine = match self.gift {
                    Gift::Driftwood => "driftwood",
                    Gift::Amulet => AMULET,
                };
                Next::Put(format!("put my {mine} in {}", table(self.way).into))
            }
            At::Enter => {
                self.at = At::Leaving(false);
                Next::Go(format!("go {}", table(self.way).into))
            }
            At::Leaving(refused) => {
                if let Some(close) = self.search.as_mut().and_then(Search::close) {
                    return close;
                }
                self.at = At::Ended(*refused);
                Next::Steps(vec![step(Action::FillHands)])
            }
            At::Ended(true) => Next::Stop(NO_AMULET.to_owned()),
            At::Ended(false) => Next::Done,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::shopping::MAX_CONTAINERS;
    use super::super::shopping::scenes::{buying, holding, line};
    use super::super::testing::Scene;
    use super::*;

    fn put(command: &str) -> Next {
        Next::Put(command.into())
    }

    fn empty() -> Next {
        Next::Steps(vec![step(Action::EmptyHands)])
    }

    fn fill() -> Next {
        Next::Steps(vec![step(Action::FillHands)])
    }

    fn citizen(mut scene: Scene) -> Scene {
        scene.walker.citizenship = Some("River's Rest".into());
        scene
    }

    /// A scene whose open cloak shows these.
    fn carrying(mut scene: Scene, things: &[(&str, &str, &str)]) -> Scene {
        scene.answer = vec![line("In the cloak: ", things)];
        scene
    }

    /// Through the empty hands and `inventory containers` to the first look.
    fn to_the_cloak(giant: &mut Giant, scene: impl Fn() -> Scene) {
        assert_eq!(scene().ask(giant), empty());
        assert_eq!(scene().ask(giant), put("inventory containers"));
        let mut listed = scene();
        listed.answer = vec![line("You are wearing ", &[("10", "cloak", "cloak")])];
        assert_eq!(listed.ask(giant), put("look in #10"));
    }

    #[test]
    fn an_amulet_already_carried_is_given_with_no_shopping() {
        let mut giant = Giant::new(Way::ToRiversRest);
        let here = || buying(Scene::at(6274, 1));
        to_the_cloak(&mut giant, here);
        let inside = carrying(here(), &[("5", "amulet", "crystal amulet")]);
        assert_eq!(inside.ask(&mut giant), put("_drag #5 right"));
        assert_eq!(
            here().ask(&mut giant),
            put("put my crystal amulet in giant's boot")
        );
        assert_eq!(here().ask(&mut giant), Next::Go("go giant's boot".into()));
        assert_eq!(Scene::at(1, 1).ask(&mut giant), fill());
        assert_eq!(Scene::at(1, 1).ask(&mut giant), Next::Done);
    }

    #[test]
    fn a_citizen_gives_driftwood_before_an_amulet_and_nobody_else_does() {
        let both = [
            ("5", "amulet", "crystal amulet"),
            ("6", "driftwood", DRIFTWOOD),
        ];
        let mut giant = Giant::new(Way::FromRiversRest);
        to_the_cloak(&mut giant, || citizen(Scene::at(11032, 1)));
        assert_eq!(
            carrying(Scene::at(11032, 1), &both).ask(&mut giant),
            put("_drag #6 right")
        );
        assert_eq!(
            Scene::at(11032, 1).ask(&mut giant),
            put("put my driftwood in log")
        );
        assert_eq!(
            Scene::at(11032, 1).ask(&mut giant),
            Next::Go("go log".into())
        );

        let mut giant = Giant::new(Way::FromRiversRest);
        to_the_cloak(&mut giant, || Scene::at(11032, 1));
        assert_eq!(
            carrying(Scene::at(11032, 1), &both).ask(&mut giant),
            put("_drag #5 right")
        );
        assert_eq!(
            Scene::at(11032, 1).ask(&mut giant),
            put("put my crystal amulet in log")
        );
    }

    #[test]
    fn a_thing_the_model_has_is_found_with_nothing_asked() {
        let mut giant = Giant::new(Way::ToRiversRest);
        assert_eq!(Scene::at(6274, 1).ask(&mut giant), empty());
        // Emptying the hands left it in one: an odd table, but the model's word.
        let scene = holding(Scene::at(6274, 1), "77", "amulet", "crystal amulet");
        assert_eq!(scene.ask(&mut giant), put("_drag #77 right"));
    }

    /// From nothing found to the shop's answer, asserting each step.
    fn shop(giant: &mut Giant, here: impl Fn() -> Scene, buy: [&str; 3]) {
        to_the_cloak(giant, &here);
        let nothing = here().answered(&["There is nothing in there."]);
        assert_eq!(nothing.ask(giant), Next::WalkToTag("bank".into()));
        assert_eq!(here().ask(giant), put(buy[0]));
        let paid = here().answered(&["The teller hands you some silvers."]);
        assert_eq!(paid.ask(giant), Next::WalkToTag(buy[1].into()));
        assert_eq!(here().ask(giant), put(buy[2]));
        assert_eq!(here().ask(giant), put("buy"));
    }

    #[test]
    fn with_no_amulet_one_is_bought_and_the_walker_comes_back_to_where_it_began() {
        let mut giant = Giant::new(Way::ToRiversRest);
        // Begun somewhere that is not upstream's fixed room, to tell them apart.
        let here = || buying(Scene::at(6000, 1));
        shop(
            &mut giant,
            here,
            ["withdraw 2000 silvers", "alchemist", "order 10"],
        );
        let sold = here().answered(&["The alchemist hands you a crystal amulet."]);
        assert_eq!(sold.ask(&mut giant), Next::WalkToTag("bank".into()));
        assert_eq!(here().ask(&mut giant), put("deposit all"));
        assert_eq!(here().ask(&mut giant), Next::WalkTo(RoomId(6000)));
        assert_eq!(
            here().ask(&mut giant),
            put("put my crystal amulet in giant's boot")
        );
        assert_eq!(here().ask(&mut giant), Next::Go("go giant's boot".into()));
    }

    #[test]
    fn from_rivers_rest_a_citizen_buys_driftwood_and_anyone_else_an_amulet() {
        let mut giant = Giant::new(Way::FromRiversRest);
        let here = || citizen(buying(Scene::at(11032, 1)));
        shop(
            &mut giant,
            here,
            ["withdraw 600 silvers", "general store", "order 11"],
        );
        let sold = here().answered(&["He hands you a piece of sun-washed driftwood."]);
        assert_eq!(sold.ask(&mut giant), Next::WalkToTag("bank".into()));
        assert_eq!(here().ask(&mut giant), put("deposit all"));
        assert_eq!(here().ask(&mut giant), Next::WalkTo(RoomId(11032)));
        assert_eq!(here().ask(&mut giant), put("put my driftwood in log"));

        let mut giant = Giant::new(Way::FromRiversRest);
        shop(
            &mut giant,
            || buying(Scene::at(11032, 1)),
            ["withdraw 2000 silvers", "alchemist", "order 15"],
        );
    }

    #[test]
    fn a_walker_with_no_room_walks_back_to_upstreams() {
        let mut giant = Giant::new(Way::ToRiversRest);
        let mut nowhere = buying(Scene::at(1, 1));
        nowhere.here = None;
        assert_eq!(nowhere.ask(&mut giant), empty());
        let here = || buying(Scene::at(2, 1));
        assert_eq!(here().ask(&mut giant), put("inventory containers"));
        assert_eq!(here().ask(&mut giant), Next::WalkToTag("bank".into()));
        here().ask(&mut giant);
        here().answered(&["hands you"]).ask(&mut giant);
        here().ask(&mut giant);
        here().ask(&mut giant);
        here()
            .answered(&["hands you a crystal amulet"])
            .ask(&mut giant);
        here().ask(&mut giant);
        assert_eq!(here().ask(&mut giant), Next::WalkTo(RoomId(6274)));
    }

    #[test]
    fn short_of_silver_or_too_laden_stops_the_trip_in_plain_words() {
        let mut giant = Giant::new(Way::ToRiversRest);
        let here = || buying(Scene::at(6274, 1));
        to_the_cloak(&mut giant, here);
        here().ask(&mut giant);
        assert_eq!(here().ask(&mut giant), put("withdraw 2000 silvers"));
        let poor = here().answered(&["You don't seem to have that much in the account."]);
        assert_eq!(poor.ask(&mut giant), Next::Stop(POOR_TO.into()));

        let mut giant = Giant::new(Way::FromRiversRest);
        shop(
            &mut giant,
            here,
            ["withdraw 2000 silvers", "alchemist", "order 15"],
        );
        let laden = here().answered(&["Looks like you might buckle under this weight."]);
        assert_eq!(laden.ask(&mut giant), Next::Stop(LADEN_FROM.into()));
    }

    #[test]
    fn a_walker_not_allowed_to_buy_closes_up_takes_its_hands_back_and_stops() {
        let mut giant = Giant::new(Way::ToRiversRest);
        let here = || Scene::at(6274, 1);
        to_the_cloak(&mut giant, here);
        assert_eq!(
            here().answered(&["That is closed."]).ask(&mut giant),
            put("open #10")
        );
        assert_eq!(
            here().answered(&["You open the cloak."]).ask(&mut giant),
            put("look in #10")
        );
        assert_eq!(here().ask(&mut giant), put("close #10"));
        assert_eq!(here().ask(&mut giant), fill());
        assert_eq!(here().ask(&mut giant), Next::Stop(NO_AMULET.into()));
    }

    #[test]
    fn the_whole_errand_is_bounded() {
        let mut giant = Giant::new(Way::ToRiversRest);
        let ids: Vec<String> = (0..500).map(|n| n.to_string()).collect();
        let things: Vec<(&str, &str, &str)> =
            ids.iter().map(|id| (id.as_str(), "sack", "sack")).collect();
        giant.next_n(&things);
    }

    impl Giant {
        /// Every container closed, five hundred listed: it still ends.
        fn next_n(&mut self, things: &[(&str, &str, &str)]) {
            Scene::at(6274, 1).ask(self);
            Scene::at(6274, 1).ask(self);
            let mut scene = Scene::at(6274, 1);
            scene.answer = vec![line("You are wearing ", things)];
            let mut next = scene.ask(self);
            let mut asks = 0;
            while !matches!(next, Next::Stop(_)) {
                asks += 1;
                assert!(asks <= MAX_CONTAINERS * 4 + 2, "asked {asks} times");
                let answer = match &next {
                    Next::Put(command) if command.starts_with("look") => "That is closed.",
                    _ => "You open it.",
                };
                next = Scene::at(6274, 1).answered(&[answer]).ask(self);
            }
        }
    }
}
