//! What the GUI's tests draw: Ashryn mid-hunt, as a snapshot and a story.

use std::sync::Arc;

use cena_session::{
    Amount as Numbers, ChunkLine, Event, Frame, GameState, Generation, Notice, NoticeKind,
    ObservedEvent, ProgressBar, RoomItem, SessionId, Snapshot, State,
};

use crate::story::Story;

fn bar(id: &str, percent: u32, current: i32, max: i32) -> Frame {
    Frame::ProgressBar(ProgressBar {
        id: id.to_owned(),
        dialog: Some("minivitals".to_owned()),
        percent,
        text: format!("{id} {current}/{max}"),
        amount: Some(Numbers { current, max }),
        time_remaining_secs: None,
        attrs: Vec::new(),
    })
}

fn item(noun: &str, text: &str) -> RoomItem {
    RoomItem {
        id: format!("-{}", noun.len()),
        noun: noun.to_owned(),
        text: text.to_owned(),
        before: None,
        after: None,
        status: None,
    }
}

/// Ashryn mid-hunt: hurt, holding a sword, a kobold and a player in the
/// room, in roundtime.
pub(crate) fn snapshot() -> Snapshot {
    let mut state = GameState::default();
    state.apply(&bar("health", 87, 348, 400));
    state.apply(&bar("mana", 40, 48, 120));
    state.apply(&Frame::LeftHand {
        item: "a steel broadsword".to_owned(),
        link: None,
    });
    state.apply(&Frame::RightHand {
        item: "Empty".to_owned(),
        link: None,
    });
    state.apply(&Frame::Prompt {
        time: "1000".to_owned(),
        text: ">".to_owned(),
    });
    state.roundtime_ends = Some(1_030);
    for id in ["room objs", "room players"] {
        state.apply(&Frame::Component {
            id: id.to_owned(),
            body: ChunkLine::plain("").runs,
        });
    }
    // As the model keeps it: the subtitle less its leading ` - `, with no
    // brackets (`cena_model::state::Room::title`).
    state.room.title = Some("Rawknuckle's, Watering Hole".to_owned());
    state.room.exits = Some(vec!["north".to_owned(), "out".to_owned()]);
    state.room.creatures = vec![item("kobold", "a kobold")];
    state.room.players = vec![item("Maravel", "Maravel")];
    Snapshot {
        session: SessionId::FIRST,
        generation: Generation::FIRST,
        cursor: 0,
        state,
        lifecycle: State::Ready,
        retry: None,
        stopped: None,
        triggers: Arc::default(),
    }
}

pub(crate) fn story() -> Story {
    let mut story = Story::default();
    let observed = |event| ObservedEvent {
        session: SessionId::FIRST,
        generation: Generation::FIRST,
        cursor: 1,
        event,
    };
    story.hear(
        &observed(Event::Line(Arc::new(cena_session::Line::new(
            "",
            ChunkLine::plain("You swing a steel broadsword at a kobold!").runs,
        )))),
        None,
    );
    story.hear(
        &observed(Event::Frame(Box::new(Frame::Prompt {
            time: "1000".to_owned(),
            text: ">".to_owned(),
        }))),
        None,
    );
    story.typed("look");
    story.tell(Notice::line(
        NoticeKind::Info,
        "Hunt: resting until mana is 50%.",
    ));
    story.hear(
        &observed(Event::Attention(Arc::new(
            cena_session::trigger::Attention {
                trigger: "kobold".to_owned(),
                sound: None,
                notify: None,
                alert: Some("A kobold is here!".to_owned()),
                cooldown: 0,
            },
        ))),
        None,
    );
    story
}
