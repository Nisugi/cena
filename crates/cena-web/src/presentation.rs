//! A single bounded presentation pump. Native snapshots are coalesced, never
//! rebuilt by applying frames to a second `GameState`. Native cursor fences are
//! separate from the presentation sequence emitted to viewers.

use crate::server::Shared;
use cena_session::{Event, Frame, Generation, ObservedEvent, SessionObserver, Snapshot, State};
use cena_ui::{
    LifecycleView, LineAssembler, ServerMessage, SessionView, StoryLine, StyledRun, WIRE_VERSION,
};
use std::collections::VecDeque;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::broadcast;

const MAX_HISTORY_LINES: usize = 256;
const MAX_HISTORY_BYTES: usize = 96 * 1024;
pub(crate) const MAX_WIRE_BYTES: usize = 512 * 1024;
const MAX_DRAIN: usize = 4096;

pub(crate) struct Hub {
    pub(crate) snapshot: Option<Arc<str>>,
    pub(crate) session: String,
    pub(crate) generation: String,
    pub(crate) updates: broadcast::Sender<Arc<str>>,
    sequence: u64,
    history: VecDeque<StoryLine>,
    history_bytes: usize,
    history_gap: bool,
}

impl Hub {
    pub(crate) fn new() -> Self {
        Self {
            snapshot: None,
            session: String::new(),
            generation: String::new(),
            updates: broadcast::channel(16).0,
            sequence: 0,
            history: VecDeque::new(),
            history_bytes: 0,
            history_gap: false,
        }
    }

    fn publish(&mut self, snapshot: &Snapshot, lines: Vec<StoryLine>, gap: bool) -> Result<(), ()> {
        self.session = snapshot.session.0.to_string();
        self.generation = snapshot.generation.0.to_string();
        self.sequence = self.sequence.checked_add(1).ok_or(())?;
        self.history_gap |= gap;
        for line in &lines {
            let bytes = line_bytes(line);
            self.history.push_back(line.clone());
            self.history_bytes += bytes;
            while self.history.len() > MAX_HISTORY_LINES || self.history_bytes > MAX_HISTORY_BYTES {
                if let Some(old) = self.history.pop_front() {
                    self.history_bytes -= line_bytes(&old);
                    self.history_gap = true;
                }
            }
        }
        let view = SessionView::project(
            &snapshot.state,
            lifecycle(snapshot),
            snapshot.state.game_time_now(),
        );
        let full = ServerMessage::Snapshot {
            version: WIRE_VERSION,
            session: self.session.clone(),
            generation: self.generation.clone(),
            cursor: self.sequence.to_string(),
            view: view.clone(),
            story: self.history.iter().cloned().collect(),
            history_gap: self.history_gap,
        };
        let full = encode(&full)?;
        let outgoing = if gap {
            Arc::clone(&full)
        } else {
            encode(&ServerMessage::Update {
                version: WIRE_VERSION,
                session: self.session.clone(),
                generation: self.generation.clone(),
                cursor: self.sequence.to_string(),
                view,
                lines,
            })?
        };
        self.snapshot = Some(full);
        let _ = self.updates.send(outgoing);
        Ok(())
    }
}

pub(crate) fn encode(message: &ServerMessage) -> Result<Arc<str>, ()> {
    // Stop serialization at the cap too: a giant native room must not first
    // allocate an unbounded JSON buffer only to be rejected afterwards.
    struct Bounded(Vec<u8>);
    impl std::io::Write for Bounded {
        fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
            if self.0.len().saturating_add(bytes.len()) > MAX_WIRE_BYTES {
                return Err(std::io::Error::other("presentation message limit"));
            }
            self.0.extend_from_slice(bytes);
            Ok(bytes.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut bounded = Bounded(Vec::new());
    serde_json::to_writer(&mut bounded, message).map_err(|_| ())?;
    String::from_utf8(bounded.0).map(Arc::from).map_err(|_| ())
}

fn lifecycle(snapshot: &Snapshot) -> LifecycleView {
    match snapshot.lifecycle {
        State::Ready => LifecycleView::Ready,
        State::Closed => LifecycleView::Closed { detail: None },
        State::Reconnecting => LifecycleView::Reconnecting {
            attempt: snapshot.retry.as_ref().map(|retry| retry.attempt),
            retry_delay_ms: snapshot
                .retry
                .as_ref()
                .map(|retry| u64::try_from(retry.delay.as_millis()).unwrap_or(u64::MAX)),
            detail: snapshot.retry.as_ref().map(|retry| retry.detail.clone()),
        },
        State::Connecting | State::Authenticating | State::Syncing => LifecycleView::Connecting,
    }
}

fn line_bytes(line: &StoryLine) -> usize {
    line.stream.len()
        + line
            .runs
            .iter()
            .map(|run| run.text.len() + run.preset.as_ref().map_or(0, String::len) + 64)
            .sum::<usize>()
}

struct Pending {
    assembler: LineAssembler,
    lines: VecDeque<StoryLine>,
    bytes: usize,
    cursor: u64,
    generation: Generation,
    gap: bool,
}

impl Pending {
    fn new(snapshot: &Snapshot) -> Self {
        Self {
            assembler: LineAssembler::default(),
            lines: VecDeque::new(),
            bytes: 0,
            cursor: snapshot.cursor,
            generation: snapshot.generation,
            gap: false,
        }
    }

    fn observe(&mut self, event: ObservedEvent) {
        if event.cursor <= self.cursor {
            return;
        }
        self.cursor = event.cursor;
        if event.generation != self.generation {
            self.assembler.reset();
            self.generation = event.generation;
        }
        if let Event::Frame(frame) = event.event {
            let lines = frame_lines(&mut self.assembler, &frame);
            for line in lines {
                self.bytes += line_bytes(&line);
                self.lines.push_back(line);
                while self.lines.len() > MAX_HISTORY_LINES || self.bytes > MAX_HISTORY_BYTES {
                    if let Some(old) = self.lines.pop_front() {
                        self.bytes -= line_bytes(&old);
                        self.gap = true;
                    }
                }
            }
        }
    }

    fn missing(&mut self) {
        self.gap = true;
        self.assembler.reset();
    }

    fn fence(&mut self, snapshot: &Snapshot, old: &mut broadcast::Receiver<ObservedEvent>) {
        // Events through this fence were published before subscribe answered.
        // Never await a missing event or drain beyond this fixed budget.
        for _ in 0..MAX_DRAIN {
            if self.cursor >= snapshot.cursor {
                break;
            }
            match old.try_recv() {
                Ok(event) if event.cursor <= snapshot.cursor => self.observe(event),
                _ => {
                    self.missing();
                    break;
                }
            }
        }
        if self.cursor < snapshot.cursor {
            self.missing();
        }
        if self.generation != snapshot.generation {
            self.assembler.reset();
        }
        self.cursor = snapshot.cursor;
        self.generation = snapshot.generation;
    }
}

fn frame_lines(assembler: &mut LineAssembler, frame: &Frame) -> Vec<StoryLine> {
    match frame {
        Frame::Text(text) => assembler.push(
            &text.stream,
            &StyledRun {
                text: text.content.clone(),
                bold: text.style.bold_depth > 0,
                monospace: text.style.mono,
                preset: text.style.preset.clone(),
            },
            text.ends_line,
        ),
        Frame::Prompt { .. } => assembler.flush(),
        Frame::Component { id, body } => {
            let mut lines = assembler.flush();
            let count = body.runs.len();
            for (index, run) in body.runs.iter().enumerate() {
                lines.extend(assembler.push(
                    id,
                    &StyledRun {
                        text: run.text.clone(),
                        bold: run.style.bold_depth > 0,
                        monospace: run.style.mono,
                        preset: run.style.preset.clone(),
                    },
                    index + 1 == count,
                ));
            }
            lines
        }
        Frame::ClearStream { id } => {
            assembler.clear_stream(id);
            Vec::new()
        }
        _ => Vec::new(),
    }
}

pub(crate) async fn pump(observer: SessionObserver, shared: Arc<Shared>) -> std::io::Result<()> {
    let result = tokio::select! {
        () = shared.stop.cancelled() => return Ok(()),
        result = observer.subscribe() => result,
    };
    let (initial, mut events) = result
        .map_err(|error| std::io::Error::other(format!("Session observation failed: {error:?}")))?;
    let mut pending = Pending::new(&initial);
    if shared
        .hub
        .lock()
        .await
        .publish(&initial, Vec::new(), false)
        .is_err()
    {
        return Err(std::io::Error::other(
            "Presentation message exceeds its size limit",
        ));
    }
    let mut terminal = initial.lifecycle == State::Closed;
    let mut dirty = false;
    let mut ticking = initial.state.in_roundtime() == Some(true);
    let mut interval = tokio::time::interval(Duration::from_millis(100));
    interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    loop {
        let refresh = tokio::select! {
            () = shared.stop.cancelled() => return Ok(()),
            event = events.recv(), if !terminal => match event {
                Ok(event) => {
                    let immediate = event.generation != pending.generation
                        || matches!(event.event, Event::StateChanged(_) | Event::ConnectFailed { .. });
                    pending.observe(event);
                    dirty = true;
                    immediate
                }
                Err(broadcast::error::RecvError::Lagged(_)) => { pending.missing(); true }
                Err(broadcast::error::RecvError::Closed) => { terminal = true; true }
            },
            _ = interval.tick() => dirty || ticking,
        };
        if !refresh {
            continue;
        }
        let result = tokio::select! {
            () = shared.stop.cancelled() => return Ok(()),
            result = observer.subscribe() => result,
        };
        // Stale state must never masquerade as an authoritative live view.
        let (snapshot, next) = result.map_err(|error| {
            std::io::Error::other(format!("Session observation failed: {error:?}"))
        })?;
        pending.fence(&snapshot, &mut events);
        events = next;
        let lines = pending.lines.drain(..).collect();
        pending.bytes = 0;
        let gap = std::mem::take(&mut pending.gap);
        if shared
            .hub
            .lock()
            .await
            .publish(&snapshot, lines, gap)
            .is_err()
        {
            return Err(std::io::Error::other(
                "Presentation message exceeds its size limit",
            ));
        }
        dirty = false;
        ticking = snapshot.state.in_roundtime() == Some(true);
        terminal = snapshot.lifecycle == State::Closed;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cena_session::{GameState, SessionId};

    fn snapshot(cursor: u64) -> Snapshot {
        Snapshot {
            session: SessionId::FIRST,
            generation: Generation::FIRST,
            cursor,
            state: GameState::default(),
            lifecycle: State::Ready,
            retry: None,
        }
    }

    #[test]
    fn subscription_and_cache_share_a_presentation_sequence() {
        let mut hub = Hub::new();
        hub.publish(&snapshot(99), Vec::new(), false).unwrap();
        let first: serde_json::Value =
            serde_json::from_str(hub.snapshot.as_ref().unwrap()).unwrap();
        assert_eq!(first["cursor"], "1");
        let mut events = hub.updates.subscribe();
        hub.publish(&snapshot(99), Vec::new(), false).unwrap();
        let next: serde_json::Value = serde_json::from_str(&events.try_recv().unwrap()).unwrap();
        assert_eq!(next["cursor"], "2");
        assert_eq!(next["kind"], "update");
    }

    #[test]
    fn reconnecting_before_retry_decision_keeps_unknown_attempt_and_delay() {
        let mut reconnecting = snapshot(1);
        reconnecting.lifecycle = State::Reconnecting;
        assert_eq!(
            lifecycle(&reconnecting),
            LifecycleView::Reconnecting {
                attempt: None,
                retry_delay_ms: None,
                detail: None,
            }
        );
        reconnecting.retry = Some(cena_session::RetryStatus {
            attempt: 2,
            delay: Duration::from_secs(5),
            detail: "fixture retry".into(),
        });
        assert_eq!(
            lifecycle(&reconnecting),
            LifecycleView::Reconnecting {
                attempt: Some(2),
                retry_delay_ms: Some(5000),
                detail: Some("fixture retry".into()),
            }
        );
    }

    #[test]
    fn missing_fence_clears_partial_lines_and_marks_snapshot_gap() {
        let mut pending = Pending::new(&snapshot(0));
        pending.assembler.push(
            "",
            &StyledRun {
                text: "incomplete".into(),
                ..StyledRun::default()
            },
            false,
        );
        let (_sender, mut events) = broadcast::channel(1);
        pending.fence(&snapshot(3), &mut events);
        assert!(pending.gap);
        assert!(pending.assembler.flush().is_empty());
        let mut hub = Hub::new();
        let mut changes = hub.updates.subscribe();
        hub.publish(&snapshot(3), Vec::new(), pending.gap).unwrap();
        let wire: serde_json::Value = serde_json::from_str(&changes.try_recv().unwrap()).unwrap();
        assert_eq!(wire["kind"], "snapshot");
        assert_eq!(wire["history_gap"], true);
    }

    #[test]
    fn fence_consumes_exactly_the_native_snapshot_prefix() {
        let mut pending = Pending::new(&snapshot(0));
        let (sender, mut events) = broadcast::channel(8);
        for cursor in 1..=3 {
            sender
                .send(ObservedEvent {
                    session: SessionId::FIRST,
                    generation: Generation::FIRST,
                    cursor,
                    event: Event::Frame(Box::new(Frame::Prompt {
                        time: cursor.to_string(),
                        text: ">".into(),
                    })),
                })
                .unwrap();
        }
        pending.fence(&snapshot(2), &mut events);
        assert_eq!(pending.cursor, 2);
        assert!(!pending.gap);
        assert_eq!(events.try_recv().unwrap().cursor, 3);
    }

    #[test]
    fn changed_generation_discards_unfinished_old_text() {
        let mut pending = Pending::new(&snapshot(0));
        pending.assembler.push(
            "",
            &StyledRun {
                text: "old".into(),
                ..StyledRun::default()
            },
            false,
        );
        pending.observe(ObservedEvent {
            session: SessionId::FIRST,
            generation: Generation(1),
            cursor: 1,
            event: Event::Frame(Box::new(Frame::Prompt {
                time: "1".into(),
                text: ">".into(),
            })),
        });
        assert!(pending.lines.is_empty());
    }

    #[test]
    fn serialization_refuses_oversized_presentation_messages() {
        let message = ServerMessage::Receipt {
            version: WIRE_VERSION,
            session: "0".into(),
            generation: "0".into(),
            request_id: "1".into(),
            status: cena_ui::ReceiptStatus::Uncertain,
            detail: "x".repeat(MAX_WIRE_BYTES),
        };
        assert!(encode(&message).is_err());
    }

    #[test]
    fn history_and_slow_viewers_are_bounded() {
        let mut hub = Hub::new();
        let mut slow = hub.updates.subscribe();
        for _ in 0..300 {
            hub.publish(
                &snapshot(0),
                vec![StoryLine {
                    stream: String::new(),
                    runs: vec![StyledRun {
                        text: "x".repeat(1024),
                        ..StyledRun::default()
                    }],
                    truncated: false,
                }],
                false,
            )
            .unwrap();
        }
        assert!(hub.history_bytes <= MAX_HISTORY_BYTES);
        assert!(hub.history.len() <= MAX_HISTORY_LINES);
        assert!(hub.history_gap);
        assert!(matches!(
            slow.try_recv(),
            Err(broadcast::error::TryRecvError::Lagged(_))
        ));
    }
}
