//! Pure model projection: the caller supplies lifecycle and observation time.

use cena_model::state::group::{Group, Leader};
use cena_model::{GameState, Hand, RoomItem, Vital};

use crate::view::{
    GroupView, HandView, LifecycleView, RoomItemView, RoomView, RoundtimeView, SessionView,
    StyledRun, UnknownTagView, VitalView, VitalsView,
};

impl SessionView {
    /// Project without reading a clock or changing game state.
    ///
    /// Live callers pass `state.game_time_now()`; replay callers pass the
    /// recorded server time. An unknown clock remains an unknown remainder.
    /// With a known clock, an unreported roundtime is over, matching the model's
    /// `roundtime_remaining` contract. Native invalidation clears the clock.
    #[must_use]
    pub fn project(state: &GameState, lifecycle: LifecycleView, server_now: Option<u32>) -> Self {
        let room = &state.room;
        let contents_known = room.component("room objs").is_some();
        Self {
            map_location: None,
            group: group(&state.group),
            room: RoomView {
                id: room.id.clone(),
                title: room.title.clone(),
                description: room.description.as_ref().map(|body| {
                    body.runs
                        .iter()
                        .map(|run| StyledRun {
                            text: run.text.clone(),
                            bold: run.style.bold_depth > 0,
                            monospace: run.style.mono,
                            preset: run.style.preset.clone(),
                            ..StyledRun::default()
                        })
                        .collect()
                }),
                exits: room.exits.clone(),
                creatures: contents_known.then(|| items(&room.creatures)),
                objects: contents_known.then(|| items(&room.objects)),
                players: room.saw_players().then(|| items(&room.players)),
            },
            left_hand: hand(&state.left_hand),
            right_hand: hand(&state.right_hand),
            vitals: VitalsView {
                health: state.health().map(vital),
                mana: state.mana().map(vital),
                stamina: state.stamina().map(vital),
                spirit: state.spirit().map(vital),
            },
            roundtime: RoundtimeView {
                ends_at: state.roundtime_ends,
                remaining_seconds: server_now.map(|now| {
                    state
                        .roundtime_ends
                        .map_or(0, |end| end.saturating_sub(now))
                }),
            },
            lifecycle,
            prompt: state.prompt.clone(),
            // The model owns the complete diagnostics. Repeated snapshots need
            // only a bounded sample; do not clone its entire raw-tag ring.
            unknown_tags: state
                .unknown_tags
                .iter()
                .take(32)
                .map(|tag| UnknownTagView {
                    name: bounded_text(&tag.name, 128).to_owned(),
                    raw: bounded_text(&tag.raw, 1024).to_owned(),
                    truncated: tag.name.len() > 128 || tag.raw.len() > 1024,
                })
                .collect(),
        }
    }
}

/// The game group, when there is one to show: leading members, or in
/// someone's. Alone, or unknown, there is none.
fn group(value: &Group) -> Option<GroupView> {
    let leader = match value.leader() {
        Leader::Unknown => return None,
        Leader::You if value.is_empty() => return None,
        Leader::You => None,
        Leader::Other(leader) => Some(leader.noun.clone()),
    };
    let members = value
        .members()
        .iter()
        .map(|member| member.noun.clone())
        .filter(|noun| leader.as_ref() != Some(noun))
        .collect();
    Some(GroupView { leader, members })
}

fn hand(value: &Hand) -> HandView {
    match value {
        Hand::Unknown => HandView::Unknown,
        Hand::Empty => HandView::Empty,
        Hand::Holding { id, noun, name } => HandView::Holding {
            id: id.clone(),
            noun: noun.clone(),
            name: name.clone(),
        },
    }
}

fn vital(value: Vital) -> VitalView {
    VitalView {
        percent: value.percent,
        current: value.current,
        max: value.max,
    }
}

fn items(values: &[RoomItem]) -> Vec<RoomItemView> {
    values
        .iter()
        .map(|item| RoomItemView {
            id: item.id.clone(),
            noun: item.noun.clone(),
            text: item.text.clone(),
            status: item.status.as_ref().map(ToString::to_string),
        })
        .collect()
}

pub(crate) fn bounded_text(text: &str, max_bytes: usize) -> &str {
    let mut end = text.len().min(max_bytes);
    while !text.is_char_boundary(end) {
        end -= 1;
    }
    &text[..end]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_group_is_who_leads_whom_and_nothing_alone() {
        use cena_model::state::group::{GroupEvent, Member};
        let member = |noun: &str| Member {
            id: format!("-10{}", noun.len()),
            noun: noun.to_owned(),
            text: noun.to_owned(),
        };
        let mut state = GameState::default();
        assert_eq!(group(&state.group), None, "nobody has said");
        state.group.apply(&GroupEvent::NotInGroup, None);
        assert_eq!(group(&state.group), None, "alone");
        state.group.apply(
            &GroupEvent::Listed {
                leading: true,
                members: vec![member("Kiyna")],
            },
            None,
        );
        assert_eq!(
            group(&state.group),
            Some(GroupView {
                leader: None,
                members: vec!["Kiyna".to_owned()]
            })
        );
        state
            .group
            .apply(&GroupEvent::JoinedGroup(member("Ashryn")), None);
        assert_eq!(
            group(&state.group),
            Some(GroupView {
                leader: Some("Ashryn".to_owned()),
                members: Vec::new()
            })
        );
    }

    #[test]
    fn unobserved_values_remain_unknown() {
        let view = SessionView::project(&GameState::default(), LifecycleView::Connecting, None);
        assert_eq!(view.room.exits, None);
        assert_eq!(view.room.description, None);
        assert_eq!(view.room.objects, None);
        assert_eq!(view.room.creatures, None);
        assert_eq!(view.room.players, None);
        assert_eq!(view.left_hand, HandView::Unknown);
        assert_eq!(view.vitals.health, None);
        assert_eq!(view.roundtime.remaining_seconds, None);
    }

    #[test]
    fn empty_hands_empty_compass_and_zero_vital_are_real_observations() {
        let mut state = GameState::default();
        state.left_hand = Hand::Empty;
        state.room.exits = Some(Vec::new());
        state
            .vitals
            .insert("mana".to_owned(), Vital::percent_only(0));
        state.vitals.insert(
            "health".to_owned(),
            Vital {
                percent: 0,
                current: Some(-5),
                max: Some(100),
            },
        );
        let view = SessionView::project(&state, LifecycleView::Ready, Some(100));
        assert_eq!(view.left_hand, HandView::Empty);
        assert_eq!(view.room.exits, Some(Vec::new()));
        assert_eq!(view.vitals.mana.unwrap().current, None);
        assert_eq!(view.vitals.health.unwrap().current, Some(-5));
        assert_eq!(view.roundtime.remaining_seconds, Some(0));
        assert_eq!(view.roundtime.ends_at, None);
    }

    #[test]
    fn explicit_time_makes_projection_deterministic_and_roundtime_saturates() {
        let mut state = GameState::default();
        state.roundtime_ends = Some(110);
        let a = SessionView::project(&state, LifecycleView::Ready, Some(103));
        let b = SessionView::project(&state, LifecycleView::Ready, Some(103));
        assert_eq!(a, b);
        assert_eq!(a.roundtime.remaining_seconds, Some(7));
        assert_eq!(
            SessionView::project(&state, LifecycleView::Ready, Some(111))
                .roundtime
                .remaining_seconds,
            Some(0)
        );
        assert_eq!(
            SessionView::project(&state, LifecycleView::Ready, None)
                .roundtime
                .remaining_seconds,
            None
        );
    }

    #[test]
    fn diagnostics_are_bounded_at_utf8_boundaries() {
        let mut state = GameState::default();
        state.unknown_tags = (0..40)
            .map(|_| cena_model::UnknownTag {
                name: "新".repeat(100),
                raw: "🦀".repeat(500),
            })
            .collect();
        let view = SessionView::project(&state, LifecycleView::Ready, None);
        assert_eq!(view.unknown_tags.len(), 32);
        assert!(
            view.unknown_tags
                .iter()
                .all(|tag| tag.truncated && tag.name.len() <= 128 && tag.raw.len() <= 1024)
        );
    }
}
