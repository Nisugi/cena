//! The [`Frame`](super::Frame) enum: the vocabulary itself.
//!
//! Moved down out of `frame.rs` under Rule 4.4 (`plan/05:385-388`) -- move
//! code down, do not raise the cap -- when documenting every field
//! (`missing_docs`, 2026-09-23) needed ~100 lines a split parent capped at 500
//! did not have. The parent keeps the module's account of the vocabulary and
//! re-exports this, so `cena_protocol::frame::Frame` is unchanged.

use super::{
    ActiveEffect, Attrs, CmdListUpdate, DialogWidgets, InventoryResponse, ItemView, Link, Menu,
    Objective, ObjectivesAction, ProgressBar, RoomMeta, TextFrame,
};
use crate::runs::Runs;

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
    Prompt {
        /// `time=` as a string (epoch seconds on the wire); empty when absent.
        time: String,
        /// The body, e.g. `>` or `HR>`: entity-decoded, control characters
        /// stripped.
        text: String,
    },
    /// `<spell>` -- the currently prepared spell.
    Spell {
        /// The body as display text: nested markup removed, entity-decoded,
        /// control characters stripped.
        text: String,
    },
    /// `<left exist= noun=>` -- left hand contents.
    LeftHand {
        /// The body as display text, as `Spell::text` is.
        item: String,
        /// From `href=`, else `exist=`/`noun=`, else `cmd=`; `None` when the
        /// tag carries none of them.
        link: Option<Link>,
    },
    /// `<right exist= noun=>` -- right hand contents.
    RightHand {
        /// The body as display text, as `Spell::text` is.
        item: String,
        /// From `href=`, else `exist=`/`noun=`, else `cmd=`; `None` when the
        /// tag carries none of them.
        link: Option<Link>,
    },

    // --- timers -----------------------------------------------------------
    /// `<roundTime value=>` -- absolute epoch second the roundtime ends.
    RoundTime {
        /// `value=`, epoch seconds; `0` when absent or not a number.
        value: u32,
    },
    /// `<castTime value=>`.
    CastTime {
        /// `value=`: the epoch second the cast time ends, not a duration
        /// (`Wrayth protocol.txt:355`); `0` when absent or not a number.
        value: u32,
    },
    /// `<timer>`, read as the aim timer. The parser does not check `id=`
    /// (`parser/thin.rs`, the `"timer"` arm): every `<timer>` lands here, and
    /// `aimTimer` is the only id this was written for.
    AimTime {
        /// `value=` as a whole number; `0` when absent or not a number.
        value: u32,
    },

    // --- room and vitals --------------------------------------------------
    /// `<resource picture=>`.
    RoomPicture {
        /// `picture=`. Only emitted when it parses as a number; a `<resource>`
        /// without one becomes `Structural`.
        id: u32,
    },
    /// `<progressBar>`, bare or inside `<dialogData>`.
    ProgressBar(ProgressBar),
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
        /// Every attribute, in wire order. `<label>` carries its layout --
        /// `top`/`left`/`width`/`height`/`align` in `character_info.xml` --
        /// and only `id` and `value` were kept (Rule 2.2a).
        attrs: Attrs,
    },
    /// `<compass><dir value=>` -- the obvious exits, as direction tokens.
    Compass {
        /// Each `<dir value=>`, entity-decoded, in wire order; a `<dir>`
        /// without `value=` is skipped.
        directions: Vec<String>,
    },
    /// `<component id=>` / `<compDef id=>` -- a named slice of the room.
    ///
    /// **`body` is parsed, not raw.** Vellum stores the inner XML string
    /// verbatim (`src/parser.rs:803-832`, `tag[start+1..end].to_string()`),
    /// handing markup to the layer above and making it re-parse -- a direct
    /// Rule 2.1 violation sitting in M1's room path. [`Runs`] is the fix.
    Component {
        /// `id=`, e.g. `room objs`; empty when absent.
        id: String,
        /// The tag's body, parsed into styled runs.
        body: Runs,
    },
    /// `<pushStream id=>`.
    StreamPush {
        /// `id=` of the `<pushStream>`, or of a paired `<stream>` /
        /// `<dynaStream>`; empty when absent.
        id: String,
    },
    /// `<crtrStatus exist= ...>`; `attrs` raw so the layer above owns the
    /// flag-name mapping (Vellum's own principle, `src/parser.rs:134-137`).
    CreatureStatus {
        /// The creature's `exist=` id (there is no `id=`); empty when absent.
        id: String,
        /// Every attribute, in wire order, `exist` included.
        attrs: Attrs,
    },
    /// `<roommeta .../>`: the room's environment codes.
    RoomMeta(RoomMeta),

    // --- stream stack -----------------------------------------------------
    /// `<popStream/>`, bare or `<popStream id=>`.
    StreamPop {
        /// The stream closed: `id=` of a `<popStream>`, or the stream a
        /// `</stream>` / `</dynaStream>` ended. `None` for a bare
        /// `<popStream/>`, which closes whichever is current.
        id: Option<String>,
    },
    /// Synthesized when a `<prompt>` force-closes a stream nobody popped.
    StreamPopForced {
        /// The stream that was still open; emitted innermost first, before the
        /// `Prompt`.
        id: String,
    },
    /// Synthesized by the stream stack when an inner stream ends.
    StreamResume {
        /// The enclosing stream, current again. Not emitted when the stack
        /// empties: that is the main window, which `StreamPop` already implies.
        id: String,
    },
    /// `<clearStream id=>`.
    ClearStream {
        /// `id=` of the `<clearStream>` or `<clearDynaStream>`; empty when
        /// absent.
        id: String,
    },
    /// `<clearDialogData>`, or an empty `<dialogData>`.
    ClearDialogData {
        /// `id=`: the dialog to clear; empty when absent.
        id: String,
    },
    /// `<closeDialog>` / `<closedialog>`.
    CloseDialog {
        /// `id=`: the dialog to close; empty when absent.
        id: String,
    },
    /// `<exposeDialog>` / `<exposeStream>` / `<exposeContainer>`.
    Expose {
        /// The tag name, spelled as received: which of the three it was.
        kind: String,
        /// `id=`: the dialog, stream or container to expose; empty when absent.
        id: String,
    },
    /// `<deleteContainer id=>`.
    DeleteContainer {
        /// `id=`: the container to delete; empty when absent.
        id: String,
    },
    /// Placement attrs riding a `<streamWindow>`/`<openDialog>`/`<container>`.
    WindowHints {
        /// `id=`, or the tag name when absent. Always the tag name for
        /// client-configuration tags such as `playerID` and `mode`.
        id: String,
        /// Every attribute, in wire order.
        attrs: Attrs,
    },
    /// `<endSetup/>`: the login setup is over. The session keys `Ready` on the
    /// first `<prompt>` after it (`cena-session`, `actor/readiness.rs`). It
    /// was a `WindowHints` bag, which a consumer could only recognise by
    /// string; unit because the tag has no attributes (`Wrayth protocol.txt:21`
    /// "Key Attributes: (none)"). MEASURED 2026-09-23 over
    /// `C:/Gemstone/lich-5/logs/*/2026/09/*.xml`: 143 of 164 logs carry
    /// it, every one bare --
    /// `grep -ho "<endSetup[^>]*>" | sort | uniq -c` -> `143 <endSetup/>`.
    EndSetup,
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
        /// `char=`: the character's name; empty when absent.
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
    RoomId {
        /// `rm=`, entity-decoded; `None` for a bare `<nav/>`.
        id: Option<String>,
    },
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
        /// `id=`: the stream this window shows; empty when absent.
        id: String,
        /// `title=`; `None` when absent.
        title: Option<String>,
        /// `subtitle=`; `None` when absent.
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
        /// Every attribute, in wire order. Not only layout: the toolbar
        /// `<image>`s in `inventory_container.xml` carry `cmd=`, `echo=` and
        /// `tooltip=` -- a button's whole behaviour -- and all of it was
        /// dropped while only `id` and `name` were kept (Rule 2.2a).
        attrs: Attrs,
    },
    /// `<indicator id= visible=>`.
    StatusIndicator {
        /// `id=` verbatim, `Icon` prefix kept (e.g. `IconSTUNNED`); empty when
        /// absent.
        id: String,
        /// True only when `visible='y'`; any other value, or none, is false.
        active: bool,
    },
    /// A row of `ActiveSpells` / `Buffs` / `Debuffs` / `Cooldowns`.
    ActiveEffect(ActiveEffect),
    /// `<objectives action=>` and the `<objective>` rows it carries.
    ///
    /// One frame, like [`Frame::MenuResponse`] and for the same reason: the
    /// rows used to tokenize separately and land in
    /// [`Frame::WindowHints`] -- the placement-attrs bag, which is not what
    /// a quest is -- while the action arrived on a frame of its own. An
    /// update that says `delete-objective` is meaningless apart from the
    /// rows it deletes.
    ObjectivesUpdate {
        /// What to do with `entries`.
        action: ObjectivesAction,
        /// The rows, in wire order.
        entries: Vec<Objective>,
    },
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
    /// `<cmdlist>`: dictionary rows the server is teaching us.
    ///
    /// One frame carrying its `<cli>` children, for the same reason
    /// [`Frame::MenuResponse`] is: the rows are the payload, and they used to
    /// arrive as separate `WindowHints` bags that nothing joined.
    CmdListUpdate(CmdListUpdate),
    /// `<cmdtimestamp data=>`: the dictionary version the server just sent.
    ///
    /// Separate from [`Frame::CmdListUpdate`] because the wire sends it as a
    /// separate tag, and because it is meaningful alone: a timestamp with no
    /// rows says the client is already current.
    CmdTimestamp {
        /// `data=`, as sent; empty when absent.
        version: String,
    },

    // --- dialogs and quickbar ---------------------------------------------
    /// `<switchQuickBar id=>`.
    QuickbarSwitch {
        /// `id=`: the quickbar to show; empty when absent.
        id: String,
    },
    /// `<openDialog>` / `<opendialog>`.
    DialogOpen {
        /// `id=`: the dialog's name; empty when absent.
        id: String,
        /// `title=`; always `None` when the frame came from `<dialogData>`.
        title: Option<String>,
        /// Every attribute, in wire order.
        attrs: Attrs,
    },
    /// A resident `<openDialog>` panel.
    DialogPanelOpen {
        /// `id=`: the panel's name; empty when absent.
        id: String,
        /// `title=`; `None` when absent.
        title: Option<String>,
        /// Every attribute, in wire order, `resident='true'` among them.
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
        /// `id=`; empty when absent.
        id: String,
        /// `title=`; `None` when absent.
        title: Option<String>,
        /// `target=`, entity-decoded; `None` when absent.
        target: Option<String>,
    },
    /// `<clearContainer id=>`.
    ClearContainer {
        /// `id=`: the container to empty; empty when absent.
        id: String,
    },
    /// `<inv id=>` -- one item in a container.
    ContainerItem {
        /// `id=` of the `<inv>`: the container the item is in; empty when
        /// absent.
        container_id: String,
        /// The tag's body, parsed into styled runs.
        content: Runs,
    },
    /// `<inventoryManager>`: a whole-inventory snapshot, assembled.
    ///
    /// One frame, like [`Frame::MenuResponse`] and
    /// [`Frame::ObjectivesUpdate`]: a snapshot is only a snapshot whole. See
    /// [`InventoryResponse`] for the fields and for the defect it closes.
    InventoryManager(InventoryResponse),
    /// `<inventoryViewItem>`: one item's detail, assembled across lines.
    ///
    /// The parser's only multi-line capture; `parser/view_item.rs` records
    /// why this one exists when the general path was removed. See
    /// [`ItemView`] for the fields.
    InventoryViewItem(ItemView),

    // --- misc -------------------------------------------------------------
    /// `<launchURL>` / `<LaunchURL>`. The scheme allowlist is the UI's job.
    LaunchUrl {
        /// `src=`, else `url=`, entity-decoded; empty when neither is present.
        url: String,
    },
    /// `<pulse min= max= mana=>`.
    Pulse {
        /// True only when `mana="1"`; the wire sends `0` about as often.
        mana: bool,
        /// `min=` in seconds, `46` when absent -- which it always is on the
        /// wire seen so far.
        min: u32,
        /// `max=` in seconds, `75` when absent, as `min` is.
        max: u32,
    },
    /// `<worldEvent realm= expires=>`.
    WorldEvent {
        /// `realm=`, entity-decoded; empty when absent.
        realm: String,
        /// `expires=` as a whole number; `None` when absent or not one.
        expires_min: Option<u32>,
        /// The tag's body as display text, nested markup removed.
        text: String,
    },
    /// `<PantheonStatus value=>`.
    PantheonStatus {
        /// `value=` as a whole number; `0` when absent or not a number.
        value: u32,
    },
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
    ClientCommand {
        /// The text after `<c>` in the region, e.g. `;go2 3609`:
        /// entity-decoded, control characters stripped.
        command: String,
    },
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
    UnknownTag {
        /// The tag name, as spelled on the wire.
        name: String,
        /// The tag as received; a close tag is rebuilt as `</name>`.
        raw: String,
    },
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
    Structural {
        /// The tag name, e.g. `a` for a `</a>`.
        name: String,
        /// The tag as received; a close tag is rebuilt as `</name>`.
        raw: String,
    },
    /// A tag that never closed before the line ended.
    ///
    /// Vellum appends the fragment to the text buffer and silently desyncs
    /// (`src/parser.rs:733-736`, "No closing >, treat rest as text"), with no
    /// log and no test asserting the behaviour. Typed here so the case is
    /// visible, testable, and cannot be mistaken for prose.
    MalformedTag {
        /// The unterminated text as received, through the end of the line.
        raw: String,
    },
}
