//! A widget: one thing a player reads about one character, drawn bare -- no
//! title, no frame -- into the space it is given (`plan/49` §2). Its frame
//! comes from what holds it: a standalone window gives it one, and a custom
//! window holds it bare beside others (`plan/28` §7d, §7d.3).
//!
//! The author: *"individual things, a bar for health is a widget, a bar for
//! stamina is a widget, each hands are their own widget"* (`plan/49` §1). So
//! the catalog is of small things, and Saga's composite panels are presets
//! built from them.
//!
//! A closed enum, not a trait: every kind lives here, and a trait object
//! would buy nothing (`plan/05` §−1). Each kind says its name, the size it
//! would like, and draws itself; what holds it decides where.

mod character;
mod described;
mod dialog;
pub(crate) mod doll;
mod doll_text;
mod draw;
pub(crate) mod find;
#[cfg(feature = "doll-infinite")]
pub(crate) mod infinite;
mod kind;
mod lines;
mod listed;
mod lists;
pub(crate) mod minimap;
mod room;
mod split;
mod state;
mod status;

use std::sync::Arc;

use cena_session::Snapshot;
use cena_ui::HuntView;
use egui::Id;

use crate::story::Story;
pub(crate) use described::RoomParts;
pub(crate) use kind::{Group, Widget};
pub(crate) use lines::{Lines, Stamps, line_under};
pub(crate) use listed::Listing;
pub(crate) use split::{Scroll, ask as ask_scroll};
pub(crate) use status::{Category, Indicator};

/// What a widget draws from: one character, as its play window has it.
#[derive(Clone, Copy)]
pub(crate) struct Seen<'a> {
    /// What it knows, once its feed has seen anything.
    pub(crate) snapshot: Option<&'a Snapshot>,
    /// Its story and Hydra's messages.
    pub(crate) story: &'a Story,
    /// What its hunt is doing, when one runs.
    pub(crate) hunt: Option<&'a HuntView>,
    /// Whose it is, when the widget follows another character than its
    /// window's, which it then names (`plan/49` §1 row 3).
    pub(crate) who: Option<&'a str>,
    /// The streams a widget in this window shows, whose lines the story
    /// then leaves out (`plan/49` Stage B step 4).
    pub(crate) open: &'a [String],
    /// Where it is on the map, as the minimap draws it (`plan/53`); `None`
    /// without a map.
    pub(crate) minimap: Option<&'a cena_ui::MinimapView>,
    /// How long each creature's tag after its name is, when shown
    /// (`.targetid`, `cena_session::targetid`).
    pub(crate) tags: Option<usize>,
}

/// Another character running in this Hydra, as a widget that follows it
/// sees it: a party's vitals, say, in the Advanced place (`plan/49` §1 row
/// 3). Never its story, which no other window shows.
#[derive(Clone, Debug)]
pub(crate) struct Character {
    /// Its name.
    pub(crate) name: String,
    /// What it knows, once its feed has seen anything.
    pub(crate) snapshot: Option<Arc<Snapshot>>,
    /// What its hunt is doing, when one runs.
    pub(crate) hunt: Option<HuntView>,
}

impl Widget {
    /// How much it has said, ever, when it is a stream of lines: a tab not
    /// showing counts what came since it last did (`plan/49` §2). `None`
    /// for the rest, which have nothing to count.
    pub(crate) fn count(&self, seen: &Seen<'_>) -> Option<u64> {
        match self {
            Widget::Story => Some(seen.story.heard),
            Widget::Hydra => Some(seen.story.told),
            Widget::Stream(id) => Some(seen.story.streams.get(id).map_or(0, |kept| kept.heard)),
            _ => None,
        }
    }

    /// Draw it into `ui`, bare, filling what it is given. `id` is its own,
    /// so two of one kind keep apart what they remember -- a scroll, say.
    /// A line the player asked it to send, as if typed -- a compass's
    /// direction -- if one was.
    #[cfg(test)]
    pub(crate) fn draw(&self, ui: &mut egui::Ui, seen: &Seen<'_>, id: Id) -> Option<Clicked> {
        draw::draw(self, ui, seen, id, &Chosen::default())
    }

    /// [`Self::draw`], as the player chose on its own page (each kind's own
    /// otherwise).
    pub(crate) fn draw_with(
        &self,
        ui: &mut egui::Ui,
        seen: &Seen<'_>,
        id: Id,
        chosen: &Chosen,
    ) -> Option<Clicked> {
        draw::draw(self, ui, seen, id, chosen)
    }
}

/// What a click in a widget asked for.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Clicked {
    /// A line to send, as if typed: a compass's direction.
    Send(String),
    /// A line to send without an echo: an object carried and let go,
    /// `_drag #<item> <onto>` (`carry.rs`).
    Quietly(String),
    /// A link in a line of the game's, clicked at a place on the screen.
    Link(cena_ui::RunLink, egui::Pos2),
    /// A room clicked on the minimap to go to, its route shown; `None`
    /// to forget it.
    Aim(Option<u32>),
    /// One of Hydra's own commands, its symbol put in front: `go2 228`
    /// echoed as if typed, or `room 228` said with no echo.
    Hydra {
        /// The command, without its symbol.
        word: String,
        /// Whether the story shows it, as it shows what was typed.
        echo: bool,
    },
}

impl From<crate::text::Acted> for Clicked {
    fn from(acted: crate::text::Acted) -> Self {
        match acted {
            crate::text::Acted::Clicked(link, at) => Self::Link(link, at),
            crate::text::Acted::Quietly(line) => Self::Quietly(line),
        }
    }
}

/// What the player chose for one placed widget on its own page, whichever
/// its kind takes: a bar's look, the Room's parts, how lines are drawn.
#[derive(Clone, Debug, Default)]
pub(crate) struct Chosen {
    /// A bar's look.
    pub(crate) look: Option<crate::bar::Look>,
    /// The Room widget's parts.
    pub(crate) room: Option<RoomParts>,
    /// How the story or a stream draws its lines.
    pub(crate) lines: Option<Lines>,
    /// The injury doll's picture.
    pub(crate) doll: Option<doll::DollLook>,
    /// The minimap's zooms and maps next door.
    pub(crate) minimap: Option<minimap::MinimapLook>,
    /// How a room list lays out its names.
    pub(crate) list: Option<listed::Listing>,
}

/// One line of text, or a bar: what a one-line widget asks for.
pub(crate) const LINE: f32 = 20.0;

#[cfg(test)]
mod tests;
