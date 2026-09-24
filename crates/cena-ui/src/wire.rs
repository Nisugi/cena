//! Version 1 messages. Identity, generation and cursors are decimal strings.

use serde::{Deserialize, Serialize};

use crate::merge::MergedLine;
use crate::view::{SessionCard, SessionView, StoryLine};

/// The `version` every message in both directions must carry; anything else
/// is refused by the listener and rejected by the browser.
pub const WIRE_VERSION: u16 = 1;

/// Input does not expose lifecycle control or behavior authority.
///
/// Authentication and command validation are performed by the listener before
/// these values can reach a session. Unknown fields are rejected to expose
/// incompatible clients rather than silently ignoring their intent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClientMessage {
    /// `kind: "authenticate"`: must be the socket's first message; nothing is
    /// sent to the browser before it succeeds.
    Authenticate {
        /// Must equal `WIRE_VERSION`.
        version: u16,
        /// The pairing token, compared in full against the listener's secret.
        token: String,
        /// Which session this viewer is for, as a canonical decimal string --
        /// the page's own URL names it when several characters run
        /// (`plan/29` step 5). Absent: the only session, when there is one.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        session: Option<String>,
    },
    /// `kind: "command"`: one manual command line, never retried automatically.
    Command {
        /// Must equal `WIRE_VERSION`.
        version: u16,
        /// The target session id, as a canonical decimal u64 string.
        session: String,
        /// The connection generation the browser saw, as a canonical decimal
        /// u64 string; a stale generation is refused rather than sent.
        generation: String,
        /// Browser-chosen id (1-64 ASCII letters, digits, `-` or `_`) that the
        /// matching receipt echoes back.
        request_id: String,
        /// The command text, sent exactly as given (no trimming).
        line: String,
    },
    /// `kind: "add_character"`, from the hub page: start a character that has
    /// logged in before -- in the roster, with its password in the keyring
    /// (`plan/29` step 5c). No credential ever crosses this socket.
    AddCharacter {
        /// Must equal `WIRE_VERSION`.
        version: u16,
        /// The character, as the hub offered it in `available`.
        character: String,
    },
    /// `kind: "remove_session"`, from the hub page: quit a character and take
    /// it off the table.
    RemoveSession {
        /// Must equal `WIRE_VERSION`.
        version: u16,
        /// The session to remove, as a canonical decimal string.
        session: String,
    },
    /// `kind: "shutdown"`, from the hub page: shut Hydra down in order -- every
    /// character quits, the logs flush, the process exits -- as Ctrl-C does,
    /// for an operator who is not at the terminal (author, 2026-09-24).
    Shutdown {
        /// Must equal `WIRE_VERSION`.
        version: u16,
    },
    /// `kind: "reconnect_session"`, from the hub page: log a character that
    /// has stopped -- refused, idle, or given up to another client -- back in,
    /// from the roster and the keyring.
    ReconnectSession {
        /// Must equal `WIRE_VERSION`.
        version: u16,
        /// The session to reconnect, as a canonical decimal string.
        session: String,
    },
}

/// What the sender can establish. No variant asserts game action completion.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptStatus {
    /// Command bytes were sent; a reply may have been observed.
    Sent,
    /// The command was refused before sending.
    Refused,
    /// The send outcome cannot be established. Do not retry automatically.
    Uncertain,
    /// Hydra ran the line itself -- a `;` command -- and **nothing was sent
    /// to the game**.
    ///
    /// Not `Sent`: that claims bytes reached the wire, which for a claimed
    /// line is false, and every one of them used to come back saying so. Not
    /// `Refused` either: nothing was refused, the line did what it was for.
    /// A third fact, so a third word. The browser assets ship in the same
    /// binary as the server, so no viewer can be older than this variant.
    Handled,
}

/// A snapshot replaces the browser's view/history; updates append whole lines.
///
/// The listener owns monotonically increasing presentation cursors, independent
/// of native event numbering. All identity-sized integers use decimal strings
/// to survive JavaScript's narrower integer precision. Receipts echo the
/// requested generation so a stale refusal remains attributable to its input.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ServerMessage {
    /// `kind: "snapshot"`: the browser replaces its view and Story wholesale.
    Snapshot {
        /// Always `WIRE_VERSION`.
        version: u16,
        /// Session id, canonical decimal u64 string.
        session: String,
        /// Connection generation, canonical decimal u64 string; the browser
        /// ignores state older than the generation it holds.
        generation: String,
        /// Presentation cursor, canonical decimal u64 string; the browser drops
        /// any snapshot or update whose cursor is not above its last.
        cursor: String,
        /// The full current view.
        view: SessionView,
        /// The retained Story history, oldest first.
        story: Vec<StoryLine>,
        /// True when lines are missing from inside `story`; the browser shows
        /// a gap notice until that many lines have scrolled out.
        history_gap: bool,
    },
    /// `kind: "update"`: replaces the view and appends lines to the Story.
    Update {
        /// Always `WIRE_VERSION`.
        version: u16,
        /// Session id, canonical decimal u64 string.
        session: String,
        /// Connection generation, canonical decimal u64 string.
        generation: String,
        /// Presentation cursor, canonical decimal u64 string, above the last.
        cursor: String,
        /// The full current view, replacing the previous one.
        view: SessionView,
        /// Complete lines assembled since the previous message, to append.
        lines: Vec<StoryLine>,
    },
    /// `kind: "receipt"`: the send outcome for one `ClientMessage::Command`.
    Receipt {
        /// Always `WIRE_VERSION`.
        version: u16,
        /// Session id the command targeted, canonical decimal u64 string.
        session: String,
        /// The generation the command requested, echoed so a stale refusal
        /// stays attributable; the browser ignores receipts for another one.
        generation: String,
        /// The command's `request_id`, echoed so the browser can match it.
        request_id: String,
        /// What the sender can establish about the send.
        status: ReceiptStatus,
        /// Human-readable explanation, shown after the status label.
        detail: String,
    },
    /// `kind: "sessions"`: the hub page -- every character this Hydra runs, at
    /// a glance (`plan/29` step 5b). Sent to a viewer that named no session
    /// when there is not exactly one, and again whenever a card changes.
    Sessions {
        /// Always `WIRE_VERSION`.
        version: u16,
        /// One card per session, in the order they were added.
        sessions: Vec<SessionCard>,
        /// Characters the hub can add: in the roster, with a saved password,
        /// and not running. Empty when adding is not offered.
        available: Vec<String>,
    },
    /// `kind: "merged"`, to the hub page: thoughts, speech, logons, deaths and
    /// announcements across every character, each line once (`plan/29`
    /// step 5d). A line whose `id` was sent before is that line gaining a
    /// character.
    Merged {
        /// Always `WIRE_VERSION`.
        version: u16,
        /// New lines, and earlier ones gaining a character, in order.
        lines: Vec<MergedLine>,
    },
    /// `kind: "hub_note"`: what became of a hub request, for the page that
    /// made it.
    HubNote {
        /// Always `WIRE_VERSION`.
        version: u16,
        /// One line of plain text.
        detail: String,
    },
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_fixture_round_trips_without_losing_large_identifiers() {
        let fixture = include_str!("../tests/fixtures/snapshot-v1.json");
        let message: ServerMessage = serde_json::from_str(fixture).unwrap();
        let json = serde_json::to_value(&message).unwrap();
        assert_eq!(
            json,
            serde_json::from_str::<serde_json::Value>(fixture).unwrap()
        );
        assert_eq!(json["session"], "18446744073709551615");
        assert_eq!(json["view"]["vitals"]["mana"], serde_json::Value::Null);
    }

    #[test]
    fn unknown_input_fields_and_unversioned_input_are_refused() {
        for json in [
            r#"{"kind":"authenticate","version":1,"token":"x","extra":true}"#,
            r#"{"kind":"authenticate","token":"x"}"#,
            r#"{"kind":"delete_session","version":1}"#,
        ] {
            assert!(serde_json::from_str::<ClientMessage>(json).is_err());
        }
    }

    #[test]
    fn unobserved_retry_schedule_is_null_not_attempt_zero() {
        let lifecycle = crate::LifecycleView::Reconnecting {
            attempt: None,
            retry_delay_ms: None,
            detail: None,
        };
        assert_eq!(
            serde_json::to_value(lifecycle).unwrap(),
            serde_json::json!({
                "kind": "reconnecting",
                "attempt": null,
                "retry_delay_ms": null,
                "detail": null,
            })
        );
    }
}
