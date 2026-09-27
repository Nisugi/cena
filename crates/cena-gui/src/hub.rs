//! The hub: every character this Hydra runs, at a glance, in two tabs
//! (`plan/47` §3). The author: *"we probably don't want to clutter the live
//! cards with the closed cards. so tab for closed and tab for live?"*
//!
//! A card is [`SessionCard`], the web hub's own, so both hubs show the same
//! thing from the same projection and nothing is copied (`plan/47` §4). This
//! draws; starting, quitting and reconnecting come with the session table
//! (`plan/47` §5, step 3).

use cena_ui::{GroupView, LifecycleView, SessionCard, VitalView};

/// Which of the hub's tabs is showing.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Tab {
    /// Characters whose connection is open, or being retried.
    #[default]
    Live,
    /// Characters that ended this run, each with why.
    Closed,
}

/// The hub's own state, which outlives a frame.
#[derive(Debug, Default)]
pub struct Hub {
    /// The tab showing.
    pub tab: Tab,
}

impl Hub {
    /// Draw the hub over `cards`, in the order the session table added them.
    pub fn show(&mut self, ui: &mut egui::Ui, cards: &[SessionCard]) {
        let (closed, live): (Vec<&SessionCard>, Vec<&SessionCard>) = cards
            .iter()
            .partition(|card| matches!(card.lifecycle, LifecycleView::Closed { .. }));
        ui.horizontal(|ui| {
            ui.selectable_value(&mut self.tab, Tab::Live, format!("Live ({})", live.len()));
            ui.selectable_value(
                &mut self.tab,
                Tab::Closed,
                format!("Closed ({})", closed.len()),
            );
        });
        ui.separator();
        let (shown, none) = match self.tab {
            Tab::Live => (live, "No character is running."),
            Tab::Closed => (closed, "No character has closed this run."),
        };
        if shown.is_empty() {
            ui.label(none);
            return;
        }
        egui::ScrollArea::vertical().show(ui, |ui| {
            for card in shown {
                draw(ui, card);
            }
        });
    }
}

/// One card: who, how it is connected, its four gauges, and a line of what
/// else a player glances at -- roundtime, room, group (`plan/29` §5a R3).
fn draw(ui: &mut egui::Ui, card: &SessionCard) {
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
            for (label, vital) in [
                ("HP", &vitals.health),
                ("MP", &vitals.mana),
                ("SP", &vitals.stamina),
                ("Sp", &vitals.spirit),
            ] {
                gauge(ui, label, vital.as_ref());
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
    });
}

/// A gauge's bar, labelled with its percent; `?`, with no fill at all, until
/// the game has said. egui draws a rounded cap even at 0%, which on an
/// unknown gauge would read as "a little".
fn gauge(ui: &mut egui::Ui, label: &str, vital: Option<&VitalView>) {
    let bar = match vital {
        Some(vital) => egui::ProgressBar::new(
            f32::from(u16::try_from(vital.percent.min(100)).unwrap_or(100)) / 100.0,
        )
        .text(format!("{label} {}%", vital.percent)),
        None => egui::ProgressBar::new(0.0)
            .fill(egui::Color32::TRANSPARENT)
            .text(format!("{label} ?")),
    };
    ui.add(bar.desired_width(72.0));
}

/// How a character is connected, in the web hub's words (`app.js`'s
/// `lifecycleText`), so the two hubs say the same thing.
fn lifecycle(lifecycle: &LifecycleView) -> String {
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
