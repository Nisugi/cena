//! Room contents: **M2 step 2**, `plan/18` §2c.
//!
//! Split out of `state.rs` under Rule 4.1 (`plan/05:352-353`) -- move code down,
//! do not raise the cap.
//!
//! # The room is several independent feeds
//!
//! > **AUTHOR, 2026-09-19:** *"they all need to save/buffer/update
//! > independently, because you go in a room and you will get constant room
//! > object updates as creatures come in and out, or player updates if a player
//! > comes in and out, it's the real time feed right."*
//!
//! MEASURED, and the numbers make it a correctness requirement rather than a
//! preference: `<component id='room objs'>` arrives **6,778** times against
//! `<compDef id='room objs'>`'s 3,002, and **4,445** standalone `<component>`
//! updates arrive ALONE against 2,773 in groups. **191 carry an empty body** --
//! "nobody here" as a real update, which a single-blob room cannot express
//! distinctly from "not mentioned".
//!
//! So: a map of raw bodies keyed by component id, plus typed collections derived
//! from them. `VellumFE` takes the same split -- `room_components: HashMap<String,
//! Vec<Vec<TextSegment>>>` (`core/app_core/state.rs:282`) beside `room_creatures`
//! / `room_objects` / `room_players` (`core/state.rs:127-151`).
//!
//! # Creatures are the bold entries of `room objs`
//!
//! There is no separate creature feed. One component carries both and
//! `<pushBold/>` is the only thing that separates them, VERIFIED on the wire:
//!
//! ```text
//! You also see<b> <pushBold/>a <a exist="412621" noun="lookout">wary-eyed
//! halfling lookout</a><popBold/></b>, a <a exist="18122889" noun="arrow">wooden
//! arrow</a>.
//! ```
//!
//! Ported from `VellumFE` (`core/messages/component.rs:202-204`). Cena's parser
//! already keeps `bold_depth` on every run, so this reads a fact rather than
//! re-deriving one.
//!
//! # No unchanged-check, deliberately
//!
//! Vellum skips a component whose value is unchanged and then has to exempt
//! `sprite`, because *"the game sends it EMPTY on every room change, so
//! 'unchanged' would short-circuit before room-art injection runs"*
//! (`component.rs:165-171`). Cena adds no such check: `room players` going empty
//! and staying empty is two real updates, and 191 empty bodies in the sample is
//! exactly what such a check would eat. If one is ever added it must be justified
//! per component, and `tests/room_contents.rs` has the test that will catch it.

use cena_protocol::frame::LinkKind;
use cena_protocol::runs::Runs;

/// One interactable thing in the room: a creature, an object, or a player.
///
/// The three fields are the three questions a consumer asks, and all three come
/// off one `<a exist= noun=>` link, so none may be dropped: [`Self::noun`] is
/// what a command targets, [`Self::id`] is what the noun registry
/// (`plan/18` step 5) will key on, and [`Self::text`] is what a player reads.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomItem {
    /// `exist=`, **verbatim**.
    ///
    /// A `String` rather than a number: ids are negative for room fixtures and
    /// for players (`exist="-495692"`), and the wire promises no numeric range.
    /// Parsing would either lose the sign or assert a type the protocol does not.
    pub id: String,
    /// `noun=`. What a command targets it by.
    pub noun: String,
    /// The link's display text, e.g. `wary-eyed halfling lookout`.
    pub text: String,
    /// What the room says this player is doing: `hiding`, `sitting`, `dead`.
    ///
    /// **Only ever set for `room players`**, and only when the room said so.
    /// Ports `GameObj#status` for PCs (`gameobj.rb:311-317`), which Lich fills
    /// from `xmlparser.rb:1152-1155`.
    ///
    /// The status is **prose outside the `<a>` link**, so it is not an
    /// attribute and cannot be read off the link at all:
    ///
    /// ```text
    /// Also here: <a exist="-1" noun="Demandred">Demandred</a> who is hiding, ...
    /// ```
    ///
    /// `None` means the room named this player and said nothing further --
    /// which is the ordinary case, standing and visible. It does not mean
    /// "unknown": the roster states every occupant's condition or states none,
    /// so absence here is the game saying "nothing to report".
    pub status: Option<PlayerStatus>,
}

/// What a player in the room is doing.
///
/// Two wire forms, both at `xmlparser.rb:1152`:
///
/// - `who is hiding` / `who appears dead` -- a clause after the link
/// - `(hiding)` -- parenthesised, and the wire may carry **two**
///
/// Kept as the joined text rather than an enum. Lich concatenates multiple
/// statuses with a space (`:1153-1154`) and never matches the result against a
/// closed set, and the vocabulary is the game's: `hiding`, `sitting`,
/// `kneeling`, `lying down`, `stunned`, `dead`, `sleeping`, and combinations.
/// C21 reserves typed variants for closed vocabularies, and this is not one --
/// a `stunned Seaward Gutstorm who is lying down` appears in the reference logs
/// and would need a variant nobody enumerated.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct PlayerStatus(String);

impl PlayerStatus {
    /// The status text as the room stated it, e.g. `hiding`, `lying down`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Is this player hiding?
    ///
    /// The one status a hunting check cares about, and the reason this field
    /// exists: a hiding **group member** is named in the roster, unlike a
    /// hiding stranger, who is reported only as a nameless sign in `room objs`
    /// (see [`crate::state::claim`]).
    #[must_use]
    pub fn is_hiding(&self) -> bool {
        self.0.split_whitespace().any(|word| word == "hiding")
    }
}

impl std::fmt::Display for PlayerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// The room the character is in, as the wire stated it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Room {
    /// `<nav rm=>`. The game's own id, not a guess from the title.
    pub id: Option<String>,
    /// The room's name, from `<streamWindow id='room' subtitle=>`.
    ///
    /// As the wire states it, less the ` - ` every subtitle leads with:
    /// `Rawknuckle's, Watering Hole`, and `... - 7503251` after it when the
    /// player has room numbers shown. **Not** the map's bracketed spelling and
    /// not stripped of that number -- both are the map's business
    /// (`cena_map::title_from_subtitle`), and this crate does not know a map
    /// exists. Kept because identifying a room the map has no number for
    /// starts from its name (`plan/21` §5 step 4); it was parsed and dropped.
    ///
    /// Arrives *after* `<nav>`, so it is `None` for a moment on every arrival.
    pub title: Option<String>,
    /// `<compDef id='room desc'>`, body parsed.
    ///
    /// Kept as its own field because criterion 2 reads it; it is **also** in
    /// [`Self::components`], so a renderer drawing the room window by id needs no
    /// special case for the one component that is different.
    pub description: Option<Runs>,
    /// `<compass><dir value=>`. Direction tokens, not prose.
    ///
    /// # Three states, not two
    ///
    /// `None` is **not yet observed**; `Some(vec![])` is **observed, and there
    /// are no cardinal exits**. A bare `Vec` collapses those, which is
    /// `plan/19` pattern A -- an `Option` (or here, a collection) read as two
    /// states where three exist. Recorded there as §1c, OPEN, "the same shape"
    /// as the roundtime bug that made a gated send fire early.
    ///
    /// **Both of the empty cases are real**, and they are different from each
    /// other as well as from `None` (the author, 2026-09-19):
    ///
    /// - a room whose only way out is a **portal, door or teleport** rather
    ///   than a cardinal direction. The compass is empty and the room is still
    ///   exitable.
    /// - a room with genuinely no exits at all -- the consultation lounge.
    ///
    /// So an empty compass is a fact the server stated, and the distinction
    /// matters to anything that would route: "I have not looked yet" invites a
    /// look, while "there are no cardinal exits" says to find the door.
    ///
    /// Invalidated on reconnect (`§5.2`), which is what makes `None`
    /// reachable in a live session rather than only at startup.
    pub exits: Option<Vec<String>>,
    /// The bold entries of `room objs`: creatures.
    pub creatures: Vec<RoomItem>,
    /// The non-bold entries of `room objs`: items and fixtures.
    pub objects: Vec<RoomItem>,
    /// The entries of `room players`.
    pub players: Vec<RoomItem>,
    /// `<roommeta>`: the room's environment, as the game's own codes.
    ///
    /// `None` until the wire states it. The codes are not decoded -- see
    /// [`cena_protocol::RoomMeta`] for why -- but `sanctuary` is the one a
    /// behavior asks first, and it was sitting in an untyped attribute bag
    /// that nothing consumed.
    pub meta: Option<cena_protocol::RoomMeta>,
    /// Raw component bodies, keyed by component id.
    ///
    /// What a renderer draws -- prose, punctuation and all. The typed collections
    /// above answer "what can I interact with"; this answers "what does the room
    /// window say". Private so that an **unseen** component reads as absent
    /// through [`Self::component`] rather than being confused with an empty one.
    ///
    /// `BTreeMap` for criterion 7: a `HashMap`'s iteration order varies run to
    /// run, and a replay asserting over one would be non-deterministic.
    components: std::collections::BTreeMap<String, Runs>,
}

impl Room {
    /// A room the character has just entered: its id, and nothing else known.
    ///
    /// A constructor rather than `Room { id, ..default }` because
    /// [`Self::components`] is private -- and that privacy is the point. It is
    /// what makes "never told" distinguishable from "empty", so a caller outside
    /// this module must not be able to build a `Room` that claims to have been
    /// told things it has not.
    #[must_use]
    pub fn entering(id: Option<String>) -> Self {
        Self {
            id,
            ..Self::default()
        }
    }

    /// One component's raw body, if the game has sent it.
    ///
    /// **`None` is "never told", not "empty".** Unlike a stream buffer, where the
    /// two are worth collapsing, here they differ and both occur: `sprite` is
    /// EMPTY on every room change, and 191 bodies in the measured sample are
    /// empty updates that mean something.
    #[must_use]
    pub fn component(&self, id: &str) -> Option<&Runs> {
        self.components.get(id)
    }

    /// Every component the game has sent for this room, in id order.
    pub fn components(&self) -> impl Iterator<Item = (&str, &Runs)> {
        self.components.iter().map(|(id, body)| (id.as_str(), body))
    }

    /// Whether the game has said who is in the room.
    ///
    /// The three-state problem [`Self::players`] alone cannot answer: an empty
    /// vector is both "nobody is here", which the wire says 135 times in the
    /// sample, and "we have not been told". A behavior deciding whether it is
    /// alone must be able to tell those apart.
    #[must_use]
    pub fn saw_players(&self) -> bool {
        self.components.contains_key("room players")
    }

    /// Forget who and what is standing here, keeping the place itself.
    ///
    /// **The split a reconnect needs** (`reconnect.rs`, 2026-09-20). `id`,
    /// `title`, `description` and `exits` describe somewhere that cannot have changed
    /// while the character was out of the world. The roster is the opposite:
    /// **other people are still online**, so creatures wander and players come
    /// and go whether or not we are watching.
    ///
    /// Each roster entry carries an `exist` id a behavior can target, so a
    /// stale one is a live handle to something that is not there -- not a
    /// cosmetic error.
    ///
    /// Clears the raw component bodies too, which is what makes
    /// [`Self::saw_players`] answer "we have not been told" rather than
    /// "nobody is here". The typed lists alone cannot express that difference,
    /// which is the whole reason that method exists.
    pub(super) fn forget_contents(&mut self) {
        let Self {
            id,
            title,
            description,
            exits,
            creatures,
            objects,
            players,
            components,
            meta,
        } = self;

        // Kept: the place.
        let _ = (id, title, description, exits);
        // Kept: the environment is a fact about the ROOM, not about who is
        // standing in it. A room does not stop being a sanctuary because
        // the roster went stale, and the next `<roommeta>` restates it.
        let _ = meta;

        // Cleared: who else is in it.
        creatures.clear();
        objects.clear();
        players.clear();
        components.retain(|key, _| {
            !matches!(
                key.as_str(),
                "room objs" | "room players" | "room creatures"
            )
        });
    }

    /// Fold one `<component>` / `<compDef>` body into the room.
    ///
    /// The two spellings are **one feed**: `compDef` is the room-entry snapshot
    /// and `component` is the live update, carrying the same id and the same body
    /// shape. The parser already emits `Frame::Component` for both.
    ///
    /// Each id **replaces** its own entry and touches no other. That is the
    /// author's requirement and the measurement behind it: updates arrive alone
    /// 4,445 times against 2,773 in groups.
    pub(super) fn apply_component(&mut self, id: &str, body: &Runs) {
        match id {
            "room desc" => self.description = Some(body.clone()),
            "room objs" => {
                // Creatures are the BOLD entries, objects the rest. One feed,
                // separated by `<pushBold/>` and nothing else.
                self.creatures = items(body, true);
                self.objects = items(body, false);
            }
            "room players" => self.players = items_with_status(body, false, true),
            _ => {}
        }
        self.components.insert(id.to_owned(), body.clone());
    }
}

/// The linked entries of a component body, bold or not-bold.
///
/// Only `<a exist= noun=>` links become items: the prose between them
/// (`You also see`, `, a `, `.`) is for a renderer, and the raw body keeps it.
/// A `<d>` link is a command rather than a thing, so it is not an item either --
/// which is what keeps `room exits`' `<d>north</d>` out of the object list.
fn items(body: &Runs, bold: bool) -> Vec<RoomItem> {
    items_with_status(body, bold, false)
}

/// As [`items`], but reading each link's trailing status clause.
///
/// Only `room players` passes `with_status`: the clause is a property of a
/// person, and reading it for objects would attribute `who is hiding` to a
/// disk that merely followed a hiding player in the list.
fn items_with_status(body: &Runs, bold: bool, with_status: bool) -> Vec<RoomItem> {
    let runs: Vec<_> = body.runs.iter().collect();
    runs.iter()
        .enumerate()
        .filter(|(_, run)| (run.style.bold_depth > 0) == bold)
        .filter_map(|(index, run)| {
            let link = run.link.as_ref()?;
            let LinkKind::Exist { id, noun } = &link.kind else {
                return None;
            };
            // The status is prose in the NEXT run, because it sits outside the
            // link. `runs` is in wire order, which is what makes this readable
            // without re-parsing markup (Rule 2.1).
            let status = if with_status {
                runs.get(index + 1)
                    .filter(|next| next.link.is_none())
                    .and_then(|next| parse_status(&next.text))
            } else {
                None
            };
            Some(RoomItem {
                id: id.clone(),
                noun: noun.clone(),
                text: link.text.clone(),
                status,
            })
        })
        .collect()
}

/// Read a status clause from the prose that follows a player's link.
///
/// Ports `xmlparser.rb:1152`'s two alternatives:
///
/// ```ruby
/// text_string =~ /^ who (?:is|appears) ([\w\s]+)(?:,| and|\.|$)/
/// text_string =~ / \(([\w\s]+)\)(?: \(([\w\s]+)\))?/
/// ```
///
/// Both are ported; the parenthesised form may carry two statuses, which Lich
/// joins with a space (`:1153-1154`) and so does this.
///
/// Returns `None` for ordinary separators (`, `, `.`, ` and `), which is most
/// of what follows a link.
fn parse_status(text: &str) -> Option<PlayerStatus> {
    // `who is hiding,` / `who appears dead.`
    if let Some(rest) = text
        .strip_prefix(" who is ")
        .or_else(|| text.strip_prefix(" who appears "))
    {
        let status: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || c.is_whitespace())
            .collect();
        let status = status.trim();
        if !status.is_empty() {
            return Some(PlayerStatus(status.to_owned()));
        }
    }

    // ` (hiding)` or ` (hiding) (stunned)`, joined as Lich joins them.
    let mut parts: Vec<&str> = Vec::new();
    let mut rest = text;
    while let Some(open) = rest.find('(') {
        let Some(close) = rest[open..].find(')') else {
            break;
        };
        let inner = rest[open + 1..open + close].trim();
        if !inner.is_empty() && inner.chars().all(|c| c.is_alphanumeric() || c == ' ') {
            parts.push(inner);
        }
        rest = &rest[open + close + 1..];
    }
    if parts.is_empty() {
        None
    } else {
        Some(PlayerStatus(parts.join(" ")))
    }
}

impl super::GameState {
    /// A room component: the room takes it, and a `room objs` body also
    /// rebuilds the creature roster from the bold links it just parsed.
    /// Fold one room component, replacing only its own entry.
    ///
    /// `plan/18` §2c and the author's requirement: the room is several
    /// independent feeds, so a `room players` update must not disturb the
    /// objects.
    ///
    /// # `compDef`/`component` ONLY
    ///
    /// The room arrives in two shapes and they mean different things (author,
    /// 2026-09-18, from live traffic):
    ///
    /// * `<compDef id='room desc'>` feeds the ROOM WINDOW and is truth for
    ///   WHERE THE CHARACTER IS. It is also truth for the creatures, objects
    ///   and players in the room, each in its own `compDef`, which is why the
    ///   window feed stays structured.
    /// * inline text styled `<style id="roomDesc"/>` feeds the STORY WINDOW
    ///   and is only WHAT THE CHARACTER SAW. It is flattened: the live capture
    ///   shows `room objs` appended into the prose rather than kept separate.
    ///
    /// **Abilities that look into another room emit the story form WITHOUT
    /// the window form.** That asymmetry is not noise -- it is exactly how a
    /// client knows the character did NOT move. Folding the styled text here
    /// would turn every scry into a phantom relocation, and the bug would only
    /// appear for players who use those abilities.
    ///
    /// So `look` does not update `room.description`, by design. A consumer
    /// that wants the looked-at prose reads the published `Frame::Text`;
    /// `GameState` tracks location, not narration.
    pub(super) fn apply_room_component(&mut self, id: &str, body: &Runs) {
        self.room.apply_component(id, body);
        if id == "room objs" {
            let now = self.game_time_now();
            self.creatures.apply_room_objs(&self.room.creatures, now);
        }
    }

    /// A `<crtrStatus>`. The flags precede the link they describe inside a
    /// `room objs` body, so the registry holds them until the body names
    /// the creature.
    pub(super) fn apply_creature_status(&mut self, id: &str, attrs: &cena_protocol::frame::Attrs) {
        let now = self.game_time_now();
        let status = super::creature::status::CreatureStatus::from_attrs(
            id,
            attrs.iter().map(|(k, v)| (k.as_str(), v.as_str())),
        );
        self.creatures.note_status(status, now);
    }

    /// A `<streamWindow>`: the `room` window's subtitle names the room.
    ///
    /// Only `id='room'`. The `main` window carries the same subtitle, but it
    /// is the story window's caption, and every other window's subtitle is
    /// about that window (`percWindow`'s is ` - [Active]`).
    pub(super) fn name_room(&mut self, window: &str, subtitle: Option<&str>) {
        if window == "room"
            && let Some(subtitle) = subtitle
        {
            let name = subtitle.strip_prefix(" - ").unwrap_or(subtitle);
            self.room.title = Some(name.to_owned());
        }
    }

    /// Handle `<nav>`: an arrival, or a re-declaration of the room we are in.
    ///
    /// Split out of [`Self::apply`] under Rule 4.1 -- adding the same-room
    /// guard took that function to 101 lines against its 100 cap, and the rule
    /// is to move code down rather than raise the limit.
    ///
    /// A new room invalidates the old description and exits: they describe
    /// somewhere the character no longer is, and keeping them is how a
    /// consumer renders the previous room's exits under the new room's name.
    /// EVERYTHING goes, including the per-component buffers and the typed
    /// collections -- creatures from the last room are the most dangerous
    /// thing to keep, because a behavior would attack them.
    ///
    /// **Unless it is the SAME room, in which case this is a re-declaration
    /// and not an arrival.** Found by review: the login burst sends
    /// `<nav rm='7086'/>` for the room the character is already in and carries
    /// no `compass` to replace what a full reset throws away -- so a reconnect
    /// that had just been taught to KEEP the exits lost them to the burst one
    /// frame later.
    ///
    /// Guarded on `Some`, because a bare `<nav/>` cannot be compared: two
    /// consecutive id-less arrivals are two different rooms as far as anything
    /// here can tell, and treating them as one would keep a previous room's
    /// creatures. **Unknown is not equal to unknown.**
    pub(super) fn arrive(&mut self, id: Option<&str>) {
        let same_room = id.is_some() && id == self.room.id.as_deref();
        if !same_room {
            self.room = Room::entering(id.map(str::to_owned));
            // The creature roster is room contents too (`xmlparser.rb:410`).
            self.creatures.on_nav();
        }
    }
}
