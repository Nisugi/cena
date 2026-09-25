//! The lines a hunt reacts to that are not combat: a weapon knocked away, a
//! curse or infection taken, a trap sprung, an ambusher, rooted, a bless
//! gone, an arrow stuck, a charge counter, a mark on a creature, the weapon
//! reaction offered.
//!
//! Lich's `combat/defs/messages.rb` (240 lines, 2026-09-16), whole: 21
//! events in seven families, each "a private regex in bigshot, ecleanse or
//! eohunter" gathered into one table. Cena deferred it with the combat port
//! because nothing read it (`state/combat.rs`, *NOT ported*); the hunt is
//! the reader that was waiting (`inventory/12` §2).
//!
//! # Plain text and the line's objects, not the markup
//!
//! Lich's patterns match the raw XML -- `Your <a exist="..." noun="(?<noun>
//! [^"]+)">` -- to recover the id and noun its parser had already read.
//! Here each pattern matches the line's plain text and the id and noun come
//! from the line's objects (`ledger/text.rs`): the weapon is the first
//! unbolded object, a creature the first bolded one (bold is the wire's
//! mark for a creature). The weapon reaction's command is the `<d cmd=>`
//! link's own. So nothing is re-parsed (`plan/12` §3a).
//!
//! # Queued, and drained by whoever acts
//!
//! A chunk's incidents are queued at its prompt ([`Incidents::take`]),
//! capped so a session with no hunt running holds a bounded few; the hunt's
//! driver drains its own fold's queue each turn.

use std::collections::VecDeque;
use std::sync::OnceLock;

use cena_protocol::frame::LinkKind;

use super::chunks::{Chunk, ChunkLine};
use super::containers::ItemRef;
use super::ledger::text::{Pat, creature, item};

/// The most incidents held undrained; the oldest go first.
const MAX_HELD: usize = 64;

/// How a weapon left the hand (`messages.rb`'s `disarm` family).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Disarm {
    /// Knocked from the grasp, or wrenched away into the shadows: `recover
    /// item` where it fell (ecleanse's `recover`).
    Knocked,
    /// Torn free and floating (Telekinetic Disarm, 1406): `get` it.
    Telekinetic,
    /// Stuck in webbing: `pry` it free.
    Webbed,
}

/// Which hive trap (`hive_trap`'s `kind`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HiveTrap {
    /// The flickering apparatus.
    Apparatus,
    /// The churning ground.
    Ground,
}

/// One thing that happened.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Incident {
    /// `disarm_seen`: the weapon left the hand. `None` when the line named
    /// no object.
    Disarmed {
        /// How.
        how: Disarm,
        /// The weapon.
        weapon: Option<ItemRef>,
    },
    /// `sanctum_transform`: a Sanctum of Scales sentinel turned the weapon
    /// into a snake (`clench` recovers it, `sanctumwatch.lic:37-52`).
    SanctumSnake {
        /// What the weapon became.
        snake: Option<ItemRef>,
    },
    /// `itchy_curse`.
    ItchyCurse,
    /// `infected_wound`.
    InfectedWound,
    /// `hive_trap`.
    HiveTrap(HiveTrap),
    /// `entangled`: an unseen force.
    Entangled,
    /// `ambusher`: something leapt from hiding. Its noun when the line
    /// named it.
    Ambusher(Option<String>),
    /// `bolted`: `You bolt`.
    Bolted,
    /// `rooted`, with the snake's id when a snake holds you.
    Rooted(Option<String>),
    /// `unrooted`: free of the snake, by id.
    Unrooted(Option<String>),
    /// `item_limit`: too much held to pick more up.
    ItemLimit,
    /// `bless_shrugged`: the weapon struck true and the creature shrugged
    /// off some of the damage.
    BlessShrugged(Option<ItemRef>),
    /// `bless_expired`: the weapon returns to normal.
    BlessExpired(Option<ItemRef>),
    /// `arrow_stuck`: an arrow in a creature, by the creature's id, and
    /// where.
    ArrowStuck {
        /// The creature.
        creature: Option<ItemRef>,
        /// Where it stuck: `neck`, `left arm`'s `arm`, ...
        at: String,
    },
    /// `aiming`: aiming at this part now, or at nothing in particular.
    Aiming(Option<String>),
    /// `bond_return`: a bonded weapon flew back to the hand.
    BondReturn(String),
    /// `haze_703`: Sounds's blood red haze on or off a creature.
    Haze {
        /// The creature.
        creature: Option<ItemRef>,
        /// On, or dissipated.
        on: bool,
    },
    /// `rebuke_1614`: struggling against, in awe of, or recovered from the
    /// radiant aura.
    Rebuke {
        /// The creature.
        creature: Option<ItemRef>,
        /// On, or recovered.
        on: bool,
    },
    /// `swift_justice`: the charges now.
    SwiftJustice(u32),
    /// `arcane_reflex`: on, or back to normal.
    ArcaneReflex(bool),
    /// `weapon_reaction`: `You could use this opportunity to <d cmd='WEAPON
    /// <reaction> #<id>'>`. The reaction and its target, as the link's
    /// command has them after `WEAPON`: `tackle #1234`.
    WeaponReaction(String),
}

struct Patterns {
    knocked: Pat,
    wrenched: Pat,
    protrusion: Pat,
    swing_protrusion: Pat,
    floats: Pat,
    webbing: Pat,
    sanctum: Pat,
    itchy: Pat,
    infected: Pat,
    glint: Pat,
    apparatus: Pat,
    churns: Pat,
    frenzy: Pat,
    helpless: Pat,
    entangled: Pat,
    leaps: Pat,
    shadows: Pat,
    figure: Pat,
    bolt: Pat,
    rooted: Pat,
    coils: Pat,
    free: Pat,
    unable_hold: Pat,
    treasure: Pat,
    sigils: Pat,
    shrugs: Pat,
    normal: Pat,
    sticks: Pat,
    aiming: Pat,
    not_aiming: Pat,
    bond: Pat,
    haze_on: Pat,
    haze_off: Pat,
    struggling: Pat,
    awe: Pat,
    recovers: Pat,
    justice_up: Pat,
    justice_down: Pat,
    reflex_on: Pat,
    reflex_off: Pat,
    reaction: Pat,
}

fn patterns() -> &'static Patterns {
    static P: OnceLock<Patterns> = OnceLock::new();
    P.get_or_init(|| Patterns {
        knocked: Pat::new(r"^Your .+ is knocked from your grasp"),
        wrenched: Pat::new(
            r"your .+ at .+?\.  The weapon rebounds off of the hardened .+? and is wrenched from your hand\.  It slides along the ground and disappears into the shadows!",
        ),
        protrusion: Pat::new(
            r"^Your .+ strikes one of the bony protrusions on an? .+ \w+ and it is wrenched out of your grasp!",
        ),
        swing_protrusion: Pat::new(
            r"^You swing your .+ at (?:an?|the) .+\.  The weapon strikes one of the bony protrusions on the .+ \w+ and it is wrenched out of your grasp!",
        ),
        floats: Pat::new(r"Your .+ tears free from your hands and floats"),
        webbing: Pat::new(r"The webbing entangles your .+, rendering it useless"),
        sanctum: Pat::new(
            r"Striking with a serpent's unsettling quickness, .*\.  Vile .*, kindling it into an unholy semblance of life\.  The .* form twists and mutates, sprouting scales and cold eyes as it transforms into an? .+!",
        ),
        itchy: Pat::new(r"You shiver slightly as an invisible rash covers your body"),
        infected: Pat::new(
            r"The flesh around the wound feels hot and cold at the same time, heavy with infection\.",
        ),
        glint: Pat::new(r"You notice a flickering glint in the shadows"),
        apparatus: Pat::new(r"The apparatus flickers with deadly radiance"),
        churns: Pat::new(r"The ground churns violently as flashes of chitin jut from its depths"),
        frenzy: Pat::new(
            r"The ground underfoot churns violently and huge chitinous mandibles flash as the insectoid monstrosity below goes into a feeding frenzy!",
        ),
        helpless: Pat::new(
            r"Hindered by the churning terrain, you are helpless as the concealed assailant's mandibles snap at you from the safety of its pit trap!",
        ),
        entangled: Pat::new(r"^An unseen force entangles you, restricting your movement!"),
        leaps: Pat::new(r"(?i)[a-z]*? leaps from hiding to attack!"),
        shadows: Pat::new(r"(?i)flies out of the shadows toward"),
        figure: Pat::new(r"(?i)A shadowy figure leaps from hiding to attack"),
        bolt: Pat::new(r"(?i)^You bolt"),
        rooted: Pat::new(r"You don't seem to be able to move(?: your legs)? to do that\."),
        coils: Pat::new(
            r"You are unable to get out of the way as the snake coils tightly around you, holding you in place!",
        ),
        free: Pat::new(r"You're finally able to break free of the snake's coils!"),
        unable_hold: Pat::new(r"^You are unable to hold the number of items "),
        treasure: Pat::new(r"^You note some treasure of interest but are unable to pick any up\."),
        sigils: Pat::new(
            r"^At your touch, the lit sigils marking your .+ ignite, then quickly sputter out again\.",
        ),
        shrugs: Pat::new(r"(?i)The .+ strikes? true.* shrugs off some of the damage!"),
        normal: Pat::new(r"(?i)Your .+ returns? to normal\."),
        sticks: Pat::new(r"(?i)The .* sticks in an? .+'s (?:left |right )?(.*)!"),
        aiming: Pat::new(r"(?i)You're now aiming at the (.*) of"),
        not_aiming: Pat::new(r"(?i)You're now no longer aiming at anything in particular"),
        bond: Pat::new(r"(?i)^An? (.*) rises out of the shadows and flies back to your waiting hand!"),
        haze_on: Pat::new(r"(?i)is suddenly surrounded by a blood red haze\."),
        haze_off: Pat::new(r"(?i)The blood red haze dissipates from around"),
        struggling: Pat::new(r"(?i)visibly struggling against your radiant aura!"),
        awe: Pat::new(r"(?i)in awe of your radiant aura!"),
        recovers: Pat::new(r"(?i)recovers from being rebuked"),
        justice_up: Pat::new(r"(?i)Your Swift Justice charges are increased to (\d+)\."),
        justice_down: Pat::new(
            r"(?i)Your Swift Justice surges through you! Its charges are reduced to (\d+)\.",
        ),
        reflex_on: Pat::new(r"(?i)^Vital energy infuses you, hastening your arcane reflexes!"),
        reflex_off: Pat::new(
            r"(?i)^Nature's blessing of vitality departs as your arcane prowess returns to normal\.",
        ),
        reaction: Pat::new(r"(?i)^You could use this opportunity to .+!"),
    })
}

/// Every incident one closed chunk states, in line order.
#[must_use]
pub fn classify(chunk: &Chunk) -> Vec<Incident> {
    chunk.lines().iter().filter_map(read).collect()
}

/// One line's incident, if it states one. Each line states at most one:
/// no two of Lich's patterns match the same line.
#[must_use]
pub fn read(line: &ChunkLine) -> Option<Incident> {
    let p = patterns();
    let text = line.text();
    let t = text.as_str();
    let disarmed = |how| Incident::Disarmed {
        how,
        weapon: item(line),
    };
    if p.knocked.is_match(t)
        || p.wrenched.is_match(t)
        || p.protrusion.is_match(t)
        || p.swing_protrusion.is_match(t)
    {
        return Some(disarmed(Disarm::Knocked));
    }
    if p.floats.is_match(t) {
        return Some(disarmed(Disarm::Telekinetic));
    }
    if p.webbing.is_match(t) {
        return Some(disarmed(Disarm::Webbed));
    }
    if p.sanctum.is_match(t) {
        // The last object on the line is what the weapon became.
        let snake = super::ledger::text::objects(line)
            .into_iter()
            .map(|(object, _)| object)
            .next_back();
        return Some(Incident::SanctumSnake { snake });
    }
    if p.itchy.is_match(t) {
        return Some(Incident::ItchyCurse);
    }
    if p.infected.is_match(t) {
        return Some(Incident::InfectedWound);
    }
    if p.glint.is_match(t) || p.apparatus.is_match(t) {
        return Some(Incident::HiveTrap(HiveTrap::Apparatus));
    }
    if p.churns.is_match(t) || p.frenzy.is_match(t) || p.helpless.is_match(t) {
        return Some(Incident::HiveTrap(HiveTrap::Ground));
    }
    if p.entangled.is_match(t) {
        return Some(Incident::Entangled);
    }
    if p.figure.is_match(t) || p.shadows.is_match(t) {
        return Some(Incident::Ambusher(None));
    }
    if p.leaps.is_match(t) {
        return Some(Incident::Ambusher(creature(line).map(|c| c.noun)));
    }
    if p.bolt.is_match(t) {
        return Some(Incident::Bolted);
    }
    if p.rooted.is_match(t) {
        return Some(Incident::Rooted(None));
    }
    if p.coils.is_match(t) {
        return Some(Incident::Rooted(creature(line).map(|c| c.id)));
    }
    if p.free.is_match(t) {
        return Some(Incident::Unrooted(creature(line).map(|c| c.id)));
    }
    if p.unable_hold.is_match(t) || p.treasure.is_match(t) || p.sigils.is_match(t) {
        return Some(Incident::ItemLimit);
    }
    bless_and_marks(line, t, p)
}

/// The `bless`, `archery`, `marks` and `reaction` families.
fn bless_and_marks(line: &ChunkLine, t: &str, p: &Patterns) -> Option<Incident> {
    if p.shrugs.is_match(t) {
        return Some(Incident::BlessShrugged(item(line)));
    }
    if p.reflex_off.is_match(t) {
        return Some(Incident::ArcaneReflex(false));
    }
    if p.normal.is_match(t) {
        return Some(Incident::BlessExpired(item(line)));
    }
    if let Some(caps) = p.sticks.captures(t) {
        return Some(Incident::ArrowStuck {
            creature: creature(line),
            at: caps.get(1).map_or("", |m| m.as_str()).to_owned(),
        });
    }
    if let Some(caps) = p.aiming.captures(t) {
        return Some(Incident::Aiming(caps.get(1).map(|m| m.as_str().to_owned())));
    }
    if p.not_aiming.is_match(t) {
        return Some(Incident::Aiming(None));
    }
    if let Some(caps) = p.bond.captures(t) {
        return Some(Incident::BondReturn(
            caps.get(1).map_or("", |m| m.as_str()).to_owned(),
        ));
    }
    if p.haze_on.is_match(t) || p.haze_off.is_match(t) {
        return Some(Incident::Haze {
            creature: creature(line),
            on: p.haze_on.is_match(t),
        });
    }
    if p.struggling.is_match(t) || p.awe.is_match(t) || p.recovers.is_match(t) {
        return Some(Incident::Rebuke {
            creature: creature(line),
            on: !p.recovers.is_match(t),
        });
    }
    if let Some(caps) = p
        .justice_up
        .captures(t)
        .or_else(|| p.justice_down.captures(t))
    {
        let charges = caps.get(1)?.as_str().parse().ok()?;
        return Some(Incident::SwiftJustice(charges));
    }
    if p.reflex_on.is_match(t) {
        return Some(Incident::ArcaneReflex(true));
    }
    if p.reaction.is_match(t) {
        let reaction = line.links().find_map(|link| match &link.kind {
            LinkKind::Direct { cmd } => {
                let (verb, rest) = cmd.split_once(' ')?;
                verb.eq_ignore_ascii_case("weapon")
                    .then(|| rest.trim().to_owned())
            }
            _ => None,
        })?;
        return Some(Incident::WeaponReaction(reaction));
    }
    None
}

/// The incidents waiting for whoever acts on them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Incidents {
    held: VecDeque<Incident>,
}

impl Incidents {
    /// Queue a chunk's incidents; past the cap, the oldest go.
    pub fn push(&mut self, incidents: Vec<Incident>) {
        for incident in incidents {
            if self.held.len() >= MAX_HELD {
                self.held.pop_front();
            }
            self.held.push_back(incident);
        }
    }

    /// Every waiting incident, oldest first, leaving none.
    pub fn take(&mut self) -> Vec<Incident> {
        self.held.drain(..).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_pattern_compiles() {
        let p = patterns();
        for pat in [
            &p.knocked,
            &p.wrenched,
            &p.protrusion,
            &p.swing_protrusion,
            &p.floats,
            &p.webbing,
            &p.sanctum,
            &p.itchy,
            &p.infected,
            &p.glint,
            &p.apparatus,
            &p.churns,
            &p.frenzy,
            &p.helpless,
            &p.entangled,
            &p.leaps,
            &p.shadows,
            &p.figure,
            &p.bolt,
            &p.rooted,
            &p.coils,
            &p.free,
            &p.unable_hold,
            &p.treasure,
            &p.sigils,
            &p.shrugs,
            &p.normal,
            &p.sticks,
            &p.aiming,
            &p.not_aiming,
            &p.bond,
            &p.haze_on,
            &p.haze_off,
            &p.struggling,
            &p.awe,
            &p.recovers,
            &p.justice_up,
            &p.justice_down,
            &p.reflex_on,
            &p.reflex_off,
            &p.reaction,
        ] {
            assert!(pat.compiled());
        }
    }

    #[test]
    fn the_queue_is_capped_and_drained() {
        let mut queue = Incidents::default();
        queue.push(vec![Incident::Bolted; MAX_HELD + 3]);
        assert_eq!(queue.take().len(), MAX_HELD);
        assert!(queue.take().is_empty());
    }
}
