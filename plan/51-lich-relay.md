# 51 — The Lich relay: real Lich, with Hydra holding the connection

**Status: SPIKED 2026-09-28, the questions in §6 open.** The spike
(`spike/lich-relay/`) ran real Lich against a recorded login, offline, and every hop worked.
Nothing here is built into Hydra yet.

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
- **The player's typing**: Hydra's own `;` commands first, then everything else to Lich's
  stdin, so Lich's `;` commands, aliases and upstream hooks see it (question 3).
- **The display** (question 1): for a character with Lich on, the play window's text comes
  from Lich's stdout, parsed by the same parser, so squelches and script output show as they
  would in any frontend. The panels (vitals, room, hands, compass) stay on Hydra's model.
- **A reconnect**: Lich stays up. Hydra's session re-logs in, and Lich sees the new login
  burst as more stream. Stock Lich restarts its process on a reconnect. Here its scripts
  would survive the drop (question 4).

## 6. Questions for the author

1. **The display.** Show Lich's stdout for a Lich character's text, so squelches work
   (recommended)? Or show Hydra's own stream plus Lich's own lines, told apart by the mark,
   so squelches don't apply in Hydra's window?
2. **Which frontend Hydra claims to be**: `stormfront`, which 331 script lines test for
   (recommended), or `saga`, for the mark? And may I draft the small Lich change (pipe mode
   keeping an explicit frontend) as a PR for you to look at? It would go to an
   elanthia-online repository, so nothing is pushed without your word.
3. **Command names both have**, such as `;sorter`: Hydra's first, with a prefix to reach Lich's
   (e.g. `;lich sorter`)? Or Lich's first while Lich is on?
4. **On a reconnect**: keep Lich running (recommended), or restart it the way stock Lich
   does?
5. **The M7b bridge** (`plan/46`, steps 0-10 built): keep it beside the relay, for scripts
   written against Hydra and for characters without Lich? Or stop it where it is?

## 7. Steps, once §6 is answered

1. The relay in the session: the port, Lich's process, the game's bytes tee'd to it, its
   commands through the gate as `Origin::Lich`, stopped with its stdin. Tested with a
   scripted game and the spike's Lich, offline.
2. Typing routed: Hydra's `;` commands, then Lich's stdin.
3. The display from Lich's stdout, per question 1.
4. Starting late: the login kept and replayed, then the latest room.
5. The switch: settings for Lich's path, and a *Lich* switch on the card and the play window.
6. A live run, with the author.
