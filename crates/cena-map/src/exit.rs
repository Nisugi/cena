//! The exit record: where an exit goes, how it is crossed, and what it costs.
//!
//! `plan/21` §3f. An exit is crossed **either** by one plain command **or** by
//! something scripted upstream -- never both, which is why [`Crossing`] is an
//! enum flattened into the record rather than two optional fields.

use serde::{Deserialize, Serialize};

use crate::cond::{Cond, Walker};
use crate::room::RoomId;
use crate::routine::Routine;
use crate::step::Step;

/// The stable hash of a scripted edge's *shape*: its upstream Ruby with string
/// literals, regex literals and numbers normalised away.
///
/// Two edges with the same `ShapeId` differ only in their parameters, so they
/// are ported by the same code. The converter computes it; this crate only
/// carries it. Sixteen lowercase hex digits.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ShapeId(pub String);

/// How an exit is crossed.
///
/// A plain command, a ported script, or a script nothing has ported yet.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Crossing {
    /// A plain command: `north`, `go door`, `climb rope`.
    #[serde(rename = "cmd")]
    Command(String),
    /// Scripted upstream, and ported: a flat list of guarded steps
    /// (`crate::step`).
    #[serde(rename = "steps")]
    Steps(Vec<Step>),
    /// Scripted upstream as a search, and ported as a named routine
    /// (`crate::routine`).
    #[serde(rename = "routine")]
    Routine(Routine),
    /// Nothing is sent and nothing is awaited: the destination is a room that
    /// exists only in the map (an urchin hub), and the walker crosses
    /// `A -> hub -> B` as one hop, sending the hub's command from A
    /// (`plan/21` §4.1). Upstream spells it `;e true`.
    #[serde(rename = "pass")]
    PassThrough(Pass),
    /// Scripted upstream and not yet ported. **Impassable**, and counted by the
    /// converter's report -- nothing is dropped silently (`plan/21` §3a).
    #[serde(rename = "unported")]
    Unported(ShapeId),
    /// A kind of crossing this build does not know, by the name the map file
    /// gave it. **Impassable.** Only the binary loader produces this: it is how
    /// a map built after a primitive was added still loads in a client built
    /// before (`plan/21` §3a, format rule 1). Never written.
    #[serde(skip)]
    Unknown(String),
}

/// The content of [`Crossing::PassThrough`], which has none. A unit struct
/// rather than a unit variant so the flattened JSON is `"pass": null` beside
/// the exit's other keys, like every other crossing.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pass;

impl Crossing {
    /// Wire name of [`Crossing::Routine`].
    pub const ROUTINE: &'static str = "routine";
    /// Wire name of [`Crossing::PassThrough`].
    pub const PASS: &'static str = "pass";
    /// Wire name of [`Crossing::Command`].
    pub const COMMAND: &'static str = "cmd";
    /// Wire name of [`Crossing::Unported`].
    pub const UNPORTED: &'static str = "unported";
    /// Wire name of [`Crossing::Steps`].
    pub const STEPS: &'static str = "steps";

    /// Whether this build knows how to cross it.
    #[must_use]
    pub fn is_crossable(&self) -> bool {
        matches!(
            self,
            Crossing::Command(_)
                | Crossing::Steps(_)
                | Crossing::Routine(_)
                | Crossing::PassThrough(_)
        )
    }
}

/// What an exit costs the pathfinder, in seconds.
///
/// An exit with **no** cost is impassable, not defaulted. That is Lich's rule
/// -- `next unless edge_weight`, `reference/lich-5/lib/common/map/map_base.rb:829`
/// -- and the 0.2 default people remember lives only in its ETA estimate.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Cost {
    /// A constant.
    Fixed(f64),
    /// A scripted cost nothing has ported yet. Impassable until it is.
    Unported {
        /// The shape of the upstream cost script.
        unported: ShapeId,
    },
    /// A cost that depends on who is walking: `then` when the question holds,
    /// `else` when it does not -- and **impassable when it cannot be answered**
    /// (`crate::cond`), which is not the same as "does not hold".
    Gated {
        when: Cond,
        then: f64,
        #[serde(default, rename = "else", skip_serializing_if = "Option::is_none")]
        otherwise: Option<f64>,
    },
    /// A price the planner worked out and put in `Walker::tables`: table
    /// `table`, room `key`. Impassable when either is missing.
    Table { table: String, key: RoomId },
    /// Several prices, the first whose question holds being the one paid: a
    /// wall that costs little to a walker who can unlock its gate, more to
    /// one who climbs it, and most to one who waits. **A rung that cannot be
    /// answered is passed over**, not fatal as it is in [`Cost::Gated`] --
    /// every rung here is a way across, so not knowing about one only costs
    /// the walker its discount. `else` is the price when none holds;
    /// without one the exit is then impassable.
    Ladder {
        ladder: Vec<Rung>,
        #[serde(default, rename = "else", skip_serializing_if = "Option::is_none")]
        otherwise: Option<f64>,
    },
    /// A roundtime Haste shortens: the giant stairway of the Dark Grotto,
    /// fifteen seconds a step. `hasted` is the roundtime and `step` what the
    /// move costs besides. Under Haste the roundtime is scaled by
    /// `(80 - min(major elemental ranks, level) / 5 - air lore ranks / 5) /
    /// 100`, never below 0.4, and rounded down -- the divisions by five
    /// whole, as upstream's are. **Everyone passes**: without Haste, or
    /// without the ranks to work it out, the price is the full roundtime.
    Hasted { hasted: f64, step: f64 },
    /// A kind of cost this build does not know. **Impassable**; produced only
    /// by the binary loader, for the same reason as [`Crossing::Unknown`].
    #[serde(skip)]
    Unknown(String),
}

/// [`Cost::Hasted`]'s roundtime under Haste, or `None` when Haste is not
/// known to be up or a rank it needs is not known.
fn shortened(roundtime: f64, walker: &Walker) -> Option<f64> {
    Cond::SpellActive("Haste".to_owned())
        .holds(walker)
        .then_some(())?;
    let skills = walker.skills.as_ref()?;
    let ranks = |skill: &str| skills.get(skill).copied().unwrap_or(0);
    let circle = ranks("major elemental").min(walker.level?);
    let percent = 80_u32.saturating_sub(circle / 5 + ranks("elemental lore, air") / 5);
    Some((roundtime * (f64::from(percent) / 100.0).max(0.4)).floor())
}

/// One price of a [`Cost::Ladder`], and the question that earns it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Rung {
    pub when: Cond,
    pub then: f64,
}

impl Cost {
    /// Wire name of [`Cost::Hasted`].
    pub const HASTED: &'static str = "hasted";
    /// Wire name of [`Cost::Ladder`].
    pub const LADDER: &'static str = "ladder";
    /// Wire name of [`Cost::Fixed`].
    pub const FIXED: &'static str = "fixed";
    /// Wire name of [`Cost::Unported`].
    pub const UNPORTED: &'static str = "unported";
    /// Wire name of [`Cost::Table`].
    pub const TABLE: &'static str = "table";
    /// Wire name of [`Cost::Gated`].
    pub const GATED: &'static str = "gated";

    /// Seconds for this walker; `None` is impassable.
    #[must_use]
    pub fn price(&self, walker: &Walker) -> Option<f64> {
        match self {
            Cost::Fixed(seconds) => Some(*seconds),
            Cost::Gated {
                when,
                then,
                otherwise,
            } => {
                if when.ask(walker)? {
                    Some(*then)
                } else {
                    *otherwise
                }
            }
            Cost::Hasted { hasted, step } => {
                Some(shortened(*hasted, walker).unwrap_or(*hasted) + step)
            }
            Cost::Ladder { ladder, otherwise } => ladder
                .iter()
                .find(|rung| rung.when.holds(walker))
                .map(|rung| rung.then)
                .or(*otherwise),
            Cost::Table { table, key } => walker
                .tables
                .get(table)?
                .get(&key.0)
                .copied()
                .filter(|seconds| seconds.is_finite() && *seconds >= 0.0),
            Cost::Unported { .. } | Cost::Unknown(_) => None,
        }
    }
}

/// What kind of exit this is, for drawing: line colour, and whether an exit
/// whose neighbour is off the map gets a directional stub (`plan/21` §3e).
///
/// Derived from the command. Routing never reads it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ExitKind {
    /// One of the eight compass directions.
    Cardinal,
    /// `up` or `down` -- the only exits that state a floor change outright.
    Vertical,
    /// `out`.
    Out,
    /// `go <something>`.
    Go,
    /// `climb <something>`.
    Climb,
    /// Any other plain command: `swim`, `jump`, `crawl`, `push`, a verb with no
    /// object.
    Other,
    /// Scripted upstream, so no single command describes it.
    Scripted,
}

impl ExitKind {
    /// Every kind, for the wire-name round trip.
    pub const ALL: [ExitKind; 7] = [
        ExitKind::Cardinal,
        ExitKind::Vertical,
        ExitKind::Out,
        ExitKind::Go,
        ExitKind::Climb,
        ExitKind::Other,
        ExitKind::Scripted,
    ];

    /// The kind's wire name. The same spelling the JSON uses.
    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            ExitKind::Cardinal => "cardinal",
            ExitKind::Vertical => "vertical",
            ExitKind::Out => "out",
            ExitKind::Go => "go",
            ExitKind::Climb => "climb",
            ExitKind::Other => "other",
            ExitKind::Scripted => "scripted",
        }
    }

    /// The kind a wire name means. A name this build does not know is
    /// [`ExitKind::Other`]: kind only chooses how an exit is *drawn*, so a new
    /// kind degrades to a plain line rather than refusing the map.
    #[must_use]
    pub fn from_name(name: &str) -> ExitKind {
        ExitKind::ALL
            .into_iter()
            .find(|kind| kind.name() == name)
            .unwrap_or(ExitKind::Other)
    }
}

/// One directed exit.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Exit {
    /// The room this exit leads to.
    pub to: RoomId,
    /// What kind of exit this is, for drawing.
    pub kind: ExitKind,
    /// How it is crossed. Flattened: serialises as `"cmd": "…"` or
    /// `"unported": "…"` directly on the exit.
    #[serde(flatten)]
    pub crossing: Crossing,
    /// What it costs. Absent means impassable; see [`Cost`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<Cost>,
}

impl Exit {
    /// Whether a pathfinder may route through this exit knowing nothing about
    /// the walker: this build can cross it, and its cost is a constant.
    #[must_use]
    pub fn is_routable(&self) -> bool {
        self.crossing.is_crossable() && matches!(self.cost, Some(Cost::Fixed(_)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn exit(crossing: Crossing, cost: Option<Cost>) -> Exit {
        Exit {
            to: RoomId(13),
            kind: ExitKind::Cardinal,
            crossing,
            cost,
        }
    }

    #[test]
    fn a_plain_exit_serialises_flat() {
        let e = exit(Crossing::Command("north".into()), Some(Cost::Fixed(0.2)));
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(
            json,
            r#"{"to":13,"kind":"cardinal","cmd":"north","cost":0.2}"#
        );
        assert_eq!(serde_json::from_str::<Exit>(&json).unwrap(), e);
    }

    #[test]
    fn an_unported_exit_round_trips_and_is_not_routable() {
        let shape = ShapeId("00000000deadbeef".into());
        let e = exit(
            Crossing::Unported(shape.clone()),
            Some(Cost::Unported { unported: shape }),
        );
        let json = serde_json::to_string(&e).unwrap();
        assert_eq!(serde_json::from_str::<Exit>(&json).unwrap(), e);
        assert!(!e.is_routable());
    }

    /// The JSON spelling and the wire name are one vocabulary, not two.
    #[test]
    fn a_kinds_wire_name_is_its_json_spelling() {
        for kind in ExitKind::ALL {
            assert_eq!(
                serde_json::to_string(&kind).unwrap(),
                format!("\"{}\"", kind.name())
            );
            assert_eq!(ExitKind::from_name(kind.name()), kind);
        }
        assert_eq!(ExitKind::from_name("teleport"), ExitKind::Other);
    }

    /// Lich's rule, not a convenience default: `map_base.rb:829`.
    #[test]
    fn an_exit_with_no_cost_is_impassable() {
        let e = exit(Crossing::Command("north".into()), None);
        assert!(!e.is_routable());
        assert!(!serde_json::to_string(&e).unwrap().contains("cost"));
    }
}
