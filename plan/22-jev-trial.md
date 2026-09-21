# 22 — Trying Jev on the map: a brief for whoever sets it up

**Status: PROPOSAL (2026-09-21).** A working brief for an agent starting cold. It assumes
nothing from the conversation that produced it. Read `plan/21-mapdb.md` §3e and §3f for the
reasoning; this document is the *what, where and with what*.

## The question being answered

**Is Jev good enough at three map-classification jobs to be worth wiring in?** That is all.
The output of this work is a **scored report**, not a feature. Nothing here changes the map,
the converter, or Hydra. If the scores are good, wiring it in is a separate piece of work.

## What Jev is — UNVERIFIED until you make a call

From TypeSafe's announcement and the OpenRouter listing, read through a summarising fetcher,
so treat every line as a claim to check:

- A **decision model**, not a chat model. You give it text and a set of allowed answers; it
  returns one of them as a typed value **with a probability**. It generates no prose.
- Model id `typesafe/jev-1.13`. Endpoint `POST https://openrouter.ai/api/alpha/decisions` —
  **not** the chat-completions endpoint. Early access; the request shape may change.
- 32,000 tokens of context. At most 255 options in a choice field.
- $0.042 per million input tokens; output free. Claimed latency 70–500 ms.

Sources: <https://openrouter.ai/typesafe/jev-1.13>,
<https://typesafe.ai/blog/introducing-system-one-models-and-jev>.

**Your first task is to find the real request and response format** from OpenRouter's or
TypeSafe's documentation, make one call, and paste the exact request and response into the
report. Do not build anything on the description above.

## The three jobs, and the answer key for each

Each job is pick-from-a-list, and each has rooms where the right answer is **already known**.
Score Jev on those first. A job that scores badly is dropped — for pennies.

| # | job | choices | answer key |
|---|---|---|---|
| 1 | Which way does this exit go? | `up` / `down` / `same floor` | exits like `go stairs` or `climb ladder` whose **way back is an explicit `up` or `down`**. MEASURED 2026-09-21: **172** such exits (138 go up, 34 go down) out of 2,397 vertical-looking ones |
| 2 | Which side of the room is this exit drawn on? | the 8 compass sides / `none` | non-compass exits (`go door`, `climb trellis`) between two rooms that the **official layout** both places: the side is the direction from one position to the other |
| 3 | What kind of room is this? | a short legend list — **not yet written; propose one** (town street, shop, bank, inn, guild, wilds, cave, water, …) | rooms whose Lich `tags` already say (`bank`, `gemshop`, `inn`, `town`, …) |

**Job 1 is the one to do first**: smallest, cleanest key, and the one the author asked for.
Its prior is lopsided (80% go up), so **report accuracy against the "always say up" baseline**
— a model that scores 80% has learned nothing.

For every job send **both rooms** of the exit (title, every description variant, the exits
line) and the exit's command. Judging from both ends is the advantage of working from the
file rather than walking the game.

**Not a job:** `terrain` and `climate`. The author was explicit: rooms without them are set
up without them in the game. An empty field is the truth. Do not fill it, and do not use
either field as a label for indoors/outdoors.

## The data you have

All under `E:\Cena\reference\` — **gitignored; never commit anything from it**.

| what | where | notes |
|---|---|---|
| The Lich map database | `reference\mapdb\map-1789942730.json` | 43 MB, 36,838 rooms. Fields: `id`, `uid[]`, `title[]`, `description[]`, `paths[]`, `wayto{dest: command}`, `timeto`, `tags`, `location`. A `wayto` value starting `;e` is Ruby — skip those exits for this work |
| The same map, converted | run the two commands below | one JSON file per room, `rooms/<id/1000>/<id>.json`; the record is `cena_map::Room` (`crates/cena-map/src/room.rs`, `exit.rs`) |
| Official room positions | `reference\mapdb\map-data\prime\layouts.json` | OFFICIAL Simutronics data. `layouts` → 123 maps → `pos: [[uid, x, y], …]`. **Keyed by `uid`, not Lich `id`**; join through the room's `uid[]`. Covers 14,277 of Lich's rooms. No floors |
| Official rooms | `reference\mapdb\map-data\prime\rooms.json` | keyed by uid; at least a year old |

```powershell
cargo run --release -p cena-mapdb-convert -- "E:\Cena\reference\mapdb\map-1789942730.json" "E:\Cena\reference\mapdb\converted"
cargo run --release -p cena-map-combine  -- "E:\Cena\reference\mapdb\converted" "E:\Cena\reference\mapdb\converted\hydra.map"
```

Existing measurement scripts, as worked examples of reading the map in Python:
`research\mapdb-inventory\identify.py`, `chokepoints.py`, `route_key.py`.

**Both room numbers matter.** Lich's `id` names the room in the graph (`wayto` keys are ids).
The game's `uid` is what the official data uses. A room can have several uids or none.

## Where the work goes

**A Python script under `research\jev-trial\`.** Not a crate, not yet.

- This is a measurement. `plan/05` §−1: build the simplest thing that works, then stop. A
  Rust tool earns its place only if the scores say Jev is worth keeping.
- `crates/cena-map` is pure and may not do network or file I/O (enforced by an architecture
  test). Do not add an HTTP client to it or to the converter.
- If it *is* later built in Rust, it is a new offline tool beside `cena-mapdb-convert`, needs
  a row in `crates/cena-arch-tests/tests/layering.rs`, and uses the workspace's one TLS
  stack (native-tls; reqwest 0.12). That is not this task.

Suggested layout:

```
research/jev-trial/
  README.md          what was run, the exact commands, the results
  build_keys.py      map file -> answer-key JSONL for each job
  ask.py             one job's key -> Jev -> answers JSONL, cached
  score.py           answers vs key -> the report's tables
  results/           answers and scores (small; commit these)
```

**Cache every answer**, keyed by a hash of exactly what was sent. A re-run must cost nothing
and must not re-ask. Log the token count of every call so the report states the real cost.

## The API key

- Read it from the environment: `$env:OPENROUTER_API_KEY`. **Never** write it to a file, a
  log, a cached request, the report, or a commit. `CLAUDE.md`: never commit credentials.
- The author sets up the account and supplies the key. Do not look for one.
- Put `research/jev-trial/.env` and any raw request dumps in `.gitignore` before the first
  call, not after.

## What to hand back

`research\jev-trial\README.md`, with, for each job attempted:

1. The exact request and response for one call, key redacted.
2. Key size, accuracy, and the **baseline** accuracy (always answering the commonest class).
3. **Accuracy by stated probability** — a table of probability bands against how often Jev
   was right. This decides the threshold above which a proposal can be trusted and below
   which it goes to the author for review. It is the most useful number in the report.
4. Twenty of its mistakes, with the room text, so a person can judge whether the *key* or
   the *model* was wrong. Lich's text is stale for ~5% of rooms; some "mistakes" will be the
   map's.
5. Tokens used and what it cost.
6. A recommendation: use it, use it above probability *p*, or drop it.

Label every claim VERIFIED, INFERRED or UNVERIFIED, and give the command that produced every
number (`plan/05` §−2). Do not restate a number from memory.

## Rules of the repository

- **Do not change branches. Do not push** — not to any remote, for any reason, unless the
  author asks in that conversation. Commit only if asked, and only your own files.
- Other agents are working in this repository at the same time. Touch nothing outside
  `research/jev-trial/` and `.gitignore`.
- **Never run the `cena` binary** and never log into the game. This work needs neither.
- Ask before searching `E:\Gemstone\data\log archive` (49.55 GB). This work does not need it.
- The author runs **PowerShell**; give them PowerShell commands.
- On this machine, Bash heredocs mangle backslashes. Write files with the editor's write
  tool, not `cat <<EOF`.
- The product is **Hydra**; `cena` is the working name of the repository and its crates.

## What is deliberately not decided here

- **Where proposals will land.** `plan/21` §3f calls it the *authored-corrections file*: a
  hand-editable file that survives regenerating the map, with each entry marked by its
  source (`llm`) so a person can review a diff. Its format does not exist yet. **Do not
  invent it as part of this work** — a trial that writes nowhere is the point. Say in the
  report what shape of record the results suggest.
- **The legend for job 3.** Propose a list; the author decides.
- **Floors beyond job 1.** Rule-based passes (explicit `up`/`down`, then paired returns)
  come first and Jev sees only what they leave open. Those passes are not built either.
