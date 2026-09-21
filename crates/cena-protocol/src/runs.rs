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
    ///
    /// The outermost link per run -- what a click acts on. For the object a
    /// run refers to, which may be nested inside it, see [`Self::objects`].
    pub fn links(&self) -> impl Iterator<Item = &Link> {
        self.runs.iter().filter_map(|r| r.link.as_ref())
    }

    /// Every game object named in the body, in order, nested or not.
    ///
    /// What a consumer asks when it wants `exist` ids rather than clicks.
    pub fn objects(&self) -> impl Iterator<Item = &Link> {
        self.runs.iter().filter_map(Run::object)
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
    ///
    /// The **outermost** open link, which is the one a click acts on.
    pub link: Option<Link>,
    /// The innermost open `<a exist=>`, when it is not already [`Self::link`].
    ///
    /// **Nesting is real, and dropping the inner link lost object identity.**
    /// `ready list` sends
    ///
    /// ```text
    /// weapon: <d cmd="store WEAPON clear">a <a exist="208924336" noun="katar">...</a></d>
    /// ```
    ///
    /// **There is ONE clickable region here, not two.** The katar's own text
    /// is what the player clicks, and clicking it sends `store WEAPON clear`
    /// (author, 2026-09-20). The `<d>` is not a separate widget wrapping an
    /// object; it is the command attached to that object's link.
    ///
    /// So the markup states two facts about the same span: *this text names
    /// object 208924336*, and *clicking it sends this command*. `link` keeps
    /// the command, because that is what a click does. Without this field the
    /// other fact -- the `exist` id and noun -- reached no consumer at all,
    /// which is Rule 2.2a: the model dropping what the parser preserved.
    ///
    /// MEASURED over the 208 live Lich XML logs: **322 nested
    /// `<d>...<a exist>` occurrences across 62 files**, so this is a shape the
    /// wire uses routinely, not an edge case.
    ///
    /// `None` when nothing is nested, and **also** when the outermost link is
    /// itself the `exist` -- the common case -- so a consumer reads
    /// [`Run::object`] rather than testing both.
    pub inner_link: Option<Link>,
}

impl Run {
    /// The game object this run refers to, whether or not it is nested.
    ///
    /// The one place the two spellings are resolved, as [`Link::command`] is
    /// for the two spellings of a direct link.
    #[must_use]
    pub fn object(&self) -> Option<&Link> {
        for link in [self.inner_link.as_ref(), self.link.as_ref()]
            .into_iter()
            .flatten()
        {
            if matches!(link.kind, crate::frame::LinkKind::Exist { .. }) {
                return Some(link);
            }
        }
        None
    }
}
