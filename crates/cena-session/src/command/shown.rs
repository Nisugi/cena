//! What every viewer of a session is shown of its lines, beyond the game's
//! own: `;sorter`'s sorted container looks, `.targetid`'s creature tags, and
//! the lines as the game sent them for a script runner. Each switch holds
//! across a reconnect, because every connection's actor shares the one
//! publisher that keeps it. Moved out of `handle.rs` when it reached its
//! cap.

use super::handle::SessionHandle;

impl SessionHandle {
    /// `;sorter`: publish this session's container looks one line per
    /// category, or as the game sent them (`cena_model::sorter`). Every
    /// viewer gets what is published, and it holds across a reconnect; the
    /// model and the player log keep the look whole either way.
    pub fn sort_containers(&self, on: bool) {
        self.events.sort_containers(on);
    }

    /// Whether this session's container looks are published sorted.
    #[must_use]
    pub fn sorts_containers(&self) -> bool {
        self.events.sorts_containers()
    }

    /// Show each creature's tag after its name, as `style` makes it
    /// (`None`: none), to every viewer, and take a tag the player types for
    /// its creature (`cena_model::targetid`, `.targetid`), or stop. It holds
    /// across a reconnect, as `;sorter` does.
    pub fn tag_creatures(&self, style: Option<cena_model::targetid::Style>) {
        self.events.tag_creatures(style);
    }

    /// How each creature's tag shown after its name looks; `None` when none
    /// is shown.
    #[must_use]
    pub fn tags_creatures(&self) -> Option<cena_model::targetid::Style> {
        self.events.tags_creatures()
    }

    /// Publish each finished line as the game sent it too
    /// ([`Event::Heard`](crate::Event::Heard)), for a script runner, or
    /// stop. It holds across a reconnect, as `;sorter` does.
    pub fn hear_lines(&self, on: bool) {
        self.events.hear_lines(on);
    }
}
