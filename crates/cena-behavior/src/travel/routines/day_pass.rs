//! `Routine::DayPass`: a Chronomage day pass between two towns (6 exits).
//!
//! Upstream (`upstream_scripts/day_pass_{wl,imt,sol}_{wl,imt,sol}.rb`, six
//! scripts that differ only by town): find the `day_pass_sack`, opening it if
//! it is shut; empty a hand; drop every pass that has expired; `get` one valid
//! for both towns. With none, and `buy_day_pass` saying `yes`, `true` or this
//! route, step to the clerk, unhide, and ask twice -- the first asking is
//! quoted a price, the second pays it. Short of silver, and allowed to fetch
//! it, walk to the bank once, `withdraw 5000`, walk back and ask again. Look
//! at the new pass, step back, `raise` the pass, drag it into the sack, fill
//! the hand, shut the sack if this opened it. The clerk's word, the step in
//! and out, and the walk to the bank are the town's ([`TOWNS`]); what to ask
//! for is the destination's.
//!
//! # Where this parts from upstream, and why
//!
//! - **Passes are read here, not remembered.** Upstream's *cost* script
//!   (`day_pass_cost_head.rb`) opens the sack and looks at each pass, and a
//!   downstream hook keeps what it read in a global. A price here cannot send
//!   commands and there are no globals, so the solver looks in the sack and at
//!   each pass itself. [`read_pass`] and [`flag_for`] are that same reading as
//!   pure functions, for the planner's pre-flight (`plan/24` stage 5).
//! - **"Now" is the game's.** Upstream compares the expiry against
//!   `Time.now`. This compares against `GameState::game_time` -- the last
//!   prompt's server time, so a replay reads the same. With no prompt seen
//!   yet, a pass not stamped `EXPIRED` is taken as good: the game refuses a
//!   dead one when raised, and nothing is dropped on a guess.
//! - **Both hands are emptied**, not one: `Action::EmptyHands` is what the
//!   trip can owe back, and there is no one-handed form.
//! - **A move that fails ends it.** Upstream walks its list to the bank
//!   blind. Here a failed `Next::Go` gives the hands back and ends, and the
//!   trip looks at where it landed.
//! - **Too poor** upstream turns `buy_day_pass` off and plans again. A solver
//!   cannot write the profile; it answers `Next::Failed`, which gives the
//!   exit up for the trip -- the same effect, for this trip only.
//! - **No pass and not buying** upstream clears what it knew and restarts;
//!   the planner's flag should have priced this exit shut, so `Next::Failed`.
//! - `$go2_get_silvers` is read as the setting `get_silvers` (`yes`/`true`).
//! - The sack is sent as `#id` when the inventory names it (upstream's four
//!   ways of matching, in order), and as `my <sack>` when it does not --
//!   upstream would raise on a sack it cannot find.
//! - Every list is bounded: [`MAX_PASSES`] passes looked at, one bank trip.

use cena_map::{Action, Step};
use cena_session::{ChunkLine, GameState, LinkKind};

use super::{Next, Seen, Solver};

/// Passes looked at in one sack: the walker's stop, not an estimate.
const MAX_PASSES: usize = 20;
/// Upstream's margin either side of the expiry, in seconds.
const MARGIN: i64 = 10;

/// What a town's script says: its code, its name as a pass spells it, what
/// to ask its clerk for a pass *to* it, the clerk's word, the step to the
/// clerk and back, and the walks to the bank and back to the clerk.
struct Town {
    code: &'static str,
    name: &'static str,
    ask_for: &'static str,
    clerk: &'static str,
    step_in: &'static str,
    step_back: &'static str,
    to_bank: &'static [&'static str],
    from_bank: &'static [&'static str],
}

const TOWNS: [Town; 3] = [
    Town {
        code: "wl",
        name: "Wehnimer's Landing",
        ask_for: "wehnimer",
        clerk: "clerk",
        step_in: "south",
        step_back: "north",
        to_bank: &["up", "north", "out", "north", "go bank", "go arch"],
        from_bank: &[
            "go arch",
            "out",
            "south",
            "go wood-sided shop",
            "south",
            "down",
        ],
    },
    Town {
        code: "imt",
        name: "Icemule Trace",
        ask_for: "icemule",
        clerk: "halfling",
        step_in: "go corridor",
        step_back: "go corridor",
        to_bank: &[
            "out",
            "go gate",
            "southeast",
            "southeast",
            "northeast",
            "southeast",
            "south",
            "west",
            "south",
            "east",
            "east",
            "south",
            "south",
            "east",
            "north",
            "north",
            "go archway",
        ],
        from_bank: &[
            "out",
            "south",
            "south",
            "west",
            "north",
            "north",
            "west",
            "west",
            "north",
            "east",
            "north",
            "northwest",
            "southwest",
            "northwest",
            "northwest",
            "go gate",
            "go door",
        ],
    },
    Town {
        code: "sol",
        name: "Solhaven",
        ask_for: "solhaven",
        clerk: "agent",
        step_in: "out",
        step_back: "go arch",
        to_bank: &[
            "out",
            "down",
            "down",
            "down",
            "down",
            "go ramp",
            "east",
            "northwest",
            "north",
            "north",
            "northwest",
            "north",
            "north",
            "north",
            "north",
            "north",
            "north",
            "northwest",
            "east",
            "go doors",
        ],
        from_bank: &[
            "out",
            "west",
            "southeast",
            "south",
            "south",
            "south",
            "south",
            "south",
            "south",
            "southeast",
            "south",
            "south",
            "southeast",
            "west",
            "climb ramp",
            "climb stairway",
            "up",
            "up",
            "up",
            "go building",
        ],
    },
];

fn town(code: &str) -> Option<&'static Town> {
    TOWNS.iter().find(|town| town.code == code)
}

/// When a pass stops working, as far as looking at it says.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(in crate::travel) enum Expiry {
    /// Stamped `EXPIRED`.
    Stamped,
    /// Server epoch seconds, read as upstream reads it: the stated time at
    /// UTC-5, daylight saving or not.
    At(i64),
    /// The pass did not say.
    Unsaid,
}

/// What looking at one pass said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(in crate::travel) struct Pass {
    /// The two towns, as the pass spells them. `None` on a stamped pass,
    /// which no longer says.
    pub towns: Option<(String, String)>,
    pub expiry: Expiry,
}

impl Pass {
    /// Upstream's `expires < Time.now - 10`. Not knowing the time, only the
    /// stamp says so.
    pub fn lapsed(&self, now: Option<i64>) -> bool {
        match (self.expiry, now) {
            (Expiry::Stamped, _) => true,
            (Expiry::At(at), Some(now)) => at < now - MARGIN,
            _ => false,
        }
    }

    /// Upstream's `towns.sort == route.sort and expires > Time.now + 10`.
    pub fn serves(&self, one: &str, other: &str, now: Option<i64>) -> bool {
        let named = self
            .towns
            .as_ref()
            .is_some_and(|(a, b)| (a == one && b == other) || (a == other && b == one));
        let live = match (self.expiry, now) {
            (Expiry::At(at), Some(now)) => at > now + MARGIN,
            (Expiry::At(_), None) => true,
            _ => false,
        };
        named && live
    }
}

/// Read the answer to `look #<pass>`, as upstream's downstream hook does.
/// `None` when the lines are not a day pass's.
pub(in crate::travel) fn read_pass(lines: &[ChunkLine]) -> Option<Pass> {
    let mut pass: Option<Pass> = None;
    for line in lines {
        let text = line.text();
        if text.starts_with("Bold red block letters spelling out \"EXPIRED\"") {
            return Some(Pass {
                towns: None,
                expiry: Expiry::Stamped,
            });
        }
        if text.starts_with("Bold calligraphy states simply") {
            let (_, rest) = text.split_once("between the towns of ")?;
            let (towns, _) = rest.split_once(", commencing")?;
            let (one, other) = towns.split_once(" and ")?;
            pass = Some(Pass {
                towns: Some((one.to_owned(), other.to_owned())),
                expiry: Expiry::Unsaid,
            });
        } else if let (Some(read), Some(rest)) = (
            pass.as_mut(),
            text.strip_prefix("[Your pass will expire on "),
        ) {
            read.expiry = expiry_of(rest).map_or(Expiry::Unsaid, Expiry::At);
        }
    }
    pass
}

/// `Sun Sep 21 14:03:22 ET 2026.` as epoch seconds at UTC-5.
fn expiry_of(stated: &str) -> Option<i64> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let mut words = stated.split_whitespace();
    let (_weekday, month, day, clock, zone, year) = (
        words.next()?,
        words.next()?,
        words.next()?,
        words.next()?,
        words.next()?,
        words.next()?,
    );
    (zone == "ET").then_some(())?;
    let month = i64::try_from(MONTHS.iter().position(|is| *is == month)?).ok()? + 1;
    let day: i64 = day.parse().ok()?;
    let year: i64 = year.split('.').next()?.parse().ok()?;
    let mut clock = clock.split(':').map(|part| part.parse::<i64>().ok());
    let (hour, minute, second) = (clock.next()??, clock.next()??, clock.next()??);
    // Days from the civil date (Howard Hinnant's algorithm).
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let doy = (153 * ((month + 9) % 12) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146_097 + doe - 719_468;
    Some(days * 86_400 + hour * 3_600 + minute * 60 + second + 5 * 3_600)
}

/// The planner's flag for a pass between these two towns, named as a pass
/// spells them: `day_pass:imt,wl`, the codes in alphabetical order.
// For the planner's pre-flight, which is not wired yet (`plan/24` stage 5).
#[cfg_attr(not(test), allow(dead_code))]
pub(in crate::travel) fn flag_for(one: &str, other: &str) -> Option<String> {
    let code = |name: &str| TOWNS.iter().find(|town| town.name == name).map(|t| t.code);
    let (mut first, mut second) = (code(one)?, code(other)?);
    if first > second {
        std::mem::swap(&mut first, &mut second);
    }
    (first != second).then(|| format!("day_pass:{first},{second}"))
}

/// The ids of the day passes a `look in` answer shows.
pub(in crate::travel) fn passes_in(lines: &[ChunkLine]) -> Vec<String> {
    let mut ids = Vec::new();
    for link in lines.iter().flat_map(ChunkLine::objects) {
        if let LinkKind::Exist { id, noun } = &link.kind
            && noun == "pass"
            && link.text.contains("day pass")
            && !ids.contains(id)
        {
            ids.push(id.clone());
        }
    }
    ids.truncate(MAX_PASSES);
    ids
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum At {
    Start,
    /// The answer is to `look in`; whether opening it has been tried.
    LookedIn(bool),
    Opened,
    Emptied,
    /// The answer is to `look` at pass `.0`.
    Read(usize),
    /// Pass `.0` was dropped; on to the next.
    Dropped(usize),
    Got,
    Raised,
    /// Stepped to the clerk, or walked back from the bank.
    Clerk,
    Unhidden,
    /// Asked this many times since arriving.
    Asked(u32),
    ToBank(usize),
    Withdrew,
    FromBank(usize),
    LookedAtNew,
    SteppedBack,
    /// Putting things back, then answering `.0` (`true` is `Done`).
    Ending(bool),
}

pub(super) struct DayPass {
    from: Option<&'static Town>,
    to: Option<&'static Town>,
    route: String,
    at: At,
    sack: String,
    opened_sack: bool,
    hands_emptied: bool,
    banked: bool,
    passes: Vec<String>,
    pass: Option<String>,
}

fn step(action: Action) -> Step {
    Step { action, when: None }
}

impl DayPass {
    pub fn new(route: &str) -> Self {
        let (from, to) = route.split_once(',').unwrap_or_default();
        DayPass {
            from: town(from),
            to: town(to),
            route: route.to_owned(),
            at: At::Start,
            sack: String::new(),
            opened_sack: false,
            hands_emptied: false,
            banked: false,
            passes: Vec::new(),
            pass: None,
        }
    }

    /// Give back the hands, shut the sack if this opened it, and answer.
    fn end(&mut self, done: bool) -> Next {
        self.at = At::Ending(done);
        if std::mem::take(&mut self.hands_emptied) {
            return Next::Steps(vec![step(Action::FillHands)]);
        }
        if std::mem::take(&mut self.opened_sack) {
            return Next::Put(format!("close {}", self.sack));
        }
        if done { Next::Done } else { Next::Failed }
    }

    fn look_at(&mut self, index: usize, seen: &Seen<'_>) -> Next {
        if let Some(id) = self.passes.get(index) {
            self.at = At::Read(index);
            return Next::Put(format!("look #{id}"));
        }
        if let Some(id) = &self.pass {
            self.at = At::Got;
            return Next::Put(format!("get #{id}"));
        }
        let buying = seen.walker.settings.get("buy_day_pass").is_some_and(|is| {
            let is = is.to_lowercase();
            is == "yes" || is == "true" || is.split([' ', ';']).any(|word| word == self.route)
        });
        match (buying, self.from) {
            (true, Some(from)) => {
                self.at = At::Clerk;
                Next::Go(from.step_in.to_owned())
            }
            _ => self.end(false),
        }
    }

    fn read(&mut self, index: usize, seen: &Seen<'_>) -> Next {
        let now = seen.state.game_time().map(i64::from);
        let (Some(id), Some(from), Some(to)) = (self.passes.get(index), self.from, self.to) else {
            return self.end(false);
        };
        if let Some(pass) = read_pass(seen.answer) {
            if pass.lapsed(now) {
                self.at = At::Dropped(index);
                return Next::Put(format!("_drag #{id} drop"));
            }
            if self.pass.is_none() && pass.serves(from.name, to.name, now) {
                self.pass = Some(id.clone());
            }
        }
        self.look_at(index + 1, seen)
    }

    fn ask(&mut self, asked: u32) -> Next {
        let (Some(from), Some(to)) = (self.from, self.to) else {
            return self.end(false);
        };
        self.at = At::Asked(asked + 1);
        Next::Put(format!("ask {} for {}", from.clerk, to.ask_for))
    }

    /// The clerk's answer to the second asking, or a later one.
    fn asked(&mut self, asked: u32, seen: &Seen<'_>) -> Next {
        if seen.answered("quickly hands you") {
            let held = [&seen.state.right_hand, &seen.state.left_hand]
                .into_iter()
                .find(|hand| hand.noun() == Some("pass"))
                .and_then(|hand| hand.id());
            let Some(id) = held else {
                return self.end(true);
            };
            self.pass = Some(id.to_owned());
            self.at = At::LookedAtNew;
            return Next::Put(format!("look #{id}"));
        }
        if seen.answered("don't have enough") {
            let fetch = seen
                .walker
                .settings
                .get("get_silvers")
                .is_some_and(|is| is == "yes" || is == "true");
            if fetch && !std::mem::replace(&mut self.banked, true) {
                self.at = At::ToBank(0);
                return self.bank(seen);
            }
            return self.end(false);
        }
        // Quoted a price: ask again, as upstream does, but only the once.
        if seen.answered("says to you") && asked < 2 {
            return self.ask(asked);
        }
        self.end(true)
    }

    /// The next move of a walk, or `None` at its end.
    fn walk(&mut self, dirs: &[&str], index: usize, at: fn(usize) -> At) -> Option<Next> {
        let dir = dirs.get(index)?;
        self.at = at(index + 1);
        Some(Next::Go((*dir).to_owned()))
    }

    /// To the bank, the withdrawal, and back to the clerk.
    fn bank(&mut self, seen: &Seen<'_>) -> Next {
        let Some(from) = self.from else {
            return self.end(false);
        };
        let walked = match self.at {
            At::ToBank(index) => self.walk(from.to_bank, index, At::ToBank),
            At::FromBank(index) => self.walk(from.from_bank, index, At::FromBank),
            _ => self.walk(from.from_bank, 0, At::FromBank),
        };
        match (walked, self.at) {
            (Some(next), _) => next,
            (None, At::ToBank(_)) => {
                self.at = At::Withdrew;
                Next::Put("withdraw 5000".to_owned())
            }
            (None, _) => self.arrive(seen),
        }
    }

    fn arrive(&mut self, seen: &Seen<'_>) -> Next {
        let hiding = ["hidden", "invisible"]
            .iter()
            .any(|flag| seen.walker.flags.get(*flag) == Some(&true));
        if hiding {
            self.at = At::Unhidden;
            return Next::Put("unhide".to_owned());
        }
        self.ask(0)
    }

    fn start(&mut self, seen: &Seen<'_>) -> Next {
        let (Some(name), Some(_), Some(_)) = (
            seen.walker.settings.get("day_pass_sack"),
            self.from,
            self.to,
        ) else {
            return Next::Failed;
        };
        self.sack =
            sack_in(seen.state, name).map_or_else(|| format!("my {name}"), |id| format!("#{id}"));
        self.at = At::LookedIn(false);
        Next::Put(format!("look in {}", self.sack))
    }
}

/// Upstream's four ways of finding the sack among what is worn, in order.
pub(in crate::travel) fn sack_in(state: &GameState, named: &str) -> Option<String> {
    let lower = named.to_lowercase();
    let worn = || state.inventory_snapshot.on_person();
    let in_order = |name: &str| {
        let name = name.to_lowercase();
        let mut from = 0;
        lower.split(' ').all(|word| {
            name.get(from..)
                .and_then(|rest| rest.find(word))
                .is_some_and(|found| {
                    from += found + word.len();
                    true
                })
        })
    };
    worn()
        .find(|item| item.noun == named)
        .or_else(|| worn().find(|item| item.name == named))
        .or_else(|| worn().find(|item| item.name.to_lowercase().ends_with(&lower)))
        .or_else(|| worn().find(|item| in_order(&item.name)))
        .map(|item| item.id.clone())
}

impl Solver for DayPass {
    fn next(&mut self, seen: &Seen<'_>) -> Next {
        let from = self.from;
        match self.at {
            At::Start => self.start(seen),
            At::LookedIn(tried) => {
                if seen.answered("closed") && !tried {
                    self.at = At::Opened;
                    return Next::Put(format!("open {}", self.sack));
                }
                self.passes = passes_in(seen.answer);
                self.hands_emptied = true;
                self.at = At::Emptied;
                Next::Steps(vec![step(Action::EmptyHands)])
            }
            At::Opened => {
                self.opened_sack = seen.answered("You open");
                self.at = At::LookedIn(true);
                Next::Put(format!("look in {}", self.sack))
            }
            At::Emptied => self.look_at(0, seen),
            At::Read(index) => self.read(index, seen),
            At::Dropped(index) => self.look_at(index + 1, seen),
            At::Got | At::SteppedBack => match &self.pass {
                Some(id) => {
                    self.at = At::Raised;
                    Next::Put(format!("raise #{id}"))
                }
                None => self.end(true),
            },
            At::Raised => {
                let put = format!(
                    "_drag #{} {}",
                    self.pass.as_deref().unwrap_or_default(),
                    self.sack
                );
                self.at = At::Ending(true);
                Next::Put(put)
            }
            At::Clerk | At::ToBank(_) | At::FromBank(_) if !seen.ok => self.end(true),
            At::Clerk => self.arrive(seen),
            At::Unhidden => self.ask(0),
            // The first asking is only quoted a price.
            At::Asked(1) if !self.banked => self.ask(1),
            At::Asked(asked) => self.asked(asked, seen),
            At::ToBank(_) | At::Withdrew | At::FromBank(_) => self.bank(seen),
            At::LookedAtNew => {
                self.at = At::SteppedBack;
                Next::Go(from.map_or("", |town| town.step_back).to_owned())
            }
            At::Ending(done) => self.end(done),
        }
    }
}

#[cfg(test)]
mod tests;
