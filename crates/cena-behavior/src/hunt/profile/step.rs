//! One step of a routine or sequence: what to send, and the guards in
//! parentheses that say when (`hunt/guard.rs`).

use std::fmt;

use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::hunt::guard::Condition;

/// One step of a routine or sequence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Step {
    /// What is sent, guards removed: `coupdegrace`, `incant 611`, or a
    /// sequence's name. For a held step, the whole line as it was.
    pub send: String,
    /// The guards, all of which must hold for the step to run.
    pub when: Vec<Condition>,
    /// Why this step never runs: it was imported with a guard Hydra does not
    /// have yet, or a shape it does not read. `None` for a step that runs.
    pub held: Option<String>,
}

impl Step {
    /// Read a step: what to send, then optionally the guards in parentheses,
    /// such as `coupdegrace (thp 20 empowered_below 30)`.
    ///
    /// # Errors
    ///
    /// Nothing to send, an unbalanced parenthesis, or a guard
    /// [`Condition::parse_group`] refuses.
    pub fn parse(text: &str) -> Result<Self, String> {
        let text = text.trim();
        let (send, group) = split_guards(text)?;
        if send.is_empty() {
            return Err(format!("{text:?} has nothing to send"));
        }
        let when = group.map_or_else(|| Ok(Vec::new()), Condition::parse_group)?;
        Ok(Self {
            send: send.to_owned(),
            when,
            held: None,
        })
    }

    /// A step that never runs, kept with the reason.
    #[must_use]
    pub fn held(text: &str, why: &str) -> Self {
        Self {
            send: text.trim().to_owned(),
            when: Vec::new(),
            held: Some(why.to_owned()),
        }
    }
}

/// `send` and the text inside the trailing parentheses, split at the first
/// `(` outside quotes. A line not ending in `)` has no guards.
fn split_guards(text: &str) -> Result<(&str, Option<&str>), String> {
    let Some(body) = text.strip_suffix(')') else {
        return Ok((text, None));
    };
    let mut quoted = false;
    for (at, c) in body.char_indices() {
        match c {
            '"' => quoted = !quoted,
            '(' if !quoted => {
                let (send, group) = body.split_at(at);
                return Ok((send.trim(), Some(group.get(1..).unwrap_or("").trim())));
            }
            _ => {}
        }
    }
    Err(format!(
        "{text:?} ends with `)` and has no `(` to open the guards"
    ))
}

impl fmt::Display for Step {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.send)?;
        if self.when.is_empty() {
            return Ok(());
        }
        f.write_str(" (")?;
        for (i, condition) in self.when.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{condition}")?;
        }
        f.write_str(")")
    }
}

/// How a step is written: a string, or a table for a held one.
#[derive(Serialize, Deserialize)]
#[serde(untagged)]
pub(super) enum Written {
    Line(String),
    Held { step: String, held: String },
}

impl Serialize for Step {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let written = match &self.held {
            Some(why) => Written::Held {
                step: self.send.clone(),
                held: why.clone(),
            },
            None => Written::Line(self.to_string()),
        };
        written.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for Step {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Self::from_written(Written::deserialize(deserializer)?).map_err(D::Error::custom)
    }
}

impl Step {
    /// A step as it was written.
    pub(super) fn from_written(written: Written) -> Result<Self, String> {
        match written {
            Written::Line(text) => Self::parse(&text),
            Written::Held { step, held } => Ok(Self::held(&step, &held)),
        }
    }
}
