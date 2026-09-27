//! A sequence: named steps a routine step stands for, played out in order
//! (`plan/30` §5: sequences replace scripts inside a routine), with guards
//! read **once**, before the first step, that decide whether it runs at all.
//!
//! The author's `volley.lic` bails out before it touches a weapon: not while
//! Briar Betrayer has more than 7 seconds left, not unless Volley is
//! available. Guards on each step cannot say that. Once `raise longbow`
//! renews Briar Betrayer, or Volley starts cooling, a guarded swap back is
//! skipped and the bow is left in hand. So a sequence carries its own
//! `when`, and once it holds every step is played out.
//!
//! ```toml
//! [sequences]
//! swap = ["store weapon", "ready 2weapon"]
//!
//! [sequences.volley]
//! when = 'expiring "Briar Betrayer" 7 available "volley"'
//! steps = ["store weapon", "ready 2weapon", "stance offensive", "weapon volley",
//!          "raise longbow", "store 2weapon", "ready weapon"]
//! ```
//!
//! Guards on the routine step that names a sequence gate it the same way:
//! `volley (!hidden)` plays the sequence only while not hidden.

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::{Step, Written};
use crate::hunt::guard::Condition;

/// A named list of steps, and when it runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Sequence {
    /// Read once, before the first step: all must hold, or the sequence is
    /// skipped whole. Empty: it always runs.
    pub when: Vec<Condition>,
    /// The steps, in order.
    pub steps: Vec<Step>,
}

/// How a sequence is written: a list of steps, or a table with `when`.
#[derive(Deserialize)]
#[serde(untagged)]
enum Raw {
    Steps(Vec<Written>),
    Guarded(Guarded),
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Guarded {
    #[serde(default)]
    when: String,
    #[serde(default)]
    steps: Vec<Written>,
}

/// The same two shapes, written back.
#[derive(Serialize)]
#[serde(untagged)]
enum Out<'a> {
    Steps(&'a [Step]),
    Guarded { when: String, steps: &'a [Step] },
}

impl<'de> Deserialize<'de> for Sequence {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let (when, steps) = match Raw::deserialize(deserializer).map_err(|_| {
            D::Error::custom(
                "a sequence is a list of steps, or a table of `when` (guards) and `steps`",
            )
        })? {
            Raw::Steps(steps) => (String::new(), steps),
            Raw::Guarded(Guarded { when, steps }) => (when, steps),
        };
        let when = Condition::parse_group(&when).map_err(D::Error::custom)?;
        let steps = steps
            .into_iter()
            .map(Step::from_written)
            .collect::<Result<_, _>>()
            .map_err(D::Error::custom)?;
        Ok(Self { when, steps })
    }
}

impl Serialize for Sequence {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        if self.when.is_empty() {
            return Out::Steps(&self.steps).serialize(serializer);
        }
        let when = self
            .when
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(" ");
        Out::Guarded {
            when,
            steps: &self.steps,
        }
        .serialize(serializer)
    }
}
