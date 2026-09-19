//! `KNOWN_WIRE_TAGS`: every element name the wire is known to emit.
//!
//! Ported from `reference/VellumFE/src/parser/text.rs:174-192` per `plan/13`
//! §4a. It is the union of Vellum's dispatch chain and the tag set the Saga
//! client recognizes -- knowledge that lives only in this list.
//!
//! # The count, measured (plan/05 §-2)
//!
//! ```text
//! $ sed -n '175,193p' src/parser/text.rs | grep -oE '"[^"]+"' | tr -d '"' > /tmp/tags
//! $ wc -l < /tmp/tags        # 116
//! $ sort -u /tmp/tags | wc -l # 116
//! $ LC_ALL=C sort -c /tmp/tags && echo sorted   # sorted
//! ```
//!
//! **116 entries, all distinct, ASCII-sorted.** Three figures were in
//! circulation and two are wrong: CLAUDE.md says "~130" (over by ~12%) and the
//! brief for this work said 117. The 116 here is reproduced by two independent
//! extractions and by the test below, which prints the real number when it
//! fails.
//!
//! # Seven tags added to the ported table
//!
//! The table here holds **126**: Vellum's 116, the seven below, and three
//! more from Saga 0.9.9 (see "Three tags added from Saga 0.9.9"). Of the
//! seven: Vellum's 116, plus five the Tier 2 replay
//! found in real traffic at 60, 900 and 3,000 files respectively, plus two
//! found by cross-checking the table against this crate's own dispatch arms.
//!
//! - **`c`** -- the **echo of the player's own typed command**:
//!   `<!-- CLIENT --><c>ask lu about bounty<!-- ENDCLIENT -->`. VERIFIED in
//!   `GSIV-Nisugi/2024/11/xml/2024-11-25_21-13-51.xml`: 376 occurrences, and
//!   all 376 preceded by `<!-- CLIENT -->` (`grep -o '<c>' | wc -l` and
//!   `grep -o '<!-- CLIENT --><c>' | wc -l` agree). Vellum never needed it
//!   because it renders the user's input itself; for Cena, which replays its
//!   own logs as fixtures, the echo is how a replay knows what the player did.
//! - **`map`**, **`mapInfo`** -- the automapper dialog, plain server output:
//!   `<dialogData id='mapViewMain'><map id='mapViewMain' mapId='toadwort' ...>`
//!   and `<mapInfo id='mapViewMain' pos='14017003'/>`, VERIFIED in
//!   `GSIV-Nisugi/2024/11/xml/2024-11-23_14-48-43.xml`.
//! - **`streamBox`** -- a text box inside a dialog:
//!   `<dialogData name="bugitemReport"><streamBox id="instructions" ...>`,
//!   VERIFIED in `GSIV-Nisugi/2025/01/xml/2025-01-08_00-21-29.xml`, and never
//!   inside a `<!-- CLIENT -->` region (0 of 2 occurrences).
//! - **`FEStart`** -- session start, sibling of the `FEVersion` already in the
//!   table: `<FEVersion="0" character="..." /><FEStart name="..."
//!   time="1745968462" />`, where `name` is the game's own title. VERIFIED in
//!   `GSIV-Nisugi/2025/04/xml/2025-04-29_18-07-54.xml`. Found only once the
//!   sample reached 3,000 files, which is the argument for this tier being
//!   gated-but-real rather than replaced by a fixed fixture set.
//!
//! The last two came from `every_tag_with_a_handler_arm_is_in_the_table`,
//! not from the corpus, and the distinction matters:
//!
//! - **`style`** -- the room-name / room-desc preset switch, `<style
//!   id="roomName" />` and `<style id=""/>` to close. **338,516 occurrences
//!   in a 272-file sample**, handled as markup here since the first commit,
//!   and absent from the reference's table as well, so this is an inherited
//!   gap rather than a Cena regression.
//! - **`clearDialogData`** -- has a dispatch arm, had no entry, and does not
//!   appear in that sample at all. Listed on the handler's authority.
//!
//! **Why the corpus replay could not find either.** It counts
//! `UnknownTag { name } if !is_known(name)`, but `is_known` is consulted only
//! at the END of dispatch, after every explicit name arm. A tag that has its
//! own arm never reaches the check, so its table entry is dead weight the
//! replay cannot see -- and 87 of these entries have such an arm. An attack on
//! this crate's tests demonstrated the consequence: `c`, `map`, `mapInfo`,
//! `castTime`, `nav`, `roundTime` and `indicator` were each deleted from the
//! table in turn and the gated replay stayed green on all seven. The
//! handler-arm test is what makes the table and the dispatcher one fact
//! instead of two.
//!
//! Two categories were deliberately **not** added, because neither is game
//! protocol and both are handled structurally in [`crate::parser`]:
//!
//! - The login `<settings>` blob's own 26 element names (`h`, `dc`, `cmdline`,
//!   `ignores`, `panels`, ...). It is a client-configuration document, and the
//!   region is consumed whole.
//! - Help text that merely looks like markup -- `<skill#>`, `<#ranks>`,
//!   `<stat name>`. An XML name cannot contain `#` or a space; these reach the
//!   user as text via `Frame::UnknownTag`, which is Rule 2.2 working.
//!
//! XML comments (`<!-- ... -->`) are also real wire content and are handled in
//! [`crate::parser`], not here: a comment is not an element and has no name.
//!
//! # Three tags added from Saga 0.9.9 (2026-09-18)
//!
//! Saga is Simutronics' own Electron client, so its tag set is the emitter's
//! account of its own protocol -- a better authority than the wiki, though
//! not better than the corpus. `plan/15` §6 records the source and its
//! licensing constraint: **facts only, never code or data**.
//!
//! Reconciled by set difference (Saga **118**, Cena **123**, now **126**):
//!
//! ```text
//! comm -23 saga_sorted.txt cena_sorted.txt  -> closeContainers room task
//! comm -13 saga_sorted.txt cena_sorted.txt  -> FEStart LichWebUI c
//!                                              clearDialogData group map
//!                                              mapInfo streamBox
//! ```
//!
//! All three additions measure **0 occurrences** in a 1,547-file corpus
//! sample (stride-7 over 10,824 files). That is not evidence against them:
//! §1.1 of `plan/15` records why a zero means only "this sample did not
//! contain it", and `FEStart` already sits in this table on exactly that
//! footing. Simutronics' client names them, which is stronger attestation
//! than Lich-era capture coverage.
//!
//! **Nothing was removed.** Saga 0.9.9 dropped `group` from its own set, but
//! the corpus shows `group` is live and current: 434 occurrences in that same
//! sample, in **two distinct and non-overlapping shapes** --
//! `<group id open>` client settings (60, 2024-10 to 2026-01) and
//! `<group id type state cmd name>` inside `<objectives>` (374, 2026-06
//! onward, still current). Saga removing a tag from its recognizer is a
//! statement about Saga, not about the wire. A superset parser is more
//! tolerant, never less; removal is the risky act.
//!
//! The other seven Cena-only names keep the provenance they already had.
//!
//! # Why the table stays sorted, and why the test is not decoration
//!
//! [`is_known`] is a `binary_search`, so sort order is a correctness
//! precondition, not tidiness. Vellum's own comment states the stake: a
//! mis-sorted insert makes *random other* tags report unknown, and they then
//! spray into the user's text stream. The failure is silent and affects an
//! arbitrary subset.
//!
//! `plan/05` §0 asks whether the test can go RED. It can: appending `"aaa"` to
//! the end of the list fails the `windows(2)` scan immediately. The assertion
//! is strict `<`, so it catches duplicates too -- a duplicate is harmless to
//! `binary_search` but means someone added a tag that was already there and
//! should know.
//!
//! The order is **bytewise**, so uppercase sorts before lowercase
//! (`FEVersion` ... `PantheonStatus`, then `a`). A case-insensitive sort would
//! disagree with `binary_search` and break exactly the tags it was meant to
//! tidy.

#[cfg(test)]
mod arms;

#[cfg(test)]
mod table_tests;

/// Every element name the wire is known to emit, handled or not.
///
/// MUST stay ASCII-sorted: [`is_known`] binary-searches it. See the module
/// docs for what a mis-sort actually does.
static KNOWN_WIRE_TAGS: &[&str] = &[
    "FEStart",
    "FEVersion",
    "LaunchURL",
    "LichWebUI",
    "PantheonStatus",
    "a",
    "action",
    "annotate",
    "app",
    "b",
    "br",
    "c",
    "castTime",
    "celebration",
    "checkBox",
    "clearContainer",
    "clearDialogData",
    "clearDynaStream",
    "clearStream",
    "cli",
    "closeButton",
    // Attested by Saga 0.9.9's own tag set (both 0.9.1 and 0.9.9); 0 hits in
    // a 1,547-file corpus sample. Simutronics' client names it, so it is
    // real protocol Lich-era logs never happened to capture.
    "closeContainers",
    "closeDialog",
    "closedialog",
    "cmdButton",
    "cmdlist",
    "cmdtimestamp",
    "compDef",
    "compass",
    "component",
    "container",
    "continuation",
    "crtrStatus",
    "d",
    "deleteContainer",
    "description",
    "dialogData",
    "dir",
    "dropDownBox",
    "dynaStream",
    "editBox",
    "endSetup",
    "exists",
    "exits",
    "exposeContainer",
    "exposeDialog",
    "exposeStream",
    "extra",
    "flag",
    "forcesave",
    "getSkinVersion",
    "group",
    "hScrollBar",
    "hostile",
    "i",
    "image",
    "indicator",
    "inv",
    "inventoryManager",
    "inventoryViewItem",
    "label",
    "launchURL",
    "left",
    "link",
    "macros",
    "map",
    "mapInfo",
    "menu",
    "menuImage",
    "menuLink",
    "mi",
    "mode",
    "monopolize",
    "name",
    "nav",
    "nomenu",
    "noverbupdates",
    "objective",
    "objectives",
    "openDialog",
    "opendialog",
    "output",
    "palette",
    "playerID",
    "players",
    "popBold",
    "popInputState",
    "popStream",
    "popup",
    "preset",
    "presets",
    "progressBar",
    "prompt",
    "pulse",
    "pushBold",
    "pushInputState",
    "pushStream",
    "radio",
    "resource",
    "result",
    "reward",
    "right",
    // Attested by Saga 0.9.9's tag set (and 0.9.1); 0 hits in a 1,547-file
    // corpus sample. Distinct from `roomDesc`: sorts first because bytewise
    // order puts the shorter prefix ahead.
    "room",
    "roomDesc",
    "roommeta",
    "roundTime",
    "sentSettings",
    "sep",
    "settings",
    "settingsInfo",
    "skin",
    "spell",
    "stream",
    "streamBox",
    "streamId",
    "streamWindow",
    "string",
    "style",
    "switchQuickBar",
    // ADDED by Saga between 0.9.1 and 0.9.9, alongside `action` and `reward`
    // (both already here). A `<task>` child of `<objective>` carrying quest
    // progress: text, count/max, optional units, `done`. Rare on the wire --
    // 0 hits in a 1,547-file sample, 4 files of 10,849 in a full-corpus scan
    // by the author -- but named by Simutronics' own client, and the Bounty
    // behavior will want it.
    "task",
    "timer",
    "tipInfo",
    "upDownEditBox",
    "updateverbs",
    "vScrollBar",
    "worldEvent",
];

/// Is this element name one the wire is known to emit?
///
/// A `false` here does not mean the tag is invalid -- it means Simutronics
/// changed the protocol, which is precisely what Rule 2.2 (`plan/05:276-283`)
/// requires be surfaced rather than swallowed.
#[must_use]
pub fn is_known(name: &str) -> bool {
    KNOWN_WIRE_TAGS.binary_search(&name).is_ok()
}

/// How many tags the table holds. Exposed so a test can assert the count
/// without the table being public and mutable-looking.
#[must_use]
pub fn known_count() -> usize {
    KNOWN_WIRE_TAGS.len()
}
