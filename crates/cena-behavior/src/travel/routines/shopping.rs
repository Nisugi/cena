//! What the errands share (`giant`, `sword_gorge`): finding a thing the
//! walker carries, and the trip to the bank and a shop for one it does not.
//!
//! # Finding ([`Search`])
//!
//! Upstream (`errand_giant_to_rr.rb:12-82`, `errand_sword_gorge.rb:77-93`)
//! opens and looks in the lootsack, then every container `inventory
//! containers` lists, reading `GameObj#contents` after each. **Here the model
//! is asked first**, which costs nothing: the hands, the containers whose
//! windows are open, and the whole-inventory snapshot. Only when none of them
//! has the thing is the wire asked, as upstream asks it: `inventory
//! containers`, then `look in #id` for each, opening one that says it is
//! closed and remembering to close it.
//!
//! Deviations, each because the model makes upstream's order moot:
//! - There is no lootsack. Upstream looks there first only because it is the
//!   likeliest place; the model is looked through whole.
//! - A thing is matched by the link's text being exactly the name
//!   (`GameObj#name`), and in the snapshot -- whose names carry their article
//!   -- by ending with it.
//! - At most [`MAX_CONTAINERS`] are looked in.
//!
//! # Buying ([`BankRun`])
//!
//! Upstream (`errand_giant_to_rr.rb:89-111`): `go2 bank`, `unhide` if hidden
//! or invisible, withdraw, `go2` the shop, `unhide`, `order N`, `buy`, `go2
//! bank`, `unhide`, `deposit all`, and `go2` back. A withdrawal the teller
//! does not answer with `hands you` stops the trip for the player. **The
//! gorge's script does not check its withdrawal; this does**, the same way,
//! because walking on to the shop with no silver buys nothing. A walk that
//! finds no way is [`Next::Failed`].

use cena_map::{Action, RoomId, Step};
use cena_session::LinkKind;

use super::{Next, Seen};

/// Containers one search looks in: the walker's stop, not an estimate.
pub(super) const MAX_CONTAINERS: usize = 30;

pub(super) fn step(action: Action) -> Step {
    Step { action, when: None }
}

/// `$go2_get_silvers`: whether the walker may fetch silver and buy.
pub(super) fn may_buy(seen: &Seen<'_>) -> bool {
    seen.walker
        .settings
        .get("get_silvers")
        .is_some_and(|is| is == "true")
}

/// A thing the walker carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Carried {
    /// Its position in the names asked for.
    pub which: usize,
    pub id: String,
    /// The container it is in; `None` for a hand.
    pub within: Option<String>,
}

/// The id of what a hand holds by this noun.
pub(super) fn held(seen: &Seen<'_>, noun: &str) -> Option<String> {
    [&seen.state.right_hand, &seen.state.left_hand]
        .into_iter()
        .find(|hand| hand.noun() == Some(noun))
        .and_then(|hand| hand.id().map(str::to_owned))
}

/// What the model knows, with no command sent.
fn known(seen: &Seen<'_>, which: usize, name: &str) -> Option<Carried> {
    let state = seen.state;
    let carried = |id: &str, within: Option<&str>| Carried {
        which,
        id: id.to_owned(),
        within: within.map(str::to_owned),
    };
    for hand in [&state.right_hand, &state.left_hand] {
        if let (Some(id), Some(is)) = (hand.id(), hand.name())
            && is == name
        {
            return Some(carried(id, None));
        }
    }
    for (window, container) in state.inventory.containers() {
        if let Some(item) = container.items.iter().find(|item| item.text == name) {
            let within = container.target.as_deref().unwrap_or(window);
            return Some(carried(&item.id, Some(within)));
        }
    }
    state
        .inventory_snapshot
        .all()
        .find(|item| item.name == name || item.name.ends_with(&format!(" {name}")))
        .map(|item| {
            let within = (item.parent != "player").then_some(item.parent.as_str());
            carried(&item.id, within)
        })
}

/// What a [`Search`] came to.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Found {
    Ask(Next),
    Item(Carried),
    Nothing,
}

/// Looking for the first of `names` the walker carries. See the module docs.
pub(super) struct Search {
    names: Vec<&'static str>,
    at: Looking,
    /// What `inventory containers` listed: id and noun.
    pub containers: Vec<(String, String)>,
    /// Containers this search opened, to be closed.
    opened: Vec<String>,
}

enum Looking {
    Model,
    Listed,
    /// `look in` was sent for this one; `.1`: it has been opened already.
    Looked(usize, bool),
    Opened(usize),
}

impl Search {
    pub fn new(names: Vec<&'static str>) -> Self {
        Search {
            names,
            at: Looking::Model,
            containers: Vec::new(),
            opened: Vec::new(),
        }
    }

    /// The next container this search opened, as the command that closes it.
    pub fn close(&mut self) -> Option<Next> {
        let id = self.opened.pop()?;
        Some(Next::Put(format!("close #{id}")))
    }

    pub fn next(&mut self, seen: &Seen<'_>) -> Found {
        match self.at {
            Looking::Model => {
                let found = self
                    .names
                    .iter()
                    .enumerate()
                    .find_map(|(which, name)| known(seen, which, name));
                if let Some(found) = found {
                    return Found::Item(found);
                }
                self.at = Looking::Listed;
                Found::Ask(Next::Put("inventory containers".to_owned()))
            }
            Looking::Listed => {
                self.containers = seen
                    .answer
                    .iter()
                    .filter(|line| line.text().contains("You are wearing"))
                    .flat_map(cena_session::ChunkLine::objects)
                    .filter_map(|link| match &link.kind {
                        LinkKind::Exist { id, noun } => Some((id.clone(), noun.clone())),
                        _ => None,
                    })
                    .take(MAX_CONTAINERS)
                    .collect();
                self.look(0, false)
            }
            Looking::Looked(index, opened) => {
                if let Some(found) = self.in_answer(seen, index) {
                    return Found::Item(found);
                }
                if !opened && seen.answered("That is closed") {
                    return self.open(index);
                }
                self.look(index + 1, false)
            }
            Looking::Opened(index) => {
                if !seen.answered("You open") {
                    return self.look(index + 1, false);
                }
                if let Some((id, _)) = self.containers.get(index) {
                    self.opened.push(id.clone());
                }
                self.look(index, true)
            }
        }
    }

    fn look(&mut self, index: usize, opened: bool) -> Found {
        let Some((id, _)) = self.containers.get(index) else {
            return Found::Nothing;
        };
        self.at = Looking::Looked(index, opened);
        Found::Ask(Next::Put(format!("look in #{id}")))
    }

    fn open(&mut self, index: usize) -> Found {
        let Some((id, _)) = self.containers.get(index) else {
            return Found::Nothing;
        };
        self.at = Looking::Opened(index);
        Found::Ask(Next::Put(format!("open #{id}")))
    }

    /// The first name, in the order asked for, that `look in` showed.
    fn in_answer(&self, seen: &Seen<'_>, index: usize) -> Option<Carried> {
        let (within, _) = self.containers.get(index)?;
        self.names.iter().enumerate().find_map(|(which, name)| {
            seen.answer
                .iter()
                .flat_map(cena_session::ChunkLine::objects)
                .find_map(|link| match &link.kind {
                    LinkKind::Exist { id, .. } if link.text == *name => Some(Carried {
                        which,
                        id: id.clone(),
                        within: Some(within.clone()),
                    }),
                    _ => None,
                })
        })
    }
}

/// One thing to buy, and what to tell the player when it cannot be.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Purchase {
    pub withdraw: &'static str,
    /// The shop's tag, as `go2` takes it.
    pub shop: &'static str,
    pub order: &'static str,
    /// What the shopkeeper says on a sale; `None` when upstream does not look.
    pub sold: Option<&'static str>,
    pub poor: &'static str,
    pub laden: &'static str,
}

/// What a [`BankRun`] came to.
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Ran {
    Ask(Next),
    /// Bought, banked, and back.
    Finished,
}

/// Bank, shop, bank, and back. See the module docs.
pub(super) struct BankRun {
    buy: Purchase,
    back: RoomId,
    at: Leg,
    unhid: bool,
}

#[derive(Clone, Copy)]
enum Leg {
    Leave,
    AtBank,
    Withdrew,
    AtShop,
    Buy,
    Bought,
    AtBankAgain,
    Deposited,
    Returned,
}

impl BankRun {
    pub fn new(buy: Purchase, back: RoomId) -> Self {
        BankRun {
            buy,
            back,
            at: Leg::Leave,
            unhid: false,
        }
    }

    /// `fput 'unhide' if hidden? or invisible?`, once at each place.
    fn unhide(&mut self, seen: &Seen<'_>) -> Option<Ran> {
        let unseen = ["hidden", "invisible"]
            .iter()
            .any(|flag| seen.walker.flags.get(*flag) == Some(&true));
        if !unseen || std::mem::replace(&mut self.unhid, true) {
            return None;
        }
        Some(Ran::Ask(Next::Put("unhide".to_owned())))
    }

    /// At a place walked to: the walk's failure, `unhide`, or the command.
    fn arrived(&mut self, seen: &Seen<'_>, command: &str, then: Leg) -> Ran {
        if !seen.ok {
            return Ran::Ask(Next::Failed);
        }
        if let Some(unhide) = self.unhide(seen) {
            return unhide;
        }
        self.unhid = false;
        self.at = then;
        Ran::Ask(Next::Put(command.to_owned()))
    }

    fn walk(&mut self, tag: &str, then: Leg) -> Ran {
        self.at = then;
        Ran::Ask(Next::WalkToTag(tag.to_owned()))
    }

    pub fn next(&mut self, seen: &Seen<'_>) -> Ran {
        match self.at {
            Leg::Leave => self.walk("bank", Leg::AtBank),
            Leg::AtBank => self.arrived(seen, self.buy.withdraw, Leg::Withdrew),
            Leg::Withdrew => {
                if !seen.answered("hands you") {
                    return Ran::Ask(Next::Stop(self.buy.poor.to_owned()));
                }
                self.walk(self.buy.shop, Leg::AtShop)
            }
            Leg::AtShop => self.arrived(seen, self.buy.order, Leg::Buy),
            Leg::Buy => {
                self.at = Leg::Bought;
                Ran::Ask(Next::Put("buy".to_owned()))
            }
            Leg::Bought => {
                if self.buy.sold.is_some_and(|sold| !seen.answered(sold)) {
                    return Ran::Ask(Next::Stop(self.buy.laden.to_owned()));
                }
                self.walk("bank", Leg::AtBankAgain)
            }
            Leg::AtBankAgain => self.arrived(seen, "deposit all", Leg::Deposited),
            Leg::Deposited => {
                self.at = Leg::Returned;
                Ran::Ask(Next::WalkTo(self.back))
            }
            Leg::Returned if seen.ok => Ran::Finished,
            Leg::Returned => Ran::Ask(Next::Failed),
        }
    }
}

#[cfg(test)]
pub(super) mod scenes {
    //! What the errands' tests share.

    use cena_session::{ChunkLine, Link, LinkKind, hands::Hand};

    use super::super::testing::Scene;

    /// A line naming things: `(id, noun, text)`.
    pub fn line(before: &str, things: &[(&str, &str, &str)]) -> ChunkLine {
        let mut line = ChunkLine::plain(before);
        for (id, noun, text) in things {
            let mut run = line.runs.runs[0].clone();
            run.text = (*text).to_owned();
            run.link = Some(Link {
                kind: LinkKind::Exist {
                    id: (*id).to_owned(),
                    noun: (*noun).to_owned(),
                },
                text: (*text).to_owned(),
                coord: None,
            });
            line.runs.runs.push(run);
        }
        line
    }

    pub fn holding(mut scene: Scene, id: &str, noun: &str, name: &str) -> Scene {
        scene.state.right_hand = Hand::Holding {
            id: Some(id.into()),
            noun: Some(noun.into()),
            name: name.into(),
        };
        scene
    }

    pub fn buying(mut scene: Scene) -> Scene {
        scene
            .walker
            .settings
            .insert("get_silvers".into(), "true".into());
        scene
    }
}

#[cfg(test)]
mod tests {
    use super::super::testing::Scene;
    use super::scenes::{holding, line};
    use super::*;

    const SWORD: Purchase = Purchase {
        withdraw: "withdraw 200",
        shop: "weaponshop",
        order: "order 3",
        sold: Some("hands you"),
        poor: "poor",
        laden: "laden",
    };

    fn put(command: &str) -> Found {
        Found::Ask(Next::Put(command.into()))
    }

    #[test]
    fn a_thing_in_a_hand_is_found_with_nothing_sent() {
        let mut search = Search::new(vec!["crystal amulet"]);
        let scene = holding(Scene::at(1, 2), "77", "amulet", "crystal amulet");
        assert_eq!(
            turn(&mut search, &scene),
            Found::Item(Carried {
                which: 0,
                id: "77".into(),
                within: None
            })
        );
    }

    /// Run a search against scenes, turn by turn.
    fn turn(search: &mut Search, scene: &Scene) -> Found {
        struct Probe<'a>(&'a mut Search, Option<Found>);
        impl super::super::Solver for Probe<'_> {
            fn next(&mut self, seen: &Seen<'_>) -> Next {
                self.1 = Some(self.0.next(seen));
                Next::Done
            }
        }
        let mut probe = Probe(search, None);
        scene.ask(&mut probe);
        probe.1.unwrap_or(Found::Nothing)
    }

    #[test]
    fn the_wire_is_asked_container_by_container_and_a_closed_one_opened() {
        let mut search = Search::new(vec!["piece of sun-washed driftwood", "crystal amulet"]);
        assert_eq!(
            turn(&mut search, &Scene::at(1, 2)),
            put("inventory containers")
        );
        let mut listed = Scene::at(1, 2);
        listed.answer = vec![line(
            "You are wearing ",
            &[("10", "cloak", "cloak"), ("11", "pack", "pack")],
        )];
        assert_eq!(turn(&mut search, &listed), put("look in #10"));
        let closed = Scene::at(1, 2).answered(&["That is closed."]);
        assert_eq!(turn(&mut search, &closed), put("open #10"));
        let opened = Scene::at(1, 2).answered(&["You open the cloak."]);
        assert_eq!(turn(&mut search, &opened), put("look in #10"));
        // Still nothing, and it is not opened twice.
        assert_eq!(turn(&mut search, &closed), put("look in #11"));
        let mut inside = Scene::at(1, 2);
        inside.answer = vec![line(
            "In the pack: ",
            &[
                ("5", "amulet", "crystal amulet"),
                ("6", "driftwood", "piece of sun-washed driftwood"),
            ],
        )];
        // The driftwood was asked for first, so it wins.
        assert_eq!(
            turn(&mut search, &inside),
            Found::Item(Carried {
                which: 0,
                id: "6".into(),
                within: Some("11".into())
            })
        );
        assert_eq!(search.close(), Some(Next::Put("close #10".into())));
        assert_eq!(search.close(), None);
    }

    #[test]
    fn no_more_than_the_bound_are_looked_in() {
        let mut search = Search::new(vec!["short sword"]);
        turn(&mut search, &Scene::at(1, 2));
        let ids: Vec<String> = (0..MAX_CONTAINERS + 9).map(|n| n.to_string()).collect();
        let things: Vec<(&str, &str, &str)> =
            ids.iter().map(|id| (id.as_str(), "sack", "sack")).collect();
        let mut listed = Scene::at(1, 2);
        listed.answer = vec![line("You are wearing ", &things)];
        let mut looks = 0;
        let mut found = turn(&mut search, &listed);
        while found != Found::Nothing && looks < 100 {
            looks += 1;
            found = turn(
                &mut search,
                &Scene::at(1, 2).answered(&["There is nothing"]),
            );
        }
        assert_eq!(looks, MAX_CONTAINERS);
    }

    fn ran(run: &mut BankRun, scene: &Scene) -> Ran {
        struct Probe<'a>(&'a mut BankRun, Option<Ran>);
        impl super::super::Solver for Probe<'_> {
            fn next(&mut self, seen: &Seen<'_>) -> Next {
                self.1 = Some(self.0.next(seen));
                Next::Done
            }
        }
        let mut probe = Probe(run, None);
        scene.ask(&mut probe);
        probe.1.unwrap_or(Ran::Finished)
    }

    #[test]
    fn a_hidden_walker_unhides_once_at_each_place() {
        let mut run = BankRun::new(SWORD, RoomId(6955));
        let ask = |next: Next| Ran::Ask(next);
        let mut hidden = Scene::at(1, 2);
        hidden.walker.flags.insert("hidden".into(), true);
        assert_eq!(ran(&mut run, &hidden), ask(Next::WalkToTag("bank".into())));
        assert_eq!(ran(&mut run, &hidden), ask(Next::Put("unhide".into())));
        assert_eq!(
            ran(&mut run, &hidden),
            ask(Next::Put("withdraw 200".into()))
        );
        let paid = Scene::at(1, 2).answered(&["The teller hands you 200 silvers."]);
        assert_eq!(
            ran(&mut run, &paid),
            ask(Next::WalkToTag("weaponshop".into()))
        );
        assert_eq!(ran(&mut run, &hidden), ask(Next::Put("unhide".into())));
        assert_eq!(ran(&mut run, &hidden), ask(Next::Put("order 3".into())));
    }

    #[test]
    fn a_walk_with_no_way_fails_the_exit() {
        let mut run = BankRun::new(SWORD, RoomId(6955));
        ran(&mut run, &Scene::at(1, 2));
        let mut lost = Scene::at(1, 2);
        lost.failed = true;
        assert_eq!(ran(&mut run, &lost), Ran::Ask(Next::Failed));
    }
}
