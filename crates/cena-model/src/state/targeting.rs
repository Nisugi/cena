//! What the game says you can attack: the `dDBTarget` dropdown.
//!
//! # Why a display widget is a model fact
//!
//! > **AUTHOR, 2026-09-21:** *"if you look at lich dDBTarget is part of the
//! > equation how it determines an npc is hostile or not, before the hostile
//! > flag of course."*
//!
//! The `combat` dialog's target dropdown is the **noisiest widget on the
//! wire** -- MEASURED over one live session: 2,031 `<dialogData id='combat'>`
//! and 1,034 `dDBTarget` rows -- and the model read none of it.
//!
//! It is not decoration. The server decides what goes in that list, so an id
//! appearing there is the game's own statement that the thing is a legitimate
//! attack target. Lich reads it at `xmlparser.rb:773-786` and builds three
//! answers on top (`gameobj.rb:1147-1178`):
//!
//! | Lich | What it means |
//! |---|---|
//! | `GameObj.targets` | targeted ids that ARE known NPCs, minus dead and appendages |
//! | `GameObj.hidden_targets` | targeted ids matching **no known NPC** |
//! | `GameObj.target` | the single id the dropdown's `value` names |
//!
//! `hidden_targets` is the one that earns this module. An id the game will let
//! you attack, that appears in no room list, is **something you cannot see** --
//! which is exactly the inference `overwatch.rb:117-120` feeds: it pushes a
//! revealed id onto the same list.
//!
//! # `content_value` is a LIST, and reading it as one id was my first mistake
//!
//! MEASURED on the wire:
//!
//! ```text
//! <dropDownBox id='dDBTarget' value="giant warg" content_value="#94076783" .../>
//! <dropDownBox id='dDBTarget' value="none" content_value="target help" .../>
//! ```
//!
//! `xmlparser.rb:776` splits on commas and keeps every `#<digits>` entry. A
//! single id is the common case, not the shape -- so this splits, and `#-12345`
//! (a negative id, which the game uses for players) is accepted because Lich's
//! own pattern is `\#(\-?\d+)`.
//!
//! **The current target is NOT "the first id in the list".** Lich sets it
//! from a second, separate match against the RAW attribute
//! (`xmlparser.rb:781-785`): `content_value =~ /^\#(\-?\d+)(?:,|$)/`, so
//! there is a current target only when the value *starts* with an id. This
//! doc said "then takes the first as the current one", and the code did that
//! after the filter -- so `none,#123` reported 123 as targeted when the
//! dropdown's own selection was `none`. The list and the selection are two
//! readings of one attribute, and only the first entry is the selection.
//!
//! **`value="none"` and `content_value="target help"` are the empty state.**
//! Neither is an id, so both produce an empty list -- and an empty list that
//! the game HAS stated is different from never having been told, which is why
//! `Targeting`'s private `stated` flag exists, and why
//! [`Targeting::is_targetable`] answers `Option<bool>` rather than `bool`
//! (`plan/12` section 5.2).
//!
//! # This says nothing about hostility on its own
//!
//! Being targetable is evidence, not a verdict: a player is targetable too.
//! The `hostile` flag stays where it is, and [`Targeting`] is one input to it
//! rather than a replacement -- which is the order the author gave, *"before
//! the hostile flag of course"*.

/// The ids the game currently lists as attackable, newest statement wins.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Targeting {
    /// Every targetable id, in the order the dropdown listed them.
    ids: Vec<i64>,
    /// The id the dropdown's own `value` names: the first, per
    /// `xmlparser.rb:781-785`.
    current: Option<i64>,
    /// The display text, e.g. `giant warg`. `None` for `none`.
    name: Option<String>,
    /// Whether the game has ever sent the dropdown.
    stated: bool,
}

impl Targeting {
    /// Read a `dDBTarget` dropdown's `content_value` and `value`.
    ///
    /// Returns whether anything changed, so a consumer can ignore the 1,000
    /// identical restatements a session sends.
    pub fn read(&mut self, content_value: &str, value: Option<&str>) -> bool {
        let ids = parse_ids(content_value);
        let current = current_id(content_value);
        let name = value.filter(|v| *v != "none").map(str::to_owned);
        let changed =
            !self.stated || self.ids != ids || self.current != current || self.name != name;
        self.ids = ids;
        self.current = current;
        self.name = name;
        self.stated = true;
        changed
    }

    /// Every id the game says is targetable. Empty once stated means the game
    /// said there is nothing.
    #[must_use]
    pub fn ids(&self) -> &[i64] {
        &self.ids
    }

    /// The current target's id, as the dropdown's `value` names it.
    #[must_use]
    pub const fn current(&self) -> Option<i64> {
        self.current
    }

    /// The current target's display name.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.name.as_deref()
    }

    /// Whether this id is one the game will let you attack.
    ///
    /// `None` until the game has sent a dropdown: "not told" is not "no".
    #[must_use]
    pub fn is_targetable(&self, id: i64) -> Option<bool> {
        self.stated.then(|| self.ids.contains(&id))
    }

    /// Whether the game has stated the list.
    #[must_use]
    pub const fn is_stated(&self) -> bool {
        self.stated
    }

    /// Targetable ids that are in none of the given known ids.
    ///
    /// Lich's `GameObj.hidden_targets` (`gameobj.rb:1171`). **Something the
    /// game will let you attack that appears in no room list is something you
    /// cannot see** -- the inference `overwatch.rb` feeds from the other side.
    ///
    /// Takes the known ids rather than reaching for `Creatures`, so this stays
    /// a plain question about two lists and the caller decides what "known"
    /// means.
    pub fn hidden<'a>(&'a self, known: &'a [i64]) -> impl Iterator<Item = i64> + 'a {
        self.ids.iter().copied().filter(|id| !known.contains(id))
    }

    /// Forget it: a reconnect. The login burst re-sends the `combat` dialog.
    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Every `#<id>` in a `content_value`, in order.
///
/// `xmlparser.rb:776-780`: split on commas, keep what matches `\#(\-?\d+)`.
/// `target help` and any other non-id entry is skipped rather than refused --
/// the wire sends it as the empty state.
#[must_use]
pub fn parse_ids(content_value: &str) -> Vec<i64> {
    content_value
        .split(',')
        .filter_map(|part| part.trim().strip_prefix('#')?.parse().ok())
        .collect()
}

/// The dropdown's selected id: the FIRST entry of the raw `content_value`,
/// and only if that entry is an id.
///
/// `xmlparser.rb:781-785`, which anchors at `^` on the unsplit attribute. Not
/// `parse_ids(..).first()`: that skips a leading non-id entry and promotes the
/// next id to "current" -- `none,#123` would answer 123.
fn current_id(content_value: &str) -> Option<i64> {
    let first = content_value.split(',').next()?;
    first.strip_prefix('#')?.parse().ok()
}

impl crate::GameState {
    /// Read the widgets of one `<dialogData>`.
    ///
    /// Only `dDBTarget` today. A `match` on the widget id rather than a
    /// growing chain, because the next one that matters (the ammo dropdown,
    /// the stance bar) lands the same way.
    ///
    /// **Here rather than in `state.rs` under Rule 4.1** -- move code down, do
    /// not raise the cap. It is a reader for *this* module's `Targeting`, so
    /// the dispatcher in `apply` calls it and the work lives beside the state
    /// it maintains. `state.rs` had been sitting at exactly its 550 cap.
    pub(super) fn read_widgets(&mut self, widgets: &cena_protocol::frame::DialogWidgets) {
        if widgets.kind != "dropDownBox" {
            return;
        }
        for attrs in &widgets.widgets {
            let get = |name: &str| {
                attrs
                    .iter()
                    .find(|(k, _)| k == name)
                    .map(|(_, v)| v.as_str())
            };
            if get("id") == Some("dDBTarget") {
                self.targeting
                    .read(get("content_value").unwrap_or_default(), get("value"));
            }
        }
    }
}
