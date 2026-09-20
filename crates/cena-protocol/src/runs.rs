//! [`Runs`]: a component body, parsed.
//!
//! Lives beside the rest of the frame vocabulary rather than in `text.rs`,
//! because it is what the layer above receives -- and under Rule 4.1
//! (`plan/05:352-353`), moving it down is what kept `text.rs` inside its cap.

use crate::frame::{Link, Style};

/// Parsed inner content of a `<component>` / `<compDef>` / `<inv>` body.
///
/// **This type is the fix for Vellum's one structural Rule 2.1 violation.**
/// Vellum stores a component's inner XML as a raw `String`
/// (`src/parser.rs:803-832`, `tag[start+1..end].to_string()`), so a consumer
/// receives
///
/// ```text
/// Component { id: "room players",
///             value: "Also here: <a exist=\"-10891\" noun=\"X\">X</a>" }
/// ```
///
/// and must re-parse markup to find out who is in the room. Rule 2.1 says
/// nothing above `cena-protocol` ever sees an unparsed string, and this sits
/// squarely in M1's room path -- so the body is parsed here, once, into runs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Runs {
    /// The runs, in wire order.
    pub runs: Vec<Run>,
}

impl Runs {
    /// The concatenated display text, with markup removed.
    #[must_use]
    pub fn plain(&self) -> String {
        self.runs.iter().map(|r| r.text.as_str()).collect()
    }

    /// The text of every run the wire bolded, in order.
    ///
    /// **Bold is a wire signal, not decoration.** The game marks an enhancive
    /// value by bolding it -- `<pushBold/>106<popBold/>` in an `info` stat line,
    /// and the same in a `skill` table (`plan/15` §2c) -- so a reader needs to
    /// know which numbers were emphasised, not merely that something was.
    ///
    /// Returned as fragments rather than a flag because the fragments are the
    /// answer: an enhanced stat line bolds exactly its value and its bonus, and
    /// the classifier matches them against the column it parsed.
    #[must_use]
    pub fn bold_fragments(&self) -> Vec<String> {
        self.runs
            .iter()
            .filter(|r| r.style.bold_depth > 0)
            .map(|r| r.text.clone())
            .collect()
    }

    /// True when there is no display text at all.
    ///
    /// `<compDef id='room players'></compDef>` -- an empty room -- is a real
    /// and meaningful wire message, so emptiness is asked about, not
    /// represented by the component's absence.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.runs.iter().all(|r| r.text.is_empty())
    }

    /// Every link in the body, in order. The room's players and objects.
    pub fn links(&self) -> impl Iterator<Item = &Link> {
        self.runs.iter().filter_map(|r| r.link.as_ref())
    }
}

/// One span of display text inside a component body.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Run {
    /// Display text, entity-decoded.
    pub text: String,
    /// Markup open around it.
    pub style: Style,
    /// The link it sits inside, if any.
    pub link: Option<Link>,
}
