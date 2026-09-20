//! The frame vocabulary: what the wire can say.
//!
//! Ported whole from `reference/VellumFE/src/parser.rs:37-405`
//! (`ParsedElement`), per `plan/13` §4a -- "port aggressively where knowledge
//! lives in the code" -- and CLAUDE.md, "do not reinvent the frame
//! vocabulary". Two years of protocol archaeology are encoded in this list and
//! none of it is rediscoverable cheaply.
//!
//! # Counts, measured rather than restated (plan/05 §-2)
//!
//! `ParsedElement` has **63** variants, not the 61 CLAUDE.md claims:
//!
//! ```text
//! $ sed -n '38,404p' src/parser.rs | grep -cE '^    [A-Z][A-Za-z0-9]*'
//! 63
//! ```
//!
//! run in `reference/VellumFE`, with the enum spanning `:37-405`. The names
//! deduplicate to 63, so no repeat inflates it. CLAUDE.md's "61 variants" and
//! its "~130 tags" are both wrong; see [`crate::tags`] for the second number.
//! Flagged for amendment under `plan/05` §10 rather than silently corrected.
//!
//! # What is deliberately NOT ported, and why
//!
//! **Four of the 63 are not wire vocabulary at all.** Porting them would teach
//! Cena a protocol the game does not speak -- the exact trap CLAUDE.md flags
//! for `vellumImg`.
//!
//! - `VellumImage`, `VellumCommand`, `VellumTimer`. VERIFIED absent from
//!   `KNOWN_WIRE_TAGS` (`grep -c '"vellumImg"' src/parser/text.rs` -> 0, same
//!   for the other two) and dispatched *above* the unknown-tag branch at
//!   `src/parser.rs:877-890`, so in Vellum they never reach it. They are
//!   injected by the client into its own log. Cena does not inject them, so
//!   they correctly fall to [`Frame::UnknownTag`] if one ever appears.
//! - `Event`. Not produced by any tag: it is produced by **user-configured
//!   regex detectors over text** (`use crate::config::EventAction` at
//!   `src/parser.rs:18`). A protocol parser whose output depends on a user's
//!   config file is not a protocol parser, and Rule 2.1 (`plan/05:270-274`)
//!   puts game meaning above this layer. It belongs in `cena-behavior`,
//!   consuming frames.
//!
//! `LichWebUI` is the same category one step removed: it is Lich's own
//! handshake, and since Cena *is* the Lich replacement it has no upstream Lich
//! to handshake with. Dropped too, and left to fall through to
//! [`Frame::UnknownTag`] if it ever arrives.
//!
//! # The arithmetic, stated so it can be checked
//!
//! This enum has **51** variants:
//!
//! ```text
//! $ awk '/^pub enum Frame \{/,/^\}/' src/frame.rs | grep -oE '^    [A-Z][A-Za-z0-9]*' | sort -u | wc -l
//! 51
//! ```
//!
//! (`sort -u` is load-bearing: `ActiveEffect` is both a variant name and the
//! struct it wraps, so without it the line matches twice and the count is 52.)
//!
//! This said **50**, beside the command that prints 51 -- a number restated
//! rather than re-run, refuted by the evidence quoted under it (review
//! PR-13). The missing one is `Structural`, a Cena addition the arithmetic
//! below never counted (`grep -c Structural` over Vellum's parser: 0).
//!
//! `63 - 5 - 5 - 7 + 5 = 51`, where:
//!
//! - **-5 not-wire variants**, each named above: `VellumImage`,
//!   `VellumCommand`, `VellumTimer`, `Event`, `LichWebUI`.
//! - **-5 by collapsing the widget family.** Vellum's `DialogButtons`,
//!   `DialogControls`, `DialogDropDowns`, `DialogFields`, `DialogLabelList`
//!   and `DialogProgressBars` are six variants of one shape -- "the game sent
//!   a widget tag with these attributes" -- so they become one
//!   [`payload::DialogWidgets`] keyed by the tag name. Rule of
//!   three (`plan/05` §-1). `DialogOpen` and `DialogPanelOpen` are unaffected;
//!   they carry a dialog, not a widget.
//! - **-7 declared but never constructed.** An earlier draft of this file
//!   counted `ClearActiveEffects`, `InjuryPopup`, `MindStateExp`,
//!   `QuickbarEntries`, `QuickbarOpen`, `SpellHand` and `TargetList` as
//!   ported, and the arithmetic read `63 - 5 - 5 + 4 = 57`. Nothing in this
//!   crate could produce any of them:
//!
//!   ```text
//!   $ grep -rn "Frame::SpellHand" crates/cena-protocol/ --include=*.rs | grep -v src/frame.rs
//!   (no output; same for the other six)
//!   ```
//!
//!   **A variant nothing can construct is not a port**, so they are gone and
//!   the count says 50. Five of the seven -- injuries, quickbar and the target
//!   dropdown -- are dialog modelling that `plan/12` §7.1 puts outside M1;
//!   their wire traffic is real (34,550 `id='injuries'` and 6,694
//!   `<switchQuickBar` in a 272-file sample) and they come back when that
//!   scope arrives, with handlers, in one commit. The other two were
//!   redundant rather than deferred: `SpellHand` is `Spell` again under
//!   another name for a different widget, which is render intent and Rule 2.1
//!   puts it above this crate; `ClearActiveEffects` is what
//!   [`Frame::ClearDialogData`] already emits now that `clear='t'` is read on
//!   the open tag.
//! - **+5 Cena adds**: [`Frame::UnknownTag`] and [`Frame::MalformedTag`], both
//!   mandated by Rule 2.2; [`Frame::ClientCommand`] and
//!   [`Frame::ClientSettings`], which the gated corpus replay found in real
//!   traffic that Vellum's vocabulary does not name; and
//!   [`Frame::Structural`], which types the tags that carry no payload of
//!   their own so Rule 2.2's "nothing is silently dropped" holds for them too.
//!   This read `+4` and omitted `Structural`, which is where the 50 came
//!   from.
//!
//! One variant is renamed rather than changed (`LaunchURL` ->
//! [`Frame::LaunchUrl`]), which nets to zero. Every dropped variant is
//! recorded above rather than quietly omitted.
//!
//! **Presentation is not ported either** (Rule 2.1: the `Frame` is the
//! vocabulary boundary; this crate carries wire shape, not render intent):
//!
//! - `SpanType` (`Normal | Link | Monsterbold | Spell | Speech`,
//!   `src/parser.rs:28-34`) is a render class. The preset id travels as data
//!   in [`Style::preset`] instead, so a preset the game invents tomorrow needs
//!   no change in this crate.
//! - `fg_color` / `bg_color` on `Text` are resolved against `self.presets`, a
//!   **user-configurable palette** (`src/parser/handlers.rs:112-124`). That
//!   makes parser output depend on user config -- the same bytes give
//!   different frames under a different theme, which is disqualifying for a
//!   replay test. Cena emits the markup structure and lets `cena-ui` resolve
//!   colour.
//! - The sentinels `DIRECT_LINK_SENTINEL` / `URL_LINK_SENTINEL`
//!   (`src/data/widget.rs:269-273`) stuff a kind discriminator into a
//!   `String`. [`LinkKind`] makes it a real enum, which removes both. The
//!   URL-scheme allowlist (`is_web_url`, `:278-280`) is click-safety policy
//!   and belongs with the layer that opens browsers.

use crate::runs::Runs;

mod payload;

pub use payload::{
    ActiveEffect, Amount, DialogWidgets, Link, LinkKind, Menu, MenuItem, ProgressBar, Style,
    TextFrame,
};

/// Attribute bag: name/value pairs exactly as the wire spelled them.
pub type Attrs = Vec<(String, String)>;

/// One thing the game said.
///
/// Rule 2.2 (`plan/05:276-283`) is why [`Frame::UnknownTag`] and
/// [`Frame::MalformedTag`] exist and why neither is optional: Simutronics
/// changes the protocol without notice, and an unmodelled tag must reach the
/// user as text and a log, never panicking and never silently dropped.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum Frame {
    // --- text and framing -------------------------------------------------
    /// A run of display text with the markup that was open around it.
    Text(TextFrame),
    /// `<prompt time=>`. The resync barrier; see [`crate::parser`].
    Prompt { time: String, text: String },
    /// `<spell>` -- the currently prepared spell.
    Spell { text: String },
    /// `<left exist= noun=>` -- left hand contents.
    LeftHand { item: String, link: Option<Link> },
    /// `<right exist= noun=>` -- right hand contents.
    RightHand { item: String, link: Option<Link> },

    // --- timers -----------------------------------------------------------
    /// `<roundTime value=>` -- absolute epoch second the roundtime ends.
    RoundTime { value: u32 },
    /// `<castTime value=>`.
    CastTime { value: u32 },
    /// `<timer id='aimTimer'>`.
    AimTime { value: u32 },

    // --- room and vitals --------------------------------------------------
    /// `<resource picture=>`.
    RoomPicture { id: u32 },
    /// `<progressBar>`, bare or inside `<dialogData>`.
    ProgressBar(ProgressBar),
    /// `<label id= value=>`.
    /// `<label id= value=>`, and **which dialog enclosed it**.
    ///
    /// The dialog is load-bearing for the same reason it is on
    /// [`ProgressBar`] and
    /// [`Self::InjuryImage`]: `<label id='yourLvl'>` means a character level
    /// inside `expr` and a map legend inside `mapViewMain`, and the id alone
    /// cannot tell them apart.
    Label {
        /// `id=`.
        id: String,
        /// `value=`.
        value: String,
        /// The enclosing `<dialogData id=>`, if any.
        dialog: Option<String>,
    },
    /// `<compass><dir value=>` -- the obvious exits, as direction tokens.
    Compass { directions: Vec<String> },
    /// `<component id=>` / `<compDef id=>` -- a named slice of the room.
    ///
    /// **`body` is parsed, not raw.** Vellum stores the inner XML string
    /// verbatim (`src/parser.rs:803-832`, `tag[start+1..end].to_string()`),
    /// handing markup to the layer above and making it re-parse -- a direct
    /// Rule 2.1 violation sitting in M1's room path. [`Runs`] is the fix.
    Component { id: String, body: Runs },
    /// `<pushStream id=>`.
    StreamPush { id: String },
    /// `<crtrStatus exist= ...>`; `attrs` raw so the layer above owns the
    /// flag-name mapping (Vellum's own principle, `src/parser.rs:134-137`).
    CreatureStatus { id: String, attrs: Attrs },
    /// `<roommeta .../>`.
    RoomMeta { attrs: Attrs },

    // --- stream stack -----------------------------------------------------
    /// `<popStream/>`, bare or `<popStream id=>`.
    StreamPop { id: Option<String> },
    /// Synthesized when a `<prompt>` force-closes a stream nobody popped.
    StreamPopForced { id: String },
    /// Synthesized by the stream stack when an inner stream ends.
    StreamResume { id: String },
    /// `<clearStream id=>`.
    ClearStream { id: String },
    /// `<clearDialogData>`, or an empty `<dialogData>`.
    ClearDialogData { id: String },
    /// `<closeDialog>` / `<closedialog>`.
    CloseDialog { id: String },
    /// `<exposeDialog>` / `<exposeStream>` / `<exposeContainer>`.
    Expose { kind: String, id: String },
    /// `<deleteContainer id=>`.
    DeleteContainer { id: String },
    /// Placement attrs riding a `<streamWindow>`/`<openDialog>`/`<container>`.
    WindowHints { id: String, attrs: Attrs },
    /// `<app char= game= title=>` -- who this connection is, and where.
    ///
    /// **`game` is the INSTANCE**, and a multi-session client needs it. The
    /// wire sends `<app char="Alderin" game="Prime" title="GemStone IV:
    /// Alderin [Prime]"/>` (VERIFIED in `tests/fixtures/login_setup.xml` and
    /// `reference/wiki_clean/Wrayth protocol.txt:41`), and Prime, Platinum,
    /// Shattered and Test are separate worlds: the same character name in two
    /// of them is two different characters.
    ///
    /// This carried `character` alone, dropping `game` and `title` while its
    /// own doc comment named them (review PR-11). Widened before it had a
    /// consumer, which is when it is free to do.
    ///
    /// `title` is kept rather than derived: it is what the game says to put in
    /// a window title bar, and reassembling it from the other two would be
    /// guessing at a format the server already sent.
    AppInfo {
        character: String,
        /// The instance: `Prime`, `Platinum`, `Shattered`, `Test`.
        game: String,
        /// The window title the game suggests, verbatim.
        title: String,
    },
    /// `<nav rm=>` -- the room changed. **`None` for a bare `<nav/>`**: a real
    /// shape meaning "arrived, and this room has no UID"
    /// (`reference/lich-5/lib/common/xmlparser.rb`, for `DragonRealms`). It was
    /// `String` via `unwrap_or_default()`, so that produced `id: ""` -- an
    /// empty string a consumer cannot tell from a UID (review MO-10).
    RoomId { id: Option<String> },
    /// `<streamWindow id= title= subtitle= ...>`.
    ///
    /// `id`, `title` and `subtitle` are lifted because M1 renders them.
    /// Everything else rides in [`attrs`](Self::StreamWindow::attrs) rather
    /// than being dropped: a census of a 1,547-file corpus sample found
    /// 1,176,686 `<streamWindow>` carrying **15 distinct attribute names**,
    /// of which only three were kept. `location` (1,169,402) and `target`
    /// (1,163,813) are on ~99% of them, and `ifClosed` (602,513),
    /// `resident` (605,219) and `styleIfClosed` (1,353) are all live.
    StreamWindow {
        id: String,
        title: Option<String>,
        subtitle: Option<String>,
        /// Every attribute the tag carried, including the three above.
        attrs: Attrs,
    },

    // --- status -----------------------------------------------------------
    /// `<image id= name=>`, and **which dialog enclosed it**.
    ///
    /// The name says injuries because that is the one use worth modelling, but
    /// the tag is not exclusive to them: MEASURED over 24 files, 1,313 of 2,155
    /// `<image>` tags are `nomap.jpg` map tiles and ~50 are toolbar buttons.
    /// `dialog` is what separates them, exactly as it does for
    /// [`ProgressBar`] -- the same tag shape meaning
    /// different things depending on the dialog that encloses it.
    ///
    /// Lich makes the same check by walking a stack of open element ids
    /// (`lib/common/xmlparser.rb:809`).
    InjuryImage {
        /// `id=`. The body part, when `dialog` is `injuries`.
        id: String,
        /// `name=`. The severity (`Injury1`..`Scar3`), or the part's own name
        /// when it is unhurt.
        name: String,
        /// The enclosing `<dialogData id=>`, if any.
        dialog: Option<String>,
    },
    /// `<indicator id= visible=>`.
    StatusIndicator { id: String, active: bool },
    /// A row of `ActiveSpells` / `Buffs` / `Debuffs` / `Cooldowns`.
    ActiveEffect(ActiveEffect),
    /// `<objectives action=><objective>`.
    ObjectivesUpdate { action: String, entries: Vec<Attrs> },
    /// A context menu the game built for one object: `<menu>` and its
    /// `<mi>` items, as **one** frame.
    ///
    /// The items were separate frames carrying an empty `id`, so the
    /// coordinates arrived orphaned from the menu they answer and no
    /// consumer could tell which request they belonged to. They open and
    /// close on one line -- VERIFIED in a live log, a 60-item menu on one
    /// line (`GSIV-Nisugi/2026/09/2026-09-20_12-19-45.xml:267`) -- so the
    /// envelope is assembled here, as `<component>` is.
    MenuResponse(Menu),

    // --- dialogs and quickbar ---------------------------------------------
    /// `<switchQuickBar id=>`.
    QuickbarSwitch { id: String },
    /// `<openDialog>` / `<opendialog>`.
    DialogOpen {
        id: String,
        title: Option<String>,
        attrs: Attrs,
    },
    /// A resident `<openDialog>` panel.
    DialogPanelOpen {
        id: String,
        title: Option<String>,
        attrs: Attrs,
    },
    /// Widgets inside a dialog, as raw attribute bags.
    ///
    /// Vellum splits these across seven variants carrying `Vec<DialogLink>`,
    /// `Vec<DialogSpinBox>`, `Vec<DialogSkin>` and so on
    /// (`src/parser.rs:222-321`). Spinboxes and skins are *toolkit* concepts;
    /// the wire fact is only "the game sent an `<upDownEditBox>` with these
    /// attributes". Collapsing them to one variant keyed by [`DialogWidgets::kind`]
    /// follows the `attrs`-bag pattern Vellum already uses elsewhere and keeps
    /// widget vocabulary out of this crate (Rule 2.1). Rule of three
    /// (`plan/05` §-1): seven variants of one shape is one variant.
    DialogWidgets(DialogWidgets),

    // --- containers and inventory -----------------------------------------
    /// `<container id= title= target=>`.
    Container {
        id: String,
        title: Option<String>,
        target: Option<String>,
    },
    /// `<clearContainer id=>`.
    ClearContainer { id: String },
    /// `<inv id=>` -- one item in a container.
    ContainerItem { container_id: String, content: Runs },
    /// `<inventoryManager>` with its `<i>` / `<continuation>` rows.
    InventoryManager { token: String, attrs: Attrs },
    /// `<inventoryViewItem>`.
    InventoryViewItem { id: String, attrs: Attrs },

    // --- misc -------------------------------------------------------------
    /// `<launchURL>` / `<LaunchURL>`. The scheme allowlist is the UI's job.
    LaunchUrl { url: String },
    /// `<pulse min= max= mana=>`.
    Pulse { mana: bool, min: u32, max: u32 },
    /// `<worldEvent realm= expires=>`.
    WorldEvent {
        realm: String,
        expires_min: Option<u32>,
        text: String,
    },
    /// `<PantheonStatus value=>`.
    PantheonStatus { value: u32 },
    /// The player's own typed command, echoed back in the log.
    ///
    /// Not in Vellum's vocabulary at all, and not in `KNOWN_WIRE_TAGS` as
    /// ported -- the Tier 2 corpus replay found it. It arrives as
    /// `<!-- CLIENT --><c>;go2 3609<!-- ENDCLIENT -->`, and it is how a replay
    /// knows what the player did, which for a client that replays its own logs
    /// as test fixtures is the difference between a reproducible session and a
    /// one-sided transcript. VERIFIED at 376 occurrences in
    /// `GSIV-Nisugi/2024/11/xml/2024-11-25_21-13-51.xml`, all 376 inside a
    /// `<!-- CLIENT -->` region.
    ClientCommand { command: String },
    /// The login `<settings>` blob arrived and was consumed whole.
    ///
    /// A client-configuration document -- window layout, palettes, highlight
    /// strings, macros -- not game protocol. VERIFIED to carry 26 distinct
    /// element names of its own (`h`, `dc`, `cmdline`, `ignores`, `panels`,
    /// `toggles`, ...), all on one line in
    /// `GST-Nisugi/2024/11/xml/2024-11-07_20-24-19.xml`.
    ///
    /// Cena is its own client and keeps its own configuration, so the blob's
    /// contents are deliberately not modelled. The frame records that it
    /// happened, because a login is worth seeing in a replay.
    ClientSettings,

    // --- Rule 2.2: the escape hatches, both mandatory ---------------------
    /// A tag this parser does not model, carried whole.
    ///
    /// Mandated by Rule 2.2 (`plan/05:276-283`). `raw` is the original bytes,
    /// so the tag reaches the user as text and a diagnosing reader sees
    /// exactly what the game sent. **This variant is why Rule 2.1's
    /// architecture test carries an explicit allowlist entry: 2.2 mandates
    /// the one escape 2.1 forbids, and the two are a cross-referenced pair.**
    ///
    /// Vellum has the passthrough (`src/parser.rs:1039-1051`) but not the
    /// *type* -- its unknown tag becomes a `Text` element indistinguishable
    /// from game prose, so no consumer can tell "the game said something new"
    /// from "the game said hello". That is the gap this closes.
    UnknownTag { name: String, raw: String },
    /// A tag that arrived and whose effect is not carried by any
    /// neighbouring frame's fields.
    ///
    /// The author's drop-nothing rule: "Cena shouldn't drop anything that
    /// comes in." Three arms in the port returned without emitting --
    /// `markup_tag`'s two `_ => {}` arms and `close_tag`'s
    /// `_ if tags::is_known(name) => {}` -- so 123 close forms and 9
    /// self-closing forms were invisible to every consumer. This is what they
    /// emit instead.
    ///
    /// Distinct from [`Frame::UnknownTag`] on purpose: an `UnknownTag` means
    /// the protocol changed and someone should look, a `Structural` means the
    /// protocol did exactly what it always does and there is nothing to model.
    /// Collapsing them would cry wolf on every `</a>`.
    ///
    /// `raw` is the original bytes, so nothing is unrecoverable. Use
    /// [`Frame::is_structural`] to filter these out; most consumers want to.
    Structural { name: String, raw: String },
    /// A tag that never closed before the line ended.
    ///
    /// Vellum appends the fragment to the text buffer and silently desyncs
    /// (`src/parser.rs:733-736`, "No closing >, treat rest as text"), with no
    /// log and no test asserting the behaviour. Typed here so the case is
    /// visible, testable, and cannot be mistaken for prose.
    MalformedTag { raw: String },
}

impl Frame {
    /// True for a frame that records a tag's presence without modelling it.
    ///
    /// Provided here rather than left to each caller because the drop-nothing
    /// rule creates this filter for **every** consumer at once -- a renderer,
    /// the replay differ, and a behavior all need the same predicate on the
    /// same day, which is the rule of three (`plan/05` §-1) satisfied at
    /// introduction rather than anticipated. It is one `matches!`, not a
    /// trait and not a config option.
    #[must_use]
    pub fn is_structural(&self) -> bool {
        matches!(self, Frame::Structural { .. })
    }

    /// A [`Frame::Structural`] for `name`, carrying `raw` verbatim.
    #[must_use]
    pub(crate) fn structural(name: &str, raw: &str) -> Self {
        Frame::Structural {
            name: name.to_owned(),
            raw: raw.to_owned(),
        }
    }
}
