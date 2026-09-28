//! The board the hub's tests draw from, and what it starts with.

use cena_gui::{Hub, HubAction, HubView};
use cena_ui::{
    GroupView, HubRequest, LifecycleView, Listing, MergedLine, RosterCard, RoundtimeView,
    SessionCard, StyledRun, VitalView, VitalsView,
};
use egui::accesskit::Role;
use egui_kittest::Harness;
use egui_kittest::kittest::Queryable as _;

/// What the window would gather, and what the hub asked for.
#[derive(Default)]
pub struct Board {
    pub hub: Hub,
    pub cards: Vec<SessionCard>,
    pub offered: Vec<String>,
    pub roster: Vec<RosterCard>,
    pub listing: Option<Listing>,
    pub merged: Vec<MergedLine>,
    pub said: Option<String>,
    pub windowed: Vec<u32>,
    pub asked: Vec<HubAction>,
}

impl Board {
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        let view = HubView {
            cards: &self.cards,
            offered: &self.offered,
            roster: &self.roster,
            listing: self.listing.as_ref(),
            merged: &self.merged,
            said: self.said.as_deref(),
            windowed: &self.windowed,
        };
        if let Some(action) = self.hub.show(ui, &view) {
            self.asked.push(action);
        }
    }
}

pub fn vital(percent: u32) -> VitalView {
    VitalView {
        percent,
        current: None,
        max: None,
    }
}

pub fn card(session: &str, name: &str, lifecycle: LifecycleView) -> SessionCard {
    SessionCard {
        session: session.to_owned(),
        name: name.to_owned(),
        lifecycle,
        vitals: VitalsView {
            health: None,
            mana: None,
            stamina: None,
            spirit: None,
        },
        roundtime: RoundtimeView {
            ends_at: None,
            remaining_seconds: None,
        },
        room: None,
        group: None,
    }
}

/// Two characters live -- one hunting, leading the other, which is
/// reconnecting -- and one closed.
pub fn cards() -> Vec<SessionCard> {
    let mut ashryn = card("0", "Ashryn", LifecycleView::Ready);
    ashryn.vitals = VitalsView {
        health: Some(vital(100)),
        mana: Some(vital(80)),
        stamina: Some(vital(60)),
        spirit: None,
    };
    ashryn.roundtime.remaining_seconds = Some(3);
    ashryn.room = Some("Rawknuckle's, Watering Hole".to_owned());
    ashryn.group = Some(GroupView {
        leader: None,
        members: vec!["Baelor".to_owned()],
    });
    let baelor = card(
        "1",
        "Baelor",
        LifecycleView::Reconnecting {
            attempt: Some(2),
            retry_delay_ms: Some(2_000),
            detail: None,
        },
    );
    let lorwyn = card(
        "2",
        "Lorwyn",
        LifecycleView::Closed {
            detail: Some("not logged in: [auth] bad password".to_owned()),
        },
    );
    vec![ashryn, baelor, lorwyn]
}

/// A thought both live characters heard.
pub fn merged() -> Vec<MergedLine> {
    vec![MergedLine {
        id: "0".to_owned(),
        stream: "thoughts".to_owned(),
        runs: vec![StyledRun {
            text: "[General] Maravel: anyone hunting?".to_owned(),
            ..StyledRun::default()
        }],
        from: vec!["Ashryn".to_owned(), "Baelor".to_owned()],
    }]
}

pub fn roster_card(character: &str, account: &str, game: &str, kept: bool) -> RosterCard {
    RosterCard {
        character: character.to_owned(),
        account: account.to_owned(),
        game: game.to_owned(),
        kept,
        favourite: false,
    }
}

/// Wyla's password not kept, on Shattered; Orsen's kept, so offered; Ashryn
/// starred, and playing.
pub fn roster() -> Vec<RosterCard> {
    let mut ashryn = roster_card("Ashryn", "ashryn01", "GS3", true);
    ashryn.favourite = true;
    vec![
        roster_card("Wyla", "wyla01", "GSF", false),
        roster_card("Orsen", "orsen01", "GS3", true),
        ashryn,
    ]
}

/// Ashryn's play window open; Baelor's closed, so Baelor runs headless.
pub fn board() -> Board {
    Board {
        cards: cards(),
        offered: vec!["Orsen".to_owned()],
        roster: roster(),
        merged: merged(),
        windowed: vec![0],
        ..Board::default()
    }
}

pub fn ask(request: HubRequest) -> HubAction {
    HubAction::Ask(request)
}

pub fn hub<'a>(board: Board) -> Harness<'a, Board> {
    Harness::builder()
        .with_size((560.0, 520.0))
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board)
}

/// The hub on its Not launched tab, tall enough for its logins and lists.
pub fn not_launched<'a>(board: Board) -> Harness<'a, Board> {
    let mut harness = Harness::builder()
        .with_size((720.0, 820.0))
        .build_ui_state(|ui, board: &mut Board| board.draw(ui), board);
    harness.get_by_label_contains("Not launched").click();
    harness.run();
    harness
}

/// Type `text` into the field `role` and `label` name, as a player does:
/// focused first.
pub fn type_into(harness: &mut Harness<'_, Board>, role: Role, label: &str, text: &str) {
    harness.get_by_role_and_label(role, label).focus();
    harness.run();
    harness.get_by_role_and_label(role, label).type_text(text);
    harness.run();
}
