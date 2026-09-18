//! The payloads [`Frame`](super::Frame) variants carry.
//!
//! Split out of `frame.rs` under Rule 4.1 (`plan/05:352-353`) -- move code
//! down, do not raise the cap. The parent module holds the enum, which is the
//! vocabulary; this one holds the shapes its larger variants carry.

/// A run of text plus the markup that was open around it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TextFrame {
    /// Display text, entity-decoded, control characters stripped.
    pub content: String,
    /// The stream this text belongs to (`""` is the main window).
    pub stream: String,
    /// Structural markup, not resolved colour. See the module docs.
    pub style: Style,
    /// The link this text sits inside, if any.
    pub link: Option<Link>,
}

/// Markup that was open when a run of text was emitted.
///
/// Structure, not appearance: `cena-ui` maps these to colours. `preset` is the
/// raw `<preset id=>` / `<style id=>` value, so a preset the game invents
/// needs no change here.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Style {
    /// Depth of open `<pushBold>` scopes. Semantic ("this is hostile"), not a
    /// font instruction -- Vellum's owner decision of 2026-08-11,
    /// `src/parser/text.rs:22-27`.
    pub bold_depth: u16,
    /// Innermost `<preset id=>` or `<style id=>`, verbatim.
    pub preset: Option<String>,
    /// Inside an `<output class="mono"/>` region.
    pub mono: bool,
}

/// What a link does when clicked.
///
/// Replaces Vellum's two `String` sentinels (`src/data/widget.rs:269-273`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum LinkKind {
    /// `<a exist= noun=>` -- a game object with an id.
    Exist {
        /// The object's wire id.
        id: String,
        /// The noun a command would target it by.
        noun: String,
    },
    /// `<d cmd=>` -- send this command.
    Direct {
        /// The command to send.
        cmd: String,
    },
    /// A bare `<d>` with no `cmd=`: **the link text is the command.**
    ///
    /// This is the dominant form on the wire, not an edge case. Measured over
    /// 40 stratified corpus files: 27,527 bare `<d>` against 4,890 `<d cmd=>`,
    /// so 85% of direct links carry their command as body text. Every room
    /// exit is one -- `<d>east</d>`, `<d>out</d>`, `<d>northwest</d>`.
    ///
    /// It is a **separate variant rather than a `Direct` with the text copied
    /// into `cmd`** so the distinction survives to the consumer. Vellum
    /// backfills into one slot (`src/parser/links.rs:81-84`, `popped.noun =
    /// popped.text`), which works but erases which of the two the wire sent.
    /// Here [`Link::command`] is the single place that resolves both, so a
    /// Travel behavior asks one question and cannot accidentally read an
    /// empty `cmd` as a real command -- the failure this variant exists to
    /// make unrepresentable.
    DirectText,
    /// A URL. Whether it is safe to open is the UI layer's policy.
    Url {
        /// The target, exactly as the wire spelled it.
        href: String,
    },
}

/// A clickable region of text.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Link {
    /// What clicking it does.
    pub kind: LinkKind,
    /// Display text inside the link.
    pub text: String,
    /// `coord=` for movement links, e.g. `"2524,1864"`.
    pub coord: Option<String>,
}

impl Link {
    /// The command this link sends, if it sends one.
    ///
    /// The one place the two spellings of a direct link are resolved:
    /// [`LinkKind::Direct`] carries its command in `cmd=`, and
    /// [`LinkKind::DirectText`] carries it as the link's own display text.
    /// A consumer that wants "what would clicking this send" calls this and
    /// does not need to know which spelling arrived.
    ///
    /// `None` for [`LinkKind::Exist`] and [`LinkKind::Url`]: an object link is
    /// targeted by its noun and a URL is not a game command at all, so
    /// neither has a command to send.
    #[must_use]
    pub fn command(&self) -> Option<&str> {
        match &self.kind {
            LinkKind::Direct { cmd } => Some(cmd),
            LinkKind::DirectText => Some(&self.text),
            LinkKind::Exist { .. } | LinkKind::Url { .. } => None,
        }
    }
}

/// `<progressBar>` -- a vitals bar, an experience bar, or a labelled gauge.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgressBar {
    /// `health`, `mana`, `stamina`, `spirit`, `mindState`, ...
    pub id: String,
    /// The dialog it arrived in, e.g. `minivitals`; `None` for a bare bar.
    pub dialog: Option<String>,
    /// `value=` verbatim. **This is a percentage, not the current value.**
    pub percent: u32,
    /// `text=` verbatim, e.g. `"health 213/223"` or `"numbed"`.
    pub text: String,
    /// `current/max` parsed out of `text`, when it genuinely says so.
    ///
    /// `None` for a label-only bar such as `text='numbed'`. Vellum returns
    /// `(percentage, 100)` there (`src/parser/numbers.rs:29-31`), fabricating
    /// a maximum the wire never sent; `Option` is the honest shape.
    pub amount: Option<Amount>,
    /// `time=` as **whole seconds remaining**, for the bars that carry it.
    ///
    /// Buffs, debuffs and cooldowns arrive as progress bars with a countdown:
    /// `<progressBar id='110572' value='100' text="Multi-Strike"
    /// time='00:00:37'/>`. This field was absent and the attribute was
    /// dropped, so a Heal or Hunt behavior could not tell when a cooldown
    /// expired. Measured over 40 corpus files: 154,313 progress bars carry
    /// `time=`.
    ///
    /// Parsed to seconds rather than kept as the wire's `"00:00:37"` string,
    /// because Rule 2.1 (`plan/05:270-274`) puts the parsing here and a
    /// behavior wants to compare durations, not strings. The wire form is
    /// uniform: all 55,444 values in a 20-file census match `HH:MM:SS`.
    /// `None` when the attribute is absent or does not have that shape.
    pub time_remaining_secs: Option<u32>,
}

/// A `current/max` pair parsed from a progress bar's `text=`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Amount {
    /// Signed, because negative health is real and is the interesting case.
    ///
    /// Vellum's splitter discards the minus sign. VERIFIED by compiling its
    /// actual `parse_progress_numbers` and running it: `health -10/125`
    /// returns `(10, 125)`, so a character at -10 HP reads as +10. A Heal
    /// behavior reading that concludes all is well while the character bleeds
    /// out.
    pub current: i32,
    /// Maximum, as the wire stated it.
    pub max: i32,
}

/// One row of an effects dialog.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ActiveEffect {
    /// `ActiveSpells`, `Buffs`, `Debuffs`, `Cooldowns`.
    pub category: String,
    /// The effect's id on the wire.
    pub id: String,
    /// Display text for the effect.
    pub text: String,
    /// Absolute epoch second the effect ends, when the wire said so.
    pub time: Option<u64>,
}

/// Widgets inside a dialog, as raw attribute bags.
///
/// Vellum splits these across seven variants carrying `Vec<DialogLink>`,
/// `Vec<DialogSpinBox>`, `Vec<DialogSkin>` and so on
/// (`src/parser.rs:222-321`). Spinboxes and skins are *toolkit* concepts; the
/// wire fact is only "the game sent an `<upDownEditBox>` with these
/// attributes". Collapsing them to one shape keyed by [`kind`](Self::kind)
/// follows the `attrs`-bag pattern Vellum already uses elsewhere and keeps
/// widget vocabulary out of this crate (Rule 2.1). Rule of three
/// (`plan/05` §-1): seven variants of one shape is one variant.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DialogWidgets {
    /// The dialog these belong to.
    pub id: String,
    /// The wire tag that produced them: `cmdButton`, `dropDownBox`,
    /// `editBox`, `label`, `link`, `image`, `upDownEditBox`, `skin`, ...
    pub kind: String,
    /// True when this set replaces the dialog's contents.
    pub clear: bool,
    /// One bag per widget.
    pub widgets: Vec<super::Attrs>,
}
