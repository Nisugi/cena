//! `Errand::CutterFromMarshtown` and `Errand::CutterFromRiversRest`: the
//! smugglers' cutter, which sails for a ticket (rooms 11753 and 18677).
//!
//! Upstream (`upstream_scripts/errand_cutter_marshtown.rb` and
//! `errand_cutter_rivers_rest.rb`) is one script twice, differing in who sells
//! the ticket and how he is reached -- a list of moves from Marshtown's pier,
//! `go2` from under River's Rest's bridge. Both:
//!
//! 1. **Find a ticket that names the walker.** A thing whose noun is one of
//!    [`TICKET_NOUNS`] is `look`ed at, and is the ticket if it `reads, "…` the
//!    character's name. The right hand; the left hand (then `swap`); then,
//!    hands emptied, every container whose contents are known; then
//!    `inventory containers`, and each container listed and not yet looked
//!    in is opened (`look in` if it was open already), searched, and closed
//!    again if this opened it. A ticket found in a container is `get`.
//! 2. **Or buy one**, if the profile says silver may be fetched: go to the
//!    seller and `ask` him `about ticket`. If he names a price, go to the
//!    bank, `withdraw` it, go back and ask again. Return to the pier.
//!    `unhide` before speaking to anyone.
//! 3. **Board.** `go gangplank` if there is one, else wait for the cutter to
//!    put one out, and go.
//!
//! # Where this differs, and why
//!
//! - **`echo …; exit` is [`Next::Stop`]**: no ticket and no leave to buy one,
//!   or too poor, ends the trip and says what to do.
//! - `$go2_get_silvers` is the travel profile's `get_silvers` = `true`. The
//!   name of the setting is this port's; nothing else reads it yet.
//! - The way back to River's Rest's pier is a walk to the room the routine
//!   began in, where upstream names 18677: they are the same room.
//! - Upstream waits for the cutter without end; [`CUTTER_MS`] bounds it.
//! - A move that fails on the way to the seller fails the routine; upstream's
//!   `move` would carry on walking from the wrong room.
//! - Containers' contents are read from the inventory snapshot, which is
//!   what the model keeps of `GameObj#contents`.

use std::collections::VecDeque;

use cena_map::{Action, RoomId, Step};
use cena_session::LinkKind;

use super::{Next, Seen, Solver};

const TICKET_NOUNS: [&str; 6] = ["scrip", "scroll", "document", "parchment", "paper", "note"];
/// How long the cutter is waited for: half an hour.
const CUTTER_MS: u64 = 1_800_000;
const NO_TICKET: &str = "You have no ticket for the cutter, and the travel profile does not \
    allow fetching silver to buy one. Buy a ticket, or set get_silvers to true, and start the \
    trip again.";

/// Where a leg of the errand goes, and how.
#[derive(Clone, Copy)]
enum Way {
    Moves(&'static [&'static str]),
    Walk(u32),
    /// Back to where the routine began.
    Back,
}

/// What differs between the two piers.
pub(super) struct Pier {
    seller: &'static str,
    pause: u64,
    to_seller: Way,
    to_bank: Way,
    from_bank: Way,
    way_back: Way,
    arrived: [&'static str; 2],
    poor: &'static str,
}

const ABOARD: &str = "You hear a voice on the cutter shouting, \"Are the passengers all aboard?\"";

const MARSHTOWN: Pier = Pier {
    seller: "Jyhm",
    pause: 3000,
    to_seller: Way::Moves(&[
        "north",
        "east",
        "north",
        "northwest",
        "west",
        "west",
        "go storefront",
    ]),
    to_bank: Way::Moves(&[
        "out",
        "east",
        "east",
        "southeast",
        "south",
        "east",
        "go bridge",
        "up",
        "southeast",
        "southeast",
        "down",
        "southeast",
        "east",
        "south",
        "south",
        "east",
        "go doors",
    ]),
    from_bank: Way::Moves(&[
        "out",
        "west",
        "north",
        "north",
        "west",
        "northwest",
        "up",
        "northwest",
        "northwest",
        "down",
        "northwest",
        "west",
        "north",
        "northwest",
        "west",
        "west",
        "go storefront",
    ]),
    way_back: Way::Moves(&[
        "out",
        "east",
        "east",
        "southeast",
        "south",
        "west",
        "go pier",
    ]),
    arrived: ["The crew swiftly extends a gangplank.", ABOARD],
    poor: "You do not have the silver in the bank for a ticket to River's Rest. Find the \
        silver, or a ticket, and start the trip again.",
};

const RIVERS_REST: Pier = Pier {
    seller: "Percy",
    pause: 2000,
    to_seller: Way::Walk(11748),
    to_bank: Way::Walk(10911),
    from_bank: Way::Walk(11748),
    way_back: Way::Back,
    arrived: [
        "A small cutter glides up beneath the bridge.  The crew swiftly extends a gangplank.",
        ABOARD,
    ],
    poor: "You do not have the silver in the bank for a ticket to Solhaven. Find the silver, \
        or a ticket, and start the trip again.",
};

enum Todo {
    Do(Next),
    /// `fput 'unhide' if hidden? or invisible?`, asked when it is reached.
    Unhide,
}

enum At {
    Start,
    LookedRight,
    LookedLeft,
    Emptied,
    /// Looking at what the known containers hold: the one just looked at.
    Looked(String),
    Listed,
    NextContainer,
    Opened(String),
    /// Searching container `.0`, which this opened (`.1`): the one looked at.
    Searched(String, bool, Option<String>),
    Ask,
    Asked,
    Withdrew,
    Board,
    Waited,
    /// About to `go gangplank`; `true` while the cutter may yet be waited for.
    Boarding(bool),
    Boarded(bool),
}

pub(super) struct Cutter {
    pier: Pier,
    at: At,
    began: Option<RoomId>,
    /// What is to be done before `at` is asked anything.
    queue: VecDeque<Todo>,
    /// Whether the last thing asked for was a move that must have worked.
    moved: bool,
    /// Things still to look at, and containers looked in or still to open.
    tickets: VecDeque<String>,
    checked: Vec<String>,
    containers: VecDeque<String>,
    paid: bool,
}

fn one(action: Action) -> Next {
    Next::Steps(vec![Step { action, when: None }])
}

fn put(command: String) -> Todo {
    Todo::Do(Next::Put(command))
}

fn is_ticket(noun: &str) -> bool {
    TICKET_NOUNS.contains(&noun)
}

fn hidden(seen: &Seen<'_>) -> bool {
    ["hidden", "invisible"]
        .iter()
        .any(|flag| seen.walker.flags.get(*flag) == Some(&true))
}

/// Upstream's `/reads, ".*#{checkname}/`.
fn names_the_walker(seen: &Seen<'_>) -> bool {
    let Some(name) = seen.state.character.name.as_deref() else {
        return false;
    };
    seen.answer.iter().any(|line| {
        line.text()
            .split_once("reads, \"")
            .is_some_and(|(_, reads)| !name.is_empty() && reads.contains(name))
    })
}

/// The price in `I need 500 coins for something like that.`
fn price(seen: &Seen<'_>) -> Option<String> {
    seen.answer.iter().find_map(|line| {
        let text = line.text();
        let (_, after) = text.split_once("Look -- I need ")?;
        let (coins, _) = after.split_once(" coins for something like that.")?;
        (!coins.is_empty() && coins.bytes().all(|byte| byte.is_ascii_digit()))
            .then(|| coins.to_owned())
    })
}

/// The tickets a container is known to hold.
fn tickets_in(seen: &Seen<'_>, container: &str) -> VecDeque<String> {
    seen.state
        .inventory_snapshot
        .contents_of(container)
        .filter(|item| is_ticket(&item.noun))
        .map(|item| item.id.clone())
        .collect()
}

impl Cutter {
    pub fn marshtown() -> Self {
        Self::from(MARSHTOWN)
    }

    pub fn rivers_rest() -> Self {
        Self::from(RIVERS_REST)
    }

    fn from(pier: Pier) -> Self {
        Cutter {
            pier,
            at: At::Start,
            began: None,
            queue: VecDeque::new(),
            moved: false,
            tickets: VecDeque::new(),
            checked: Vec::new(),
            containers: VecDeque::new(),
            paid: false,
        }
    }

    fn travel(&mut self, way: Way) {
        match way {
            Way::Moves(moves) => self.queue.extend(
                moves
                    .iter()
                    .map(|way| Todo::Do(Next::Go((*way).to_owned()))),
            ),
            Way::Walk(room) => self.queue.push_back(Todo::Do(Next::WalkTo(RoomId(room)))),
            Way::Back => self
                .queue
                .extend(self.began.map(|room| Todo::Do(Next::WalkTo(room)))),
        }
    }

    /// Look at the next thing that might be the ticket, if any is left.
    fn look(&mut self) -> Option<(String, Next)> {
        let id = self.tickets.pop_front()?;
        let look = Next::Put(format!("look #{id}"));
        Some((id, look))
    }

    /// The hands: upstream's first two places.
    fn hands(&mut self, seen: &Seen<'_>) -> Next {
        let state = seen.state;
        let ticket_in = |hand: &cena_session::hands::Hand| {
            hand.noun()
                .filter(|noun| is_ticket(noun))
                .and(hand.id())
                .map(str::to_owned)
        };
        if let (At::Start, Some(id)) = (&self.at, ticket_in(&state.right_hand)) {
            self.at = At::LookedRight;
            return Next::Put(format!("look #{id}"));
        }
        if let (At::Start | At::LookedRight, Some(id)) = (&self.at, ticket_in(&state.left_hand)) {
            self.at = At::LookedLeft;
            return Next::Put(format!("look #{id}"));
        }
        self.at = At::Emptied;
        one(Action::EmptyHands)
    }

    /// The containers whose contents the model already has.
    fn known_containers(&mut self, seen: &Seen<'_>) -> Next {
        let inventory = &seen.state.inventory_snapshot;
        for container in inventory.on_person() {
            if inventory.contents_of(&container.id).next().is_some() {
                self.checked.push(container.id.clone());
                self.tickets.extend(tickets_in(seen, &container.id));
            }
        }
        self.looked(seen, None)
    }

    fn looked(&mut self, seen: &Seen<'_>, last: Option<String>) -> Next {
        if let Some(found) = last.filter(|_| names_the_walker(seen)) {
            self.at = At::Board;
            return Next::Put(format!("get #{found}"));
        }
        if let Some((id, look)) = self.look() {
            self.at = At::Looked(id);
            return look;
        }
        self.at = At::Listed;
        Next::Put("inventory containers".to_owned())
    }

    fn listed(&mut self, seen: &Seen<'_>) -> Next {
        let inventory = &seen.state.inventory_snapshot;
        self.containers = seen
            .answer
            .iter()
            .flat_map(cena_session::ChunkLine::objects)
            .filter_map(|link| match &link.kind {
                LinkKind::Exist { id, .. } => Some(id.clone()),
                _ => None,
            })
            .filter(|id| !self.checked.contains(id) && inventory.get(id).is_some())
            .collect();
        self.open_next(seen)
    }

    fn open_next(&mut self, seen: &Seen<'_>) -> Next {
        let Some(container) = self.containers.pop_front() else {
            return self.buy(seen);
        };
        let open = Next::Put(format!("open #{container}"));
        self.at = At::Opened(container);
        open
    }

    fn searched(
        &mut self,
        seen: &Seen<'_>,
        container: String,
        opened: bool,
        last: Option<String>,
    ) -> Next {
        let shut = opened.then(|| put(format!("close #{container}")));
        if let Some(found) = last.filter(|_| names_the_walker(seen)) {
            self.queue.extend(shut);
            self.at = At::Board;
            return Next::Put(format!("get #{found}"));
        }
        if let Some((id, look)) = self.look() {
            self.at = At::Searched(container, opened, Some(id));
            return look;
        }
        self.queue.extend(shut);
        self.resume(At::NextContainer, seen)
    }

    fn buy(&mut self, seen: &Seen<'_>) -> Next {
        if seen.walker.settings.get("get_silvers").map(String::as_str) != Some("true") {
            return Next::Stop(NO_TICKET.to_owned());
        }
        self.queue.push_back(Todo::Do(Next::Pause(self.pier.pause)));
        self.travel(self.pier.to_seller);
        self.queue.push_back(Todo::Unhide);
        self.at = At::Ask;
        self.next(seen)
    }

    fn asked(&mut self, seen: &Seen<'_>) -> Next {
        if let (false, Some(cost)) = (self.paid, price(seen)) {
            self.travel(self.pier.to_bank);
            self.queue.push_back(Todo::Unhide);
            self.queue
                .push_back(put(format!("withdraw {cost} silvers")));
            self.at = At::Withdrew;
        } else {
            self.travel(self.pier.way_back);
            self.at = At::Board;
        }
        self.next(seen)
    }

    fn board(&mut self, seen: &Seen<'_>) -> Next {
        // Upstream: `unless checkloot.include?('gangplank') and move(…)`.
        if seen.sees_noun("gangplank") {
            self.queue.push_back(Todo::Unhide);
            return self.resume(At::Boarding(true), seen);
        }
        self.wait()
    }

    fn wait(&mut self) -> Next {
        self.at = At::Waited;
        Next::Await(self.pier.arrived.map(str::to_owned).to_vec(), CUTTER_MS)
    }

    fn dispatch(&mut self, seen: &Seen<'_>) -> Next {
        match std::mem::replace(&mut self.at, At::Boarded(false)) {
            At::Start => {
                self.began = seen.here;
                self.at = At::Start;
                self.hands(seen)
            }
            At::LookedRight if names_the_walker(seen) => self.resume(At::Board, seen),
            At::LookedRight => {
                self.at = At::LookedRight;
                self.hands(seen)
            }
            At::LookedLeft if names_the_walker(seen) => {
                self.at = At::Board;
                Next::Put("swap".to_owned())
            }
            At::LookedLeft => {
                self.at = At::Emptied;
                one(Action::EmptyHands)
            }
            At::Emptied => self.known_containers(seen),
            At::Looked(id) => self.looked(seen, Some(id)),
            At::Listed => self.listed(seen),
            At::NextContainer => self.open_next(seen),
            At::Opened(container) => {
                let opened = seen.answered("You open");
                if seen.answered("That is already open.") {
                    self.at = At::Searched(container.clone(), false, None);
                    self.tickets.clear();
                    return Next::Put(format!("look in #{container}"));
                }
                self.resume(At::Searched(container, opened, None), seen)
            }
            At::Searched(container, opened, None) => {
                self.tickets = tickets_in(seen, &container);
                self.searched(seen, container, opened, None)
            }
            At::Searched(container, opened, last) => self.searched(seen, container, opened, last),
            At::Ask => {
                self.at = At::Asked;
                Next::Put(format!(
                    "ask {} about ticket",
                    self.pier.seller.to_lowercase()
                ))
            }
            At::Asked => self.asked(seen),
            At::Withdrew if !seen.answered("hands you") => Next::Stop(self.pier.poor.to_owned()),
            At::Withdrew => {
                self.paid = true;
                self.travel(self.pier.from_bank);
                self.queue.push_back(Todo::Unhide);
                self.resume(At::Ask, seen)
            }
            At::Board => self.board(seen),
            At::Waited if !seen.ok => Next::Failed,
            At::Waited => {
                self.queue.push_back(Todo::Unhide);
                self.resume(At::Boarding(false), seen)
            }
            At::Boarding(first) => {
                self.at = At::Boarded(first);
                Next::Go("go gangplank".to_owned())
            }
            At::Boarded(true) if !seen.ok => self.wait(),
            At::Boarded(_) => Next::Done,
        }
    }

    fn resume(&mut self, at: At, seen: &Seen<'_>) -> Next {
        self.at = at;
        self.next(seen)
    }
}

impl Solver for Cutter {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        if std::mem::take(&mut self.moved) && !seen.ok {
            return Next::Failed;
        }
        while let Some(todo) = self.queue.pop_front() {
            match todo {
                Todo::Unhide if hidden(seen) => return Next::Put("unhide".to_owned()),
                Todo::Unhide => {}
                Todo::Do(next) => {
                    self.moved = matches!(next, Next::Go(_) | Next::WalkTo(_));
                    return next;
                }
            }
        }
        self.dispatch(seen)
    }
}

#[cfg(test)]
#[path = "cutter_tests.rs"]
mod tests;
