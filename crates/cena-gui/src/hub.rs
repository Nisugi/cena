//! The hub: every character this Hydra runs, at a glance, in two tabs
//! (`plan/47` §3), and a third holding those not launched (`launch.rs`,
//! `plan/49` Stage C). The author: *"we probably don't want to clutter the live cards
//! with the closed cards. so tab for closed and tab for live?"*
//!
//! A card is [`SessionCard`], the web hub's own, so both hubs show the same
//! thing from the same projection and nothing is copied (`plan/47` §4). So is
//! what the hub asks: a [`HubRequest`], answered by the binary, the one owner
//! of the session table. The words are Despana's (`cena-web/assets/app.js`),
//! so the two hubs say the same things.
//!
//! Live is a character whose connection is open or being retried; Closed is
//! one that stopped by itself this run -- a refused login, or idle -- with
//! why, to reconnect or remove. One the player quits is taken off the table,
//! so it leaves both.
//!
//! Cards tile across the tab, each as wide as its four bars until a side is
//! dragged, which sets every card's width (the author, 2026-09-27: *"the
//! width of the 4 bars there by default no auto stretch, then manually
//! adjustible by dragging a side. They should tile or grid on the panel
//! depending on window size"*).

use cena_ui::{
    GroupView, HubRequest, LifecycleView, Listing, MergedLine, RosterCard, SessionCard, VitalView,
};

use crate::bar::{self, Amount, Bar};

/// Which of the hub's tabs is showing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    /// Characters whose connection is open, or being retried.
    #[default]
    Live,
    /// Characters that ended this run, each with why.
    Closed,
    /// The roster's characters not on the table (`plan/49` Stage C).
    NotLaunched,
    /// A login by account, which lists its characters to add, star or
    /// play; and the kept passwords. Its own tab, apart from the cards
    /// (the author, 2026-09-27: *"not launched and ... login? are separate
    /// tabs"*).
    NewLogin,
}

/// What the player asked the hub for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HubAction {
    /// Something for the binary, the owner of the session table.
    Ask(HubRequest),
    /// Open this session's play window: the window's own business.
    Open(u32),
    /// Open the settings menu (`plan/50` §7 step 1).
    Settings,
    /// Open the trigger editor (`plan/54`).
    Triggers,
    /// Switch this session's own Lich on, or off (`;lich on`, `;lich off`).
    Lich(u32, bool),
    /// Open this session's log window (`plan/25` step 8).
    Log(u32),
}

/// What the hub shows this frame, gathered by the window from its sessions.
#[derive(Clone, Copy, Debug, Default)]
pub struct HubView<'a> {
    /// Every character on the table, in the order it was added.
    pub cards: &'a [SessionCard],
    /// Characters the hub can start: in the roster, with a saved password,
    /// not running.
    pub offered: &'a [String],
    /// Every character on the roster.
    pub roster: &'a [RosterCard],
    /// The last account whose characters the login service listed.
    pub listing: Option<&'a Listing>,
    /// The merged streams, oldest first.
    pub merged: &'a [MergedLine],
    /// What the binary answered the last request, if it has.
    pub said: Option<&'a str>,
    /// The sessions whose play window is open; a live card without one
    /// offers to open it.
    pub windowed: &'a [u32],
    /// The sessions whose own Lich runs (`plan/51`).
    pub lich: &'a [u32],
}

/// The hub's own state, which outlives a frame.
#[derive(Debug, Default)]
pub struct Hub {
    /// The tab showing.
    pub tab: Tab,
    /// Asking whether to shut Hydra down.
    confirming: bool,
    /// What the Not launched and New login tabs are typing, and the
    /// account logged in.
    launch: crate::launch::Launch,
    /// Every card's width, as the player last dragged it.
    pub card_width: CardWidth,
}

/// A card's width inside its frame: its four bars and the gaps between
/// them, until the player drags a side.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardWidth(pub f32);

impl CardWidth {
    /// Four bars of 72 (`Bar`'s own width) and the three gaps between them.
    pub const FOUR_BARS: Self = Self(4.0 * 72.0 + 3.0 * 8.0);
    /// The narrowest a card is dragged to: its bars wrap below the default.
    pub const NARROWEST: f32 = 160.0;
}

impl Default for CardWidth {
    fn default() -> Self {
        Self::FOUR_BARS
    }
}

/// What the shut-down question says: Despana's words.
pub const SHUT_DOWN_QUESTION: &str = "Shut Hydra down? Every character will quit.";

impl Hub {
    /// Draw the hub over `view`, and return what the player asked for, if
    /// anything.
    pub fn show(&mut self, ui: &mut egui::Ui, view: &HubView<'_>) -> Option<HubAction> {
        let mut asked = None;
        let (closed, live): (Vec<&SessionCard>, Vec<&SessionCard>) = view
            .cards
            .iter()
            .partition(|card| matches!(card.lifecycle, LifecycleView::Closed { .. }));
        let playing = !live.is_empty();
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::Live, format!("Live ({})", live.len()));
            ui.selectable_value(
                &mut self.tab,
                Tab::Closed,
                format!("Closed ({})", closed.len()),
            );
            ui.selectable_value(
                &mut self.tab,
                Tab::NotLaunched,
                format!("Not launched ({})", crate::launch::waiting(view).len()),
            );
            ui.selectable_value(&mut self.tab, Tab::NewLogin, "New login");
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // With nothing playing there is nothing to lose, so it does
                // not ask, as closing the window does not (author, 2026-09-27).
                let shut = ui.button("Shut down");
                if ui.button("Settings").clicked() {
                    asked = Some(HubAction::Settings);
                }
                if ui.button("Triggers").clicked() {
                    asked = Some(HubAction::Triggers);
                }
                if shut.clicked() {
                    if playing {
                        self.confirming = true;
                    } else {
                        asked = Some(HubAction::Ask(HubRequest::Shutdown));
                    }
                }
            });
        });
        if self.confirming {
            ui.horizontal(|ui| {
                ui.label(SHUT_DOWN_QUESTION);
                if ui.button("Shut down every character").clicked() {
                    self.confirming = false;
                    asked = Some(HubAction::Ask(HubRequest::Shutdown));
                }
                if ui.button("Keep playing").clicked() {
                    self.confirming = false;
                }
            });
        }
        if let Some(said) = view.said {
            ui.weak(said);
        }
        ui.separator();
        // Below the header, an id of its own: a line appearing above -- the
        // answer, the shut-down question -- would otherwise renumber every
        // widget under it, which a debug build draws as red boxes (`plan/49`
        // Stage C step 4).
        let body = egui::UiBuilder::new().id(ui.id().with("hub-body"));
        ui.scope_builder(body, |ui| {
            egui::Panel::bottom("hub-merged")
                .resizable(true)
                .default_size(160.0)
                .show(ui, |ui| merged(ui, view.merged));
            let width = &mut self.card_width;
            match self.tab {
                Tab::Live => list(
                    ui,
                    &live,
                    view,
                    "No character is running.",
                    width,
                    &mut asked,
                ),
                Tab::Closed => list(
                    ui,
                    &closed,
                    view,
                    "No character has closed this run.",
                    width,
                    &mut asked,
                ),
                Tab::NotLaunched => self.launch.show(ui, view, width, &mut asked),
                Tab::NewLogin => self.launch.show_login(ui, view, &mut asked),
            }
        });
        asked
    }

    /// Ask whether to shut Hydra down, as the Shut down button does: the
    /// window was asked to close while characters play.
    pub fn confirm_shutdown(&mut self) {
        self.confirming = true;
    }
}

/// The cards of one tab, tiled as many to a row as fit, or what an empty
/// one says.
fn list(
    ui: &mut egui::Ui,
    cards: &[&SessionCard],
    view: &HubView<'_>,
    none: &str,
    width: &mut CardWidth,
    asked: &mut Option<HubAction>,
) {
    if cards.is_empty() {
        ui.label(none);
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt("hub-cards")
        .auto_shrink(false)
        .show(ui, |ui| {
            tiled(ui, cards, *width, |ui, card| {
                // Each card keyed by its session, not its place, so a card
                // keeps its widgets' state (a button held, a side dragged)
                // as others come and go.
                let id = egui::Id::new(("hub-card", &card.session));
                let drawn = card_scope(ui, id, |ui| draw(ui, card, view, *width));
                if let Some(action) = drawn.inner {
                    *asked = Some(action);
                }
                side(ui, drawn.response.rect, id, width);
            });
        });
}

/// Lay `items` out as cards `width` wide, as many to a row as fit, each
/// row's cards aligned at their tops. Counted here rather than left to
/// egui's wrapping, which can only wrap what it knows the size of before it
/// is drawn.
pub(crate) fn tiled<T>(
    ui: &mut egui::Ui,
    items: &[T],
    width: CardWidth,
    mut card: impl FnMut(&mut egui::Ui, &T),
) {
    let outer = width.0 + egui::Frame::group(ui.style()).total_margin().sum().x;
    let gap = ui.spacing().item_spacing.x;
    let room = ui.available_width();
    let (mut per_row, mut used) = (1, outer);
    while used + gap + outer <= room {
        used += gap + outer;
        per_row += 1;
    }
    for row in items.chunks(per_row) {
        ui.horizontal_top(|ui| {
            for item in row {
                card(ui, item);
            }
        });
    }
}

/// A card's own area: an id of its own, laid out top to bottom.
pub(crate) fn card_scope<R>(
    ui: &mut egui::Ui,
    id: egui::Id,
    card: impl FnOnce(&mut egui::Ui) -> R,
) -> egui::InnerResponse<R> {
    let own = egui::UiBuilder::new()
        .id(id)
        .layout(egui::Layout::top_down(egui::Align::Min));
    ui.scope_builder(own, card)
}

/// A card's right side, which dragged sets every card's width.
pub(crate) fn side(ui: &egui::Ui, card: egui::Rect, id: egui::Id, width: &mut CardWidth) {
    let grip = egui::Rect::from_x_y_ranges(card.right() - 3.0..=card.right() + 3.0, card.y_range());
    let grip = ui
        .interact(grip, id.with("side"), egui::Sense::drag())
        .on_hover_cursor(egui::CursorIcon::ResizeHorizontal);
    grip.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Other, true, "Card width"));
    if grip.dragged() {
        let widest = ui.max_rect().width().max(CardWidth::NARROWEST);
        width.0 = (width.0 + grip.drag_delta().x).clamp(CardWidth::NARROWEST, widest);
    }
}

/// One card: who, how it is connected, its four gauges, a line of what else
/// a player glances at -- roundtime, room, group (`plan/29` §5a R3) -- and
/// what can be done with it.
fn draw(
    ui: &mut egui::Ui,
    card: &SessionCard,
    view: &HubView<'_>,
    width: CardWidth,
) -> Option<HubAction> {
    let mut asked = None;
    let number = card.session.parse::<u32>().ok();
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(width.0);
        ui.horizontal(|ui| {
            ui.strong(if card.name.is_empty() {
                "(unnamed)"
            } else {
                &card.name
            });
            ui.label(lifecycle(&card.lifecycle));
        });
        ui.horizontal_wrapped(|ui| {
            let vitals = &card.vitals;
            for (label, vital, color) in [
                ("HP", &vitals.health, bar::HEALTH),
                ("MP", &vitals.mana, bar::MANA),
                ("SP", &vitals.stamina, bar::STAMINA),
                ("Sp", &vitals.spirit, bar::SPIRIT),
            ] {
                ui.add(Bar::new(label, vital.as_ref().map(amount)).fill(color));
            }
        });
        let mut facts = Vec::new();
        if let Some(seconds) = card.roundtime.remaining_seconds.filter(|s| *s > 0) {
            facts.push(format!("RT {seconds}s"));
        }
        facts.push(
            card.room
                .clone()
                .unwrap_or_else(|| "Room unknown".to_owned()),
        );
        if let Some(group) = &card.group {
            facts.push(group_text(group));
        }
        ui.label(facts.join(" · "));
        let Some(number) = number else { return };
        ui.horizontal(|ui| {
            if ui
                .button("Log")
                .on_hover_text("What this character saw, read back")
                .clicked()
            {
                asked = Some(HubAction::Log(number));
            }
            if matches!(card.lifecycle, LifecycleView::Closed { .. }) {
                if ui.button("Reconnect").clicked() {
                    asked = Some(HubAction::Ask(HubRequest::Reconnect(number)));
                }
                if ui.button("Remove").clicked() {
                    asked = Some(HubAction::Ask(HubRequest::Remove(number)));
                }
                return;
            }
            if !view.windowed.contains(&number) && ui.button("Open window").clicked() {
                asked = Some(HubAction::Open(number));
            }
            if let Some(on) = crate::play::lich_switch(ui, view.lich.contains(&number)) {
                asked = Some(HubAction::Lich(number, on));
            }
            if ui.button("Quit").clicked() {
                asked = Some(HubAction::Ask(HubRequest::Remove(number)));
            }
        });
    });
    asked
}

/// The merged streams, each line once, tagged with who received it, the
/// newest at the bottom (`plan/29` step 5d).
fn merged(ui: &mut egui::Ui, lines: &[MergedLine]) {
    ui.strong("Thoughts, speech, logons, deaths and announcements");
    if lines.is_empty() {
        ui.weak("Nothing yet.");
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt("hub-merged-lines")
        .stick_to_bottom(true)
        .auto_shrink(false)
        .show(ui, |ui| {
            for line in lines {
                let mut runs = vec![cena_ui::StyledRun {
                    text: format!("[{}] ", line.from.join(", ")),
                    bold: true,
                    ..cena_ui::StyledRun::default()
                }];
                runs.extend(line.runs.iter().cloned());
                ui.label(crate::text::job(&runs, ui.style()));
            }
        });
}

/// A vital as a bar's value: `?`, with no fill at all, until the game has
/// said.
pub(crate) fn amount(vital: &VitalView) -> Amount {
    Amount {
        percent: vital.percent,
        current: vital.current,
        max: vital.max,
    }
}

/// How a character is connected, in the web hub's words (`app.js`'s
/// `lifecycleText`), so the two hubs -- and a play window -- say the same
/// thing.
pub(crate) fn lifecycle(lifecycle: &LifecycleView) -> String {
    let dashed = |detail: &Option<String>| {
        detail
            .as_ref()
            .map_or_else(String::new, |detail| format!(" — {detail}"))
    };
    match lifecycle {
        LifecycleView::Ready => "Ready".to_owned(),
        LifecycleView::Connecting => "Game connecting".to_owned(),
        LifecycleView::Closed { detail } => format!("Game closed{}", dashed(detail)),
        LifecycleView::Reconnecting {
            attempt,
            retry_delay_ms,
            detail,
        } => {
            let attempt = attempt
                .filter(|n| *n > 0)
                .map_or_else(String::new, |n| format!(" · attempt {n}"));
            let delay = retry_delay_ms.map_or_else(String::new, |ms| {
                let seconds = f64::from(u32::try_from(ms).unwrap_or(u32::MAX)) / 1000.0;
                format!(" · retry delay {seconds:.1}s")
            });
            format!("Game reconnecting{attempt}{delay}{}", dashed(detail))
        }
    }
}

/// Who a character leads or follows, in the web hub's words.
fn group_text(group: &GroupView) -> String {
    match &group.leader {
        None => format!("leading {}", group.members.join(", ")),
        Some(leader) => format!("following {leader}"),
    }
}
