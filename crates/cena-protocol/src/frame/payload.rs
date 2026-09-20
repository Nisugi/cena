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
    /// Whether this run ended a **physical wire line**.
    ///
    /// # Why a frame has to say this at all
    ///
    /// `Parser::push_bytes` splits on newlines and the newline never reaches a
    /// frame, so a consumer could not tell a *markup* boundary from a *line*
    /// boundary -- and the parser emits one run per markup boundary. A terminal
    /// printer that wrote a line per frame produced
    ///
    /// ```text
    /// [inv]   a
    /// [inv] pebbled grey leather doublet
    /// ```
    ///
    /// because `<a exist=...>` sits between those two pieces; and one that
    /// accumulated until a newline appeared joined every line in the room
    /// together, because no newline ever arrives. **Both bugs were shipped**, one
    /// fixing the other, before the missing fact was identified.
    ///
    /// `VellumFE` never needed this: its public API is `parse_line`, so its
    /// caller does the splitting with `data.lines()` and knows every boundary
    /// implicitly (`src/core/app_core/state.rs:1472`). Cena moved the split
    /// *inside* `push_bytes` -- which is what lets it handle a chunk that ends
    /// mid-line -- and that is the fact this field restores.
    ///
    /// **True on the last run of a line only.** A line yielding three runs
    /// carries `false, false, true`.
    pub ends_line: bool,
}

impl TextFrame {
    /// This run as a [`Run`](crate::runs::Run), for assembling into a line.
    ///
    /// A `TextFrame` is a `Run` plus the two facts a frame carries and a run does
    /// not: which stream it went to, and whether it ended a line. Both are
    /// consumed by whoever is doing the assembling, so what is left is exactly a
    /// `Run` -- and this lives here, beside both types, rather than being an
    /// open-coded struct literal in every consumer that buffers text.
    ///
    /// Clones rather than consuming: `apply` takes `&Frame`, because a frame is
    /// broadcast to every subscriber as well as folded into state.
    #[must_use]
    pub fn as_run(&self) -> crate::runs::Run {
        crate::runs::Run {
            text: self.content.clone(),
            style: self.style.clone(),
            link: self.link.clone(),
        }
    }
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
    /// An `<a>` carrying none of `href`, `exist` or `cmd`: **not actionable.**
    ///
    /// The wiki documents `<a char= game=>` for player references (`:317-318`),
    /// and any attribute Simutronics adds later lands here too. Such a tag
    /// marks its text as *significant* without saying what a click would do.
    ///
    /// It is a variant rather than `Option::None` because the anchor still
    /// exists on the wire and the drop-nothing rule says the consumer should
    /// know it was there. Before this, such tags fell into [`LinkKind::DirectText`] --
    /// "send the link text as a command" -- so
    /// `<a char='Someone'>Someone</a>` became a link that would send a
    /// player's NAME to the game. That is invention rather than omission,
    /// which is the failure a consumer is least able to detect, and
    /// [`Link::command`] now returns `None` for it.
    NotActionable,
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
            LinkKind::Exist { .. } | LinkKind::Url { .. } | LinkKind::NotActionable => None,
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

/// `<roommeta>`: the room's environment, as the game's own codes.
///
/// Eight attributes, and the wire sends **all eight every time** -- MEASURED
/// over the author's September logs, 2,043 occurrences and 2,043 of each:
///
/// ```sh
/// grep -oh '<roommeta [^>]*>' *.xml | grep -oE '[a-z]+=' | sort | uniq -c
/// ```
///
/// # The codes are NOT decoded here
///
/// `terrain="13"` and `climate="2"` are the game's own integers, and no
/// source in this repository says what they mean: Lich stores them as
/// integers (`common/xmlparser.rb:537-545`) and so does `VellumFE`
/// (`src/core/state.rs:1081-1092`). Inventing a mapping would be fabricating
/// a fact, which `plan/05` §-2 forbids.
///
/// The wiki shows `terrain='forest'` (`:133`) and this era's wire sends
/// `terrain="1"`, so the attribute has changed shape at least once. That is
/// the other reason to keep the code: it is what the server said.
///
/// `Option` on every field: a tag that omits one has not said, and §5.2's
/// `Unknown` is not a fabricated zero.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoomMeta {
    /// `weather=`. Observed: 0, 2.
    pub weather: Option<u32>,
    /// `bonfire=`. Observed: 0, 1 -- a flag in practice.
    pub bonfire: Option<u32>,
    /// `inside=`. Observed: 0, 1.
    pub inside: Option<u32>,
    /// `water=`. Observed: 0.
    pub water: Option<u32>,
    /// `sanctuary=`. Observed: 0, 1.
    ///
    /// The one a behavior asks about first: no combat in a sanctuary.
    pub sanctuary: Option<u32>,
    /// `realm=`. Observed: 15, 19. `VellumFE` keys alert packs off this
    /// (`src/config/alertpacks.rs:45`).
    pub realm: Option<u32>,
    /// `climate=`. Observed: 1..=10.
    pub climate: Option<u32>,
    /// `terrain=`. Observed: 0..=14.
    pub terrain: Option<u32>,
}

/// What an `<objectives>` update does to the list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObjectivesAction {
    /// `full-refresh`: this IS the list now.
    FullRefresh,
    /// `patch-objective`: add or update these.
    Patch,
    /// `delete-objective`: remove these.
    Delete,
    /// An action this port has not seen. Kept rather than dropped, because
    /// a new verb is a protocol change and silently ignoring it would apply
    /// the wrong operation to the list.
    Other(&'static str),
}

impl ObjectivesAction {
    /// The wire's word.
    #[must_use]
    pub fn parse(text: &str) -> Self {
        match text {
            "full-refresh" => Self::FullRefresh,
            "patch-objective" => Self::Patch,
            "delete-objective" => Self::Delete,
            // A leak-free `Other`: the three above are every action MEASURED
            // in the author's September logs, so an unknown one is a change
            // worth seeing rather than a string worth keeping.
            _ => Self::Other("unrecognised"),
        }
    }
}

/// One quest or bounty on the objectives list.
///
/// MEASURED over the author's September logs: `type` and `id` on all 1,067,
/// `state`/`name`/`description` on 945, `location`/`cadence` on 708, and
/// `expires` on 1. So only the first two are guaranteed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Objective {
    /// `id=`, e.g. `"24330"`.
    pub id: String,
    /// `type=`. Observed: `QUEST`, `BOUNTY`.
    pub kind: String,
    /// `state=`. Observed: `available`, `offered`.
    pub state: Option<String>,
    /// `name=`, e.g. `"Shadow's Descent"`.
    pub name: Option<String>,
    /// `description=`. Carries newlines as `&#10;` on the wire.
    pub description: Option<String>,
    /// `location=`, e.g. `"The Rift"`.
    pub location: Option<String>,
    /// `cadence=`. Observed: `weekly`, `monthly`.
    pub cadence: Option<String>,
    /// `expires=`. Seen once in 1,067.
    pub expires: Option<String>,
}

/// A context menu the game returned for one object.
///
/// # What a menu is, and why it carries no labels
///
/// Right-click menus are **coordinate lookups**. The client asks with
/// `_menu #<exist id>`; the game answers with this, where every item is a
/// key into a command dictionary (`cmdlist1.xml`, 588 entries) and nothing
/// else. The dictionary maps each coordinate to a label and a command
/// template, in which `@` substitutes the object's noun and `#` its exist
/// id. `reference/wiki_clean/Wrayth protocol.txt:429` states the contract;
/// `reference/VellumFE/src/cmdlist.rs` is the working implementation.
///
/// **The dictionary does not arrive on this wire.** MEASURED over the
/// author's six most recent logins (`C:\Gemstone\lich-5\logs\GSIV-Nisugi`):
/// `cmdlist` 0, `cli` 0, `menu` 2, `mi` 60. The wiki says it is sent at
/// login and this era's traffic does not carry it -- so it is static data a
/// client ships, which is how Vellum uses it, and only the response below
/// is live. Recorded in `plan/15` §1a rather than left as a wiki claim
/// nobody checked.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Menu {
    /// `id=`: which request this answers.
    pub id: String,
    /// `path=`, e.g. `" in #103330"` -- where the object is.
    pub path: Option<String>,
    /// `cat_list=`: the order to present categories in, e.g. `"1 2 3 6"`.
    pub categories: Vec<String>,
    /// The items, in wire order.
    pub items: Vec<MenuItem>,
}

/// One `<mi>`: a key into the command dictionary.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem {
    /// `coord=`, e.g. `"2524,1703"`. The dictionary key.
    pub coord: Option<String>,
    /// `noun=`, which **disambiguates a repeated coordinate**.
    ///
    /// Not decoration: one real menu carries nine items all on coordinate
    /// `2524,1906`, separated only by this -- `amplify`, `codex`, `dust`,
    /// `instill`, `reshape`, `shatter`, `unbind`, `unlock`, `writing`
    /// (`2026-09-20_12-19-45.xml:267`). Dropping it collapses nine commands
    /// into one.
    pub noun: Option<String>,
    /// `menu_cat=`, which **overrides the dictionary's own category**.
    ///
    /// The dictionary gives each coordinate a category, and the wire may
    /// disagree: `2524,1639` is `5_roleplay` in `cmdlist1.xml:369` and
    /// arrives as `5_Survivalist's_Kit`. The game is grouping that item
    /// under the container it came from, so the wire's word wins.
    ///
    /// MEASURED over the author's September logs -- `coord` 425, `noun` 10,
    /// `menu_cat` 8, and nothing else:
    ///
    /// ```sh
    /// grep -o '<mi [^>]*>' *.xml | grep -oE '[a-z_]+=' | sort | uniq -c
    /// ```
    pub menu_cat: Option<String>,
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
    /// **The WIDGET's own `id=`**, e.g. `exprLNK`.
    ///
    /// This was documented as "the dialog these belong to" and filled from the
    /// widget's `id=` -- so a consumer reading it got `exprLNK` where the doc
    /// promised `expr`, and had no way to reach the real answer (review PR-3).
    /// The dialog is [`Self::dialog`].
    pub id: String,
    /// The `<dialogData>` this widget arrived inside, if any.
    ///
    /// `Label`, `ProgressBar` and `InjuryImage` already carry this; widgets
    /// did not, which left them the one dialog child a consumer could not
    /// attribute.
    pub dialog: Option<String>,
    /// The wire tag that produced them: `cmdButton`, `dropDownBox`,
    /// `editBox`, `label`, `link`, `image`, `upDownEditBox`, `skin`, ...
    pub kind: String,
    // NO `clear` FIELD, and the absence is deliberate.
    //
    // There was one, and `thin.rs` wrote `false` into it at the only site
    // that built this struct -- a field with one possible value, which
    // `plan/05` §-1 names directly ("no config option with one value").
    //
    // It was also redundant: "this set replaces the dialog's contents" is
    // already on the wire as `clear='t'` or a self-closing `<dialogData/>`,
    // and `dispatch.rs` turns both into a separate `Frame::ClearDialogData`
    // ahead of the widgets. A consumer reads the clear from that frame, in
    // order, which is where the ordering information lives. Found by review
    // (PR-11).
    /// One bag per widget.
    pub widgets: Vec<super::Attrs>,
}
