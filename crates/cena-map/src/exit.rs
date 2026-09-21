//! The exit record: where an exit goes, how it is crossed, and what it costs.
//!
//! `plan/21` §3f. An exit is crossed **either** by one plain command **or** by
//! something scripted upstream -- never both, which is why [`Crossing`] is an
//! enum flattened into the record rather than two optional fields.

use serde::{Deserialize, Serialize};

use crate::room::RoomId;

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
/// `plan/21` §5 step 2 lands only the two ends of this: a plain command, and
/// "scripted, not ported yet". The primitive step list between them arrives
/// with step 7, as a third variant serialised under `steps`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum Crossing {
    /// A plain command: `north`, `go door`, `climb rope`.
    #[serde(rename = "cmd")]
    Command(String),
    /// Scripted upstream and not yet ported. **Impassable**, and counted by the
    /// converter's report -- nothing is dropped silently (`plan/21` §3a).
    #[serde(rename = "unported")]
    Unported(ShapeId),
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
    /// Whether a pathfinder may route through this exit *as converted*: it has
    /// a plain command and a constant cost.
    #[must_use]
    pub fn is_routable(&self) -> bool {
        matches!(self.crossing, Crossing::Command(_)) && matches!(self.cost, Some(Cost::Fixed(_)))
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

    /// Lich's rule, not a convenience default: `map_base.rb:829`.
    #[test]
    fn an_exit_with_no_cost_is_impassable() {
        let e = exit(Crossing::Command("north".into()), None);
        assert!(!e.is_routable());
        assert!(!serde_json::to_string(&e).unwrap().contains("cost"));
    }
}
