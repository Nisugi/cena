# 51 — The Lich relay: real Lich, with Hydra holding the connection

**Status: SPIKED 2026-09-28; §6's five questions ANSWERED the same day.** The spike
(`spike/lich-relay/`) ran real Lich against a recorded login, offline, and every hop worked.
Pipe mode's frontend fix was tried locally and works (§4, item 1). **§7's steps 1 and 2
BUILT 2026-09-28**: the relay itself, with the session's side
(`crates/cena-session/src/script/lich.rs`) and Lich's process (`crates/cena-agent/src/lich.rs`),
and the player's typing reaching Lich after Hydra's commands; tested against a stand-in and
against the real Lich, offline. Steps 3-6 are next.

## 1. The author's position

`plan/38` §2a set the relay aside on memory: 466 MB committed per character, measured on the
author's running Lich. The author, 2026-09-28, on that reason:

> *"Ok well memory seems like a dumb reason. Today for every character someone plays with lich
> they're running a full blown lich session + a frontend ... Most people will be 3-5 maybe 10
> max for majority in all honesty."*

and, the relay explained: *"I'm down for giving the relay a try."*

What the author asked about the shape, and the answers this plan builds on:

- **Lich runs per character, and only where it's wanted.** Lich keeps one character per
  process (`XMLData`, `GameObj` and `Char` are process-wide), so with five characters logged
  in and Lich wanted on one, one Lich runs.
- **Saga fakes nothing.** It does the account login and hands Lich the real key; Lich makes
  the game connection itself. The relay differs in that Hydra makes that connection and
  keeps it.

> **CORRECTED 2026-09-28.** Before the spike I told the author two things that its reading
> of Lich refuted. **First**, that Saga runs Lich in pipe mode. Pipe mode sets the frontend
> to `unknown` unconditionally (`reference/lich-5/lib/main/main.rb:569`), and a headless
> Lich resolves its frontend to `saga` (`reference/lich-5/lib/common/authentication/login_helpers.rb:594-596`),
> so Saga's Lich is INFERRED to run headless with Saga attached as a detachable client.
> Saga's own code is not here to confirm it. **Second**, that the origin sentinel `\x1f`
> marks Lich's own lines. It marks every line Lich **forwards from the game**
> (`reference/lich-5/lib/games.rb:1173`); Lich's own lines are the unmarked ones. Either way
> the mark separates the two, and neither correction changes the relay.

## 2. What the relay is

```text
 game ◄──► Hydra's session: parser, model, behaviors, the gate   (unchanged)
              │  the game's bytes, as received
              ▼
          127.0.0.1:<port>  ◄──  Lich --pipe -g 127.0.0.1:<port>
              ▲  Lich's commands          │ stdout: what Lich would show a frontend
              │                           ▼
          the gate (Origin::Lich)     that character's window
                                          ▲
          the player's typing ── stdin ───┘ (Hydra's own ; commands first)
```

Hydra starts the player's own Lich with `--pipe --no-gtk -g 127.0.0.1:<port>`, a port Hydra
holds. Lich reads a key and a version line from stdin and writes both to the socket
(`reference/lich-5/lib/main/main.rb:837-843`), then treats the socket as the game.
The key Hydra gives it is not a key. The game already has Hydra's, so **Lich never holds a
credential**, and pipe mode never reads Lich's login cache.

## 3. What the spike measured

`spike/lich-relay/src/main.rs`, excluded from the workspace like `spike/eaccess-spike`. It
binds a loopback port and starts `reference/lich-5` (236a9a2, the commit M7b vendored) in a
fresh home folder. It plays the game from `crates/cena-protocol/tests/fixtures/login_setup.xml`
and `login_burst_full.xml`, and checks each hop. Three runs, 2026-09-28, all green:

| Check | Result |
|---|---|
| Lich connects to Hydra's port | PASS, **3.25-3.88 s** after start |
| The key, then the version line, arrive first, unchanged | PASS |
| The recorded login reaches Lich's stdout | PASS, first room line **513 ms** after sending (a cold parser) |
| Lich's model from Hydra's bytes (`XMLData`, `GameObj`) | PASS: `name=Baelor game=GSIV room=7086`, hands, room title and exits |
| A script's `put 'look'` arrives on Hydra's socket | PASS, as `<c>look` |
| The player's typing, through Lich, arrives on Hydra's socket | PASS, as `exp`: **no `<c>`** |
| A downstream hook squelches a line from stdout | PASS |
| An upstream hook rewrites typing before Hydra sees it | PASS: `say hello` arrived as `say goodbye` |
| Lich stops when its stdin closes | PASS, exit 0 in **0.70 s**, the socket closed |
| Memory, settled, no map loaded | **98-106 MB** working set, **157-164 MB** committed |

To rerun: `cargo build` inside `spike/lich-relay`, then
`target/debug/lich-relay-spike <lich dir> <work dir> <effect-list.xml> <fixtures...> [--saga]`.

The memory figure does not replace `plan/38` §2a's 466 MB. That was a live session with the map
loaded, and the map alone is 100 MB. A Lich whose scripts touch `Room.current` will be nearer 466.

## 4. What the spike found that the design has to answer

1. **Pipe mode tells Lich its frontend is `unknown`** (`reference/lich-5/lib/main/main.rb:569`),
   overriding `--saga` or `--stormfront`. As `unknown`, a script's text goes out unescaped: a
   `respond '<b>not a tag</b> 5 < 6'` reached stdout as raw markup, which Hydra's parser
   would read as a tag. Set to `saga` after start, the same line came out
   `&lt;b&gt;not a tag&lt;/b&gt; 5 &lt; 6`, and game lines carried the mark. MEASURED.
   The fix belongs in Lich: pipe mode keeping an explicit frontend flag. Until then Hydra can
   set it after start, which shows `--- Lich: exec1 active.` lines on the character's screen.

   **Tried locally 2026-09-28** (the author: *"sure we can try it locally"*):
   `spike/lich-relay/lich-pipe-frontend.patch`, one helper beside `resolve_headless_frontend`
   that both pipe branches call, with three specs. It was committed in a scratch clone of
   236a9a2 with no remote. `login_helpers_spec.rb` passes 94 of 94 examples, and rubocop
   finds nothing in the three files. The spike with `--fe=stormfront` against both:

   | Lich | `$frontend` | `respond '<b>not a tag</b> 5 < 6'` reaches stdout as |
   |---|---|---|
   | 236a9a2 | `unknown` | `<b>not a tag</b> 5 < 6`: FAIL |
   | 236a9a2 + the patch | `stormfront` | `&lt;b&gt;not a tag&lt;/b&gt; 5 &lt; 6`: PASS |

   A PR to Lich is the author's to send. Players' own installs won't have the fix until Lich
   releases it, so Hydra keeps a fallback for an older Lich.

   **The fallback, as built (step 1):** not a `;e` after start, which shows on the screen, but
   Ruby's `trace_var` on `$frontend`, set before `lich.rbw` loads (`ruby -e <trace> -e '$0 =
   ARGV.shift; load $0' lich.rbw ...`). Whenever Lich sets `unknown`, it is put back to
   `stormfront`, and nothing is shown. On a patched Lich it never fires. VERIFIED on the real
   Lich at 236a9a2, unpatched: a script's `put "frontend #{$frontend}"` reached the game as
   `frontend stormfront` (`the_real_lich`, `crates/cena-agent/tests/lich_relay.rs`).
2. **Which frontend Hydra says it is.** Hydra speaks the protocol Lich registers as
   `stormfront`, alias `wrayth`. Scripts test the name: in `reference/scripts` and
   `reference/lich_repo_mirror`, `$frontend == 'stormfront'` appears 331 times and
   `$frontend == 'saga'` 19 times (MEASURED, `grep -hoE '\$frontend\s*(==|!=|=~)\s*[^ )]+'`).
   `stormfront` has `xml streams mono room_window`; `saga` has those plus `sentinel`
   (`reference/lich-5/lib/common/frontend/wrayth.rb`, `saga.rb`). Hydra needs the mark only if
   it shows the game's lines from its own stream (§5, the display), not Lich's.
3. **`<c>` tells a script's command from the player's typing.** Lich prefixes what scripts
   send (`$cmd_prefix`, `reference/lich-5/lib/main/main.rb:57`) and passes typing through as
   typed, after its upstream hooks. So Hydra can count typing as the player's attendance and
   a script's line as Lich's, without guessing.
4. **Lich changes the game's settings at login.** It sends `_flag Display Inventory Boxes 1`
   on every `<app char>` (`reference/lich-5/lib/common/xmlparser.rb:929-966`), and then hides
   the boxes from its own frontend with a hook (`reference/lich-5/lib/main/main.rb:809`).
   Through the relay that command reaches the game through Hydra's gate. Hydra decides
   whether to pass it.
5. **Lich reaches GitHub at every login.** A new install downloads its core scripts, and
   every login syncs the script repositories (`reference/lich-5/lib/games.rb:986-1009`). The
   first run did both, fetching 12 files from GitHub into the spike's scratch folder, before I
   gave the process a dead proxy (`https_proxy=http://127.0.0.1:9`). Nothing touched the game.
   On a player's own install, this is what their Lich already does.
6. **A Lich started late needs the login replayed.** Lich's model came entirely from the
   replayed bytes (VERIFIED, check 4). A Lich started mid-session gets the login Hydra kept,
   then the latest room, then the live stream. These are recorded bytes resent, never made-up
   tags (`plan/46` §10). Replaying the login also runs the player's autostart
   (`reference/lich-5/lib/games.rb:986`), which is what they'd expect.

## 5. The design proposed

- **Per character, off by default.** A character's card and its play window get a *Lich*
  switch. On, Hydra starts that character's Lich. Off, or the character closed, Hydra closes
  its stdin, and Lich exits in under a second.
- **The player's own Lich.** Their install, their Ruby, their scripts, `lich.db3` and map, at
  one path in Hydra's settings. Hydra ships no Lich.
- **The game's bytes go to Lich as received.** The copy is taken below the parser, where the
  session's bytes arrive (`cena-platform`), so Rule 2.1 holds: nothing above `cena-protocol`
  sees raw text. Hydra's model keeps coming from its own parse of the game.
- **Lich's commands go through the gate** as a new `Origin::Lich`. `<c>` lines are Lich's;
  lines without it are the player's typing, and count as attendance.
- **Lich is told it serves `stormfront`** (question 2): `--stormfront` with a Lich that has
  §4 item 1's fix, and `trace_var` on one that doesn't (§4 item 1, the fallback).
- **The player's typing** (question 3, and the author's rule below): Hydra's symbol first,
  as it is today (`crates/cena-session/src/command/claimant.rs`, `commands.symbol` in the
  character's settings), and so is a line a `Bare` behavior takes. A line with Lich's `;`
  is Lich's. With no Lich running it goes nowhere, and the player is told, as with an unknown
  Hydra command. The rest of what the player types goes to Lich while it runs, for its
  aliases and hooks, and to the game when it doesn't. Hydra's own lines (`.multi`, a relayed
  `.to`) reach Lich only with `;`; plain ones go straight to the game, since a Lich alias
  that expanded into a `.multi` of itself would never end. A player running Lich moves
  Hydra's symbol off `;`, to `.` say. Switching Lich on while both are `;` says so once.
- **The display** (question 1): for a character with Lich on, the play window's text comes
  from Lich's stdout, parsed by the same parser, so squelches and script output show as they
  would in any frontend. The panels (vitals, room, hands, compass) stay on Hydra's model.
- **A reconnect** (question 4): Lich stays up, and sees the new login as more stream. While
  the connection is down, its commands aren't sent, and the character's window says so. What
  survives is the Lich process: its settings, and scripts that only wait on lines. A script
  that was in the middle of something may stall, as it would anywhere, and the player kills
  it. Lich's autostart doesn't run again, since it runs once per process.

## 6. Questions for the author: ANSWERED 2026-09-28

1. **The display.** Show Lich's stdout for a Lich character's text, so squelches work
   (recommended)? Or show Hydra's own stream plus Lich's own lines, told apart by the mark,
   so squelches don't apply in Hydra's window?
   AUTHOR: *"yes"*. Lich's stdout.
2. **Which frontend Hydra claims to be**: `stormfront`, which 331 script lines test for
   (recommended), or `saga`, for the mark? And may I draft the small Lich change (pipe mode
   keeping an explicit frontend) as a PR for you to look at?
   AUTHOR: *"stormfront, sure we can try it locally"*. Tried: §4, item 1. No PR is sent.
3. **Command names both have**, such as `;sorter`: Hydra's first, with a prefix to reach Lich's
   (e.g. `;lich sorter`)? Or Lich's first while Lich is on?
   AUTHOR: *"if they're running lich would probably change hydra's command character to .
   or something"*. The symbol already is a setting (§5, the player's typing).
   AUTHOR, the same day, on where each line goes: *"commands typed in hydra go to the game.
   command starting with the lich command character ; get sent to lich. commands sent with
   the hydra command character . get sent to hydra."* Told that plain lines past Lich would
   miss its aliases and typing hooks: *"damn I guess commands have to go to lich then"*. So
   `.` is Hydra's, `;` is Lich's, and the rest of the typing goes to Lich too while it runs
   (§5).
4. **On a reconnect**: keep Lich running (recommended), or restart it the way stock Lich
   does?
   AUTHOR: *"we can keep lich running sure, but it's scripts aren't gonna magically keep
   working with no game connection."* This corrects §5 as first written, which said Lich's
   scripts would survive the drop. Lich survives; a script mid-action may not.
5. **The M7b bridge** (`plan/46`, steps 0-10 built): keep it beside the relay, for scripts
   written against Hydra and for characters without Lich? Or stop it where it is?
   AUTHOR: *"the relay is still going to be developed, I just think I was going about it the
   wrong way. Would be better to do some rewriting/updating of the scripts themselves instead
   of trying to support every one off."* Read as: both go on. The bridge grows by scripts
   updated to run on it, and its step 11 (the markup) is not pursued (`plan/46` §11).

## 7. Steps

1. The relay in the session: the port, Lich's process as `stormfront` (the flag, or set
   after start), the game's bytes tee'd to it, its commands through the gate as
   `Origin::Lich`, held while disconnected, stopped with its stdin. Tested with a scripted
   game and the spike's Lich, offline.

   **BUILT 2026-09-28.** Split where the crates already split scripts: the session gives a
   door, and `cena-agent`, which holds doors and never a handle, runs the process.
   - **The session's side**, `crates/cena-session/src/script/lich.rs`. `LichDoor` does
     three things. `wire` copies the game's bytes as they arrived, taken in `ingest` beside
     the recorder and read by nothing. It is kept in the session's publisher, so it lasts
     through a reconnect, and only one is allowed per character. `send` works as the script
     door's does, as `Origin::Lich` (new, `crates/cena-session/src/command/verdict.rs`) or as
     `Manual` for the player's typing. `say` tells the player something. A Lich with 4,096
     chunks unread is let go, not waited on, Lich's own rule for its own queue.
   - **Lich's process**, `crates/cena-agent/src/lich.rs`. `run` takes the copy at once, then
     starts Lich (`--pipe --stormfront -g 127.0.0.1:<port>`, and the `trace_var` for a Lich
     without the fix). It gives Lich a new token as its key, and takes only a connection that
     says it. It sends Lich's lines, `<c>` as Lich's and the rest as the player's. It writes
     typing to Lich's stdin, and tells the player when a line isn't sent and why. It stops
     Lich by closing its stdin, then kills it after 5 s. For now it reads Lich's stdout and
     lets it go (step 3 shows it).
   - **Tests.** In the session: one copy per character, and a Lich that falls behind is let
     go. In the agent: only a connection that says the key is taken. Against a stand-in in
     pipe mode (`crates/cena-agent/tests/fixtures/standin_lich.rb`, run in CI): the login
     reaches it, its `<c>look` is sent as Lich's, the pipe-mode `unknown` becomes
     `stormfront`, typing through it is sent as the player's, it stops when asked, and a
     second Lich for the character is refused. Against the real Lich
     (`the_real_lich`, ignored unless `HYDRA_TEST_LICH` names a checkout): 13.7 s, offline.
   - **Not yet:** the window's typing reaching it (step 2), its stdout shown (step 3), and a
     start after login (step 4). Until step 4, a Lich started mid-session sees only what
     comes next.
2. Typing routed: Hydra's symbol and `Bare` takers, then Lich's stdin.

   **BUILT 2026-09-28**, with the author's rule (§6, question 3), on the manual path
   (`crates/cena-session/src/command/round_trip.rs`). A line goes to Hydra's desk first.
   Then a line with Lich's `;` (`LICH_SYMBOL`) goes to Lich, or, with none running, nowhere,
   and the player is told. What the player typed at a frontend (`send_typed_at`, after the
   bridge's input hooks) goes to Lich while it runs. The rest goes to the game. A line given
   to Lich is answered `Outcome::Handled`, whose meaning widens to "Hydra took it": a new
   outcome would break five matches, one in the GUI file `gui-widgets` is changing. Past 64
   lines waiting (typed while Lich starts), a line is refused `Transient`. The door is now
   `LichDoor::attach`, giving the relay the byte copy and the typing together. Starting a
   Lich while Hydra's symbol is Lich's `;` tells the player once that Lich's commands can't
   be reached, and how to change it. The agent's contracts list `lich` among the `sent`
   origins.
   - **Tests.** In the session: `.go2 bank` runs on Hydra; `;e echo 1` and `gg` go to Lich,
     not the game; Hydra's own `;go2 bank` goes to Lich, and its `look` to the game. With
     Lich gone, `;e echo 2` goes nowhere and the player is told, and `exp` goes to the game.
     Moving the hand-off ahead of the desk fails it. The stand-in now runs a `;` line as a
     script and has one alias: `;put look around` reaches the game as Lich's, and `gg` as
     the player's `get gem`. The real Lich (11.4 s) types through `send_typed_at`.
3. The display from Lich's stdout.
4. Starting late: the login kept and replayed, then the latest room.
5. The switch: settings for Lich's path, and a *Lich* switch on the card and the play window.
6. A live run, with the author.
