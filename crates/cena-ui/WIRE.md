# Hydra frontend contract, version 1

`wire.rs` is the Rust schema. `tests/fixtures/snapshot-v1.json` is a synthetic
cross-language contract fixture, not evidence of game wire behavior. Browser
tests consume these same bytes; Rust tests pin their serialize/deserialize
identity. The fixture deliberately includes unknown values, known emptiness,
negative health, HTML-looking plain text, and integers beyond JavaScript's exact
numeric range.

Every message has `kind` and `version: 1`. Session-bearing messages contain
`session` and `generation`, encoded as canonical decimal u64 strings. Snapshot
and update `cursor` values use the same encoding. They number presentation
updates, not game events. Never convert these fields to JavaScript Number.

`view.map_location` is an optional additive projection. Older servers omit it.
When supplied it contains `map_sha256` (64 lowercase hex characters: the SHA-256
of the exact native combined-map bytes) and `room` (a u32 map node ID or null).
This is **not** `view.room.id`, which remains the observed game UID. Null means
unresolved, including ambiguity. Only a Ready native snapshot supplies this
projection; reconnecting/closed/connecting and degraded views omit it. The
browser must also clear the marker when its viewer connection is lost, even if
it retains the last room description. This field grants no command authority.

| Kind | Other fields | Meaning |
| --- | --- | --- |
| `authenticate` | `token`, optional `session` | First client message; no state before authentication. `session` names the character this page is for; absent, the hub is served -- except by a server built for one session (`WebServer::bind`), which serves that session. |
| `command` | `session`, `generation`, `request_id`, `line` | One manual command; never automatically retried. |
| `snapshot` | `session`, `generation`, `cursor`, `view`, `story`, `history_gap` | Replace view and bounded history; `history_gap` makes missing history explicit. |
| `update` | `session`, `generation`, `cursor`, `view`, `lines` | Replace view and append complete lines. |
| `receipt` | `session`, `generation`, `request_id`, `status`, `detail` | Send outcome, attributable to the requested generation. |
| `sessions` | `sessions`, `available` | The hub page (`plan/29` step 5b): one card per character, replacing the last list whole, and the characters it may start. Sent to a viewer that named no session when there is not exactly one. The hub takes no game commands. |
| `add_character` | `character` | Hub only (step 5c): start a character that has logged in before -- in the roster, with a saved password. No credential crosses the socket. |
| `remove_session` | `session` | Hub only: quit a character and take it off the table. |
| `reconnect_session` | `session` | Hub only: log a stopped character back in, from the roster and the keyring. |
| `shutdown` | -- | Hub only: shut Hydra down in order, as Ctrl-C does: every character quits, the logs flush, the process exits. |
| `merged` | `lines` | Hub only (step 5d): thoughts, speech, logons, deaths and announcements across every character. Each line has decimal `id`, `stream`, styled `runs` and `from`, the characters that received it. Identical text on one stream from different characters within 1 second is one line; a line sent again with the same `id` has gained a character. |
| `hub_note` | `detail` | What became of the hub request just made, as one line of plain text. |

`status` is `sent`, `refused`, `uncertain`, or `handled` (a `;` command Hydra ran itself; nothing was sent). Sent establishes that command
bytes were written, not that the requested game action completed. An observed
reply can be described in `detail`. A missing or uncertain receipt never gives
permission to resend.

A `SessionCard` holds decimal `session`, `name` (empty when none was given),
`lifecycle`, `vitals`, `roundtime` -- each as in `SessionView` below -- and
nullable `room`, the room's title. A character's own page is the pairing link
with `&session=` its id appended.

`SessionView` holds `room`, `left_hand`, `right_hand`, `vitals`, `roundtime`,
`lifecycle`, nullable `prompt`, and bounded `unknown_tags`. `RoomView` has
nullable `id` and `title`, nullable styled-run `description`, nullable string
array `exits`, and nullable `creatures`, `objects`, `players` arrays. Room items
have string `id`, `noun`, `text`, and nullable string `status`. A null collection
means unobserved; an empty array means observed empty.

Hands are tagged `kind: unknown`, `empty`, or `holding`; holding adds nullable
`id`/`noun` and string `name`. Vitals have fixed `health`, `mana`, `stamina`,
`spirit` fields, each null or `{percent, current, max}`. Amounts are nullable
signed numbers and percent is the reported value. Roundtime has nullable
`ends_at` and `remaining_seconds`; an unknown clock gives an unknown remainder.
Projection accepts an explicit server time so replay does not depend on a local
clock. The listener supplies current extrapolated server time for live views.

Lifecycle is tagged `kind: connecting`, `ready`, `reconnecting`, or `closed`.
Reconnecting carries nullable `attempt`, nullable `retry_delay_ms`, nullable `detail`;
closed carries nullable `detail`. The retry delay is the original scheduled
backoff in milliseconds, not the time remaining until retry. Display it as a
scheduled delay, never as a countdown. This is supplied by native lifecycle
observation. When retry scheduling has not been observed, the attempt and delay
are null; an unknown attempt must not be displayed as attempt zero.

Story lines are `{stream, runs, truncated}`. The empty stream name is Story's
main channel. Each run is `{text, bold, monospace, preset}`; preset is nullable
and is only a token for a browser-owned allowlist. It must never become arbitrary
CSS or markup. All text, including unknown-tag diagnostics, is rendered through
text nodes. Clickable links and full markup fidelity are outside this slice.
Diagnostics are `{name, raw, truncated}`, capped at 32 entries, 128 name bytes
and 1024 raw bytes each; the native model retains its own full diagnostic ring.

Commands allow 1–4096 UTF-8 bytes with non-whitespace content, no CR/LF/NUL,
and no trimming of accepted text. Request IDs allow 1–64 ASCII letters, digits,
hyphens and underscores. Validation is pure; the listener additionally checks
version, authentication, duplicate IDs, and session identity. The native send
decision enforces current generation.

`LineAssembler::push` takes already projected runs. Frame boundaries do not
finish a line, streams retain independent partials, embedded newlines and the
explicit `ends_line` marker finish lines. Prompt handling calls `flush`; a
generation change or observation gap calls `reset`. Native clear-stream handling
calls `clear_stream`. Limits are 16 KiB text and 256 runs per unfinished line,
32 pending streams, and 128 bytes per stream/preset token. Overflow is marked
as `truncated`, and oversized stream keys are never joined by a shortened key.
The caller owns bounds for completed history and transport queues.

`;sorter` changes lines, never their shape. `push_naming` is `push` plus the
noun of the object a run names; with `sort_containers(true)`, a main-stream
container look that is a list end to end finishes as a header and one line per
category, each an ordinary story line. Every other line, and every line with
sorting off, finishes exactly as `push` would finish it.
