//! The hub: every character this Hydra runs, at a glance, in two tabs
//! (`plan/47` §3). The author: *"we probably don't want to clutter the live
//! cards with the closed cards. so tab for closed and tab for live?"*
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

use cena_ui::{GroupView, HubRequest, LifecycleView, MergedLine, SessionCard, VitalView};

use crate::bar::{self, Amount, Bar};

/// Which of the hub's tabs is showing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    /// Characters whose connection is open, or being retried.
    #[default]
    Live,
    /// Characters that ended this run, each with why.
    Closed,
}

/// What the player asked the hub for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HubAction {
    /// Something for the binary, the owner of the session table.
    Ask(HubRequest),
    /// Open this session's play window: the window's own business.
    Open(u32),
}

/// What the hub shows this frame, gathered by the window from its sessions.
#[derive(Clone, Copy, Debug, Default)]
pub struct HubView<'a> {
    /// Every character on the table, in the order it was added.
    pub cards: &'a [SessionCard],
    /// Characters the hub can start: in the roster, with a saved password,
    /// not running.
    pub offered: &'a [String],
    /// The merged streams, oldest first.
    pub merged: &'a [MergedLine],
    /// What the binary answered the last request, if it has.
    pub said: Option<&'a str>,
    /// The sessions whose play window is open; a live card without one
    /// offers to open it.
    pub windowed: &'a [u32],
}

/// The hub's own state, which outlives a frame.
#[derive(Debug, Default)]
pub struct Hub {
    /// The tab showing.
    pub tab: Tab,
    /// Asking whether to shut Hydra down.
    confirming: bool,
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
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::Live, format!("Live ({})", live.len()));
            ui.selectable_value(
                &mut self.tab,
                Tab::Closed,
                format!("Closed ({})", closed.len()),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.button("Shut down").clicked() {
                    self.confirming = true;
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
        egui::Panel::bottom("hub-merged")
            .resizable(true)
            .default_size(160.0)
            .show(ui, |ui| merged(ui, view.merged));
        match self.tab {
            Tab::Live => {
                start(ui, view.offered, &mut asked);
                list(ui, &live, view, "No character is running.", &mut asked);
            }
            Tab::Closed => list(
                ui,
                &closed,
                view,
                "No character has closed this run.",
                &mut asked,
            ),
        }
        asked
    }

    /// Ask whether to shut Hydra down, as the Shut down button does: the
    /// window was asked to close while characters play.
    pub fn confirm_shutdown(&mut self) {
        self.confirming = true;
    }
}

/// A button for each character the hub can start.
fn start(ui: &mut egui::Ui, offered: &[String], asked: &mut Option<HubAction>) {
    if offered.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        for name in offered {
            if ui.button(format!("Start {name}")).clicked() {
                *asked = Some(HubAction::Ask(HubRequest::Add(name.clone())));
            }
        }
    });
    ui.separator();
}

/// The cards of one tab, or what an empty one says.
fn list(
    ui: &mut egui::Ui,
    cards: &[&SessionCard],
    view: &HubView<'_>,
    none: &str,
    asked: &mut Option<HubAction>,
) {
    if cards.is_empty() {
        ui.label(none);
        return;
    }
    egui::ScrollArea::vertical()
        .id_salt("hub-cards")
        .show(ui, |ui| {
            for card in cards {
                if let Some(action) = draw(ui, card, view.windowed) {
                    *asked = Some(action);
                }
            }
        });
}

/// One card: who, how it is connected, its four gauges, a line of what else
/// a player glances at -- roundtime, room, group (`plan/29` §5a R3) -- and
/// what can be done with it.
fn draw(ui: &mut egui::Ui, card: &SessionCard, windowed: &[u32]) -> Option<HubAction> {
    let mut asked = None;
    let number = card.session.parse::<u32>().ok();
    egui::Frame::group(ui.style()).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.strong(if card.name.is_empty() {
                "(unnamed)"
            } else {
                &card.name
            });
            ui.label(lifecycle(&card.lifecycle));
        });
        ui.horizontal(|ui| {
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
            if matches!(card.lifecycle, LifecycleView::Closed { .. }) {
                if ui.button("Reconnect").clicked() {
                    asked = Some(HubAction::Ask(HubRequest::Reconnect(number)));
                }
                if ui.button("Remove").clicked() {
                    asked = Some(HubAction::Ask(HubRequest::Remove(number)));
                }
                return;
            }
            if !windowed.contains(&number) && ui.button("Open window").clicked() {
                asked = Some(HubAction::Open(number));
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
