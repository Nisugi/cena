//! What the character wears, and what its wandolier holds in reserve: the
//! `inv` and `reserve` streams, typed. Lich's `GameObj.inv` and
//! `GameObj.reserve` (`lib/common/gameobj.rb:668,671`).
//!
//! # What the wire sends for `inv`
//!
//! A whole list each time, on its own stream. MEASURED in the committed cut
//! of a login (`crates/cena-protocol/tests/fixtures/inventory_container.xml:1-8`):
//!
//! ```text
//! <clearStream id='inv' ifClosed=''/><pushStream id='inv'/>Your worn items are:
//!   a <a exist="2376165" noun="arcarium">dark sapphire vangaris arcarium</a>
//!   a <a exist="2376163" noun="anklet">spiderweb-patterned nightshade anklet</a> caught with bloodwood spiders
//! <popStream/>
//! ```
//!
//! MEASURED over every `inv` block in the committed fixtures, 13 in five
//! files: all begin `Your worn items are:`; the 347 item lines all start with
//! two spaces and a non-space, carry an `exist` link, and are not bolded; 7
//! lines are blank. `VellumFE` reads the same list the same way, the first link
//! on a line being the item (`reference/VellumFE/src/core/game_objects/mod.rs:293-330`),
//! and splits off what follows `Placed alongside you:` as items at your feet,
//! from feeds its tests date 2026-01-02 and 2026-07-28 (`:685`, `:728`). No
//! committed fixture here carries that header: the split is `VellumFE`'s,
//! UNVERIFIED on this wire.
//!
//! # The list usually has no end tag
//!
//! **11 of the 13 blocks are never popped.** The three hunt fixtures under
//! `crates/cena-behavior/tests/fixtures/` refresh the list inside a combat
//! round, and every one of their 11 blocks runs on into the round's prose,
//! still on the `inv` stream, until the prompt closes it
//! (`Frame::StreamPopForced`): `arch_kill.xml:28-60` lists the 31 items and
//! then `You nock a faewood <a ...>arrow</a> ... in your <a ...>glowbark long bow</a>`.
//! Only the login's block is popped.
//!
//! So a list cannot be taken as everything its stream held. Lich does
//! (`xmlparser.rb:1183-1184`, every `<a>` while the stream is `inv`), and
//! commits at a `<popStream>` (`:586`) that these blocks never send. `VellumFE`
//! commits at the pop and throws away a list the prompt closed
//! (`core/messages/element.rs:486-493`), which here would be every list
//! refreshed mid-hunt. This reads the list **up to its first line that is not
//! a list line** and keeps it at the prompt, popped or not. The login's block
//! and the 11 hunt blocks all end cleanly that way (the census above).
//!
//! # The reserve
//!
//! Lich stages every `<a>` of the `reserve` stream as a reserved item
//! (`xmlparser.rb:556,1185-1186`, `gameobj.rb:509-512`), from `nil` until the
//! first list. **No committed fixture carries the stream's text**, so no line
//! shape is known and there is nothing to stop at: a `reserve` list the prompt
//! closed is dropped, `VellumFE`'s rule (`element.rs:507-513`), and one that
//! was popped is kept whole, every object on every line, Lich's rule. The
//! census in `tests/stream_routing.rs` counts 91 `reserve` pushes in 208 logs;
//! how many are popped is UNVERIFIED, and if most are not, [`Reserve::items`]
//! will stay `None` after a `reserve list`. A capture of `reserve list` would
//! settle both.
//!
//! # Never stated is not empty
//!
//! `None` until a list has been read; `Some` of an empty slice once the game
//! has listed nothing. Lich's `GameObj.inv` answers `nil` for both
//! (`registry_or_nil`, `gameobj.rb:1566-1568`). A reconnect keeps both lists:
//! a logged-off character wears what it wore (`reconnect.rs`), and the login
//! burst sends the `inv` list again anyway.

use super::GameState;
use super::chunks::ChunkLine;
use super::containers::ItemRef;
use super::ledger::text::objects;
use cena_protocol::runs::Runs;

/// The worn list's stream id.
pub const INV: &str = "inv";
/// The wandolier reserve's stream id.
pub const RESERVE: &str = "reserve";

/// The line that opens the worn list.
const WORN_HEADER: &str = "Your worn items are:";
/// The line after which the list is at your feet (`VellumFE`'s reading).
const AT_FEET_HEADER: &str = "Placed alongside you";

/// One list's lines, from its stream's push until the prompt.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Staging {
    /// `None` when no list is arriving.
    lines: Option<Vec<ChunkLine>>,
    /// The prompt closed the stream: no `<popStream>` came.
    torn: bool,
}

impl Staging {
    fn open(&mut self) {
        self.lines = Some(Vec::new());
        self.torn = false;
    }

    fn line(&mut self, line: &Runs) {
        if let Some(lines) = &mut self.lines {
            lines.push(ChunkLine { runs: line.clone() });
        }
    }

    fn tear(&mut self) {
        self.torn = self.lines.is_some();
    }

    /// The lines, and whether the prompt closed them.
    fn close(&mut self) -> Option<(Vec<ChunkLine>, bool)> {
        let lines = self.lines.take()?;
        Some((lines, std::mem::take(&mut self.torn)))
    }
}

/// What the character wears, as the last whole `inv` list stated.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Worn {
    /// `None` until a list is read.
    items: Option<Vec<ItemRef>>,
    /// Listed after `Placed alongside you:`.
    at_feet: Vec<ItemRef>,
    staging: Staging,
}

impl Worn {
    /// Every worn item, in the list's order. `None`: no list read yet.
    #[must_use]
    pub fn items(&self) -> Option<&[ItemRef]> {
        self.items.as_deref()
    }

    /// Items at your feet, from after `Placed alongside you:` (UNVERIFIED on
    /// this wire; see the module docs). `None`: no list read yet.
    #[must_use]
    pub fn at_feet(&self) -> Option<&[ItemRef]> {
        self.items.as_ref().map(|_| self.at_feet.as_slice())
    }

    /// Whether something with this noun is worn: bigshot's
    /// `GameObj.inv.map { |item| item.noun }.include?(noun)`
    /// (`bigshot.lic:4559`). `None`: no list read yet.
    #[must_use]
    pub fn wears(&self, noun: &str) -> Option<bool> {
        self.items()
            .map(|items| items.iter().any(|item| item.noun == noun))
    }

    /// A list arriving now is dropped: a reconnect cut it off.
    pub(super) fn drop_partial(&mut self) {
        self.staging = Staging::default();
    }

    fn close(&mut self) {
        if let Some((lines, _)) = self.staging.close()
            && let Some((worn, at_feet)) = read_inv(&lines)
        {
            self.items = Some(worn);
            self.at_feet = at_feet;
        }
    }
}

/// Read one `inv` list: its header, then item lines up to the first line that
/// is not one. `None` when it does not open with the header, a shape nobody
/// has seen, which is not taken as "wears nothing".
fn read_inv(lines: &[ChunkLine]) -> Option<(Vec<ItemRef>, Vec<ItemRef>)> {
    let mut lines = lines
        .iter()
        .map(|line| (line, line.text()))
        .filter(|(_, text)| !text.trim().is_empty());
    if lines.next()?.1.trim() != WORN_HEADER {
        return None;
    }
    let (mut worn, mut at_feet, mut feet) = (Vec::new(), Vec::new(), false);
    for (line, text) in lines {
        if text.trim_start().starts_with(AT_FEET_HEADER) {
            feet = true;
            continue;
        }
        let Some(item) = item_line(line, &text) else {
            break;
        };
        if feet {
            at_feet.push(item);
        } else {
            worn.push(item);
        }
    }
    Some((worn, at_feet))
}

/// One list line: two spaces, then text, and an object that is not bolded
/// (bold marks a creature). What the round's prose after an unpopped list
/// is not.
fn item_line(line: &ChunkLine, text: &str) -> Option<ItemRef> {
    let rest = text.strip_prefix("  ")?;
    if rest.starts_with(char::is_whitespace) {
        return None;
    }
    let (item, bold) = objects(line).into_iter().next()?;
    (!bold).then_some(item)
}

/// What the wandolier holds in reserve: Lich's `GameObj.reserve`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Reserve {
    /// `None` until a list is read.
    items: Option<Vec<ItemRef>>,
    staging: Staging,
}

impl Reserve {
    /// Every reserved item. `None`: no whole list read yet, which is when
    /// bigshot sends `reserve list` (`bigshot.lic:5987`).
    #[must_use]
    pub fn items(&self) -> Option<&[ItemRef]> {
        self.items.as_deref()
    }

    /// A list arriving now is dropped: a reconnect cut it off.
    pub(super) fn drop_partial(&mut self) {
        self.staging = Staging::default();
    }

    fn close(&mut self) {
        let Some((lines, torn)) = self.staging.close() else {
            return;
        };
        if torn {
            return;
        }
        self.items = Some(
            lines
                .iter()
                .flat_map(|line| objects(line).into_iter().map(|(item, _)| item))
                .collect(),
        );
    }
}

impl GameState {
    /// A stream opened: the `inv` or `reserve` list begins.
    pub(super) fn list_opened(&mut self, id: &str) {
        match id {
            INV => self.worn.staging.open(),
            RESERVE => self.reserve.staging.open(),
            _ => {}
        }
    }

    /// One completed line of a stream.
    pub(super) fn list_line(&mut self, id: &str, line: &Runs) {
        match id {
            INV => self.worn.staging.line(line),
            RESERVE => self.reserve.staging.line(line),
            _ => {}
        }
    }

    /// The prompt closed a stream nobody popped.
    pub(super) fn list_torn(&mut self, id: &str) {
        match id {
            INV => self.worn.staging.tear(),
            RESERVE => self.reserve.staging.tear(),
            _ => {}
        }
    }

    /// The prompt: keep what arrived.
    pub(super) fn close_lists(&mut self) {
        self.worn.close();
        self.reserve.close();
    }
}
