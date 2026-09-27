# 45 — Milestone 8: triggers

> **STATUS: PLAN, 2026-09-26; STAGES 1 AND 2 BUILT the same day, STAGES 4, 3, 5 AND 6 on
> 2026-09-27** (§6): every stage built, all but Stage 1's release run of the bench, and
> nothing yet run live. Branch `m8-triggers`, cut from `m6-hunt` at `e872f0b` and
> `m6-hunt` merged in again at `b7012d3`, in
> the worktree `G:\dev\Cena-m8` (locked on purpose: M8 spans sessions). The author's
> decisions are quoted in §1 with the date. **Everything else here is Claude's proposal**,
> and two of §1's rows are Claude's *reading* of an answer, marked as such. What was decided
> while building, for the author to confirm, is §5c, §5d, §6a, §6b, §6c, §6d and §6e.
> `plan/12` wins any contradiction, except where §1 records the author changing it.

---

## 0. What M8 is now

`plan/12` §8 gave M8 one line: *"remaining behaviors; customization surface (per `11` --
highlights first, §6a.4)"* (`plan/12-implementation-spec.md:717`). Most of the behaviors
landed in M6. The author redefined the milestone on 2026-09-26:

> *"Ok we will do M8 first. Trigger system (not just highlights!)"*

It came up because the author asked to start the egui GUI (`plan/28`) and the GUI plan
assumed M8 would land first: **highlights apply once, in core, before any frontend**
(`plan/28` §3d, VellumFE's best rule). A GUI built before M8 would have had to colour text
itself. So M8 is first, and the GUI's open questions (asked the same day) are parked until
it is next.

**M9 is empty.** It was the DragonRealms adapter, and `plan/12` §9d took it off the roadmap.

**The name of the thing: highlighting is one response among many.** `plan/28` §7h already
said so of VellumFE (*"a `Trigger -> Effects` system wearing the wrong name"*). A trigger
here is **when** something happens, **do** something, for **whom**.

---

## 1. Decisions (author, 2026-09-26)

| # | Asked | Answer | What it means here |
|---|---|---|---|
| 1 | Can a trigger act (send a line)? (a) not in M8, (b) from the player's own rules only, never from a shared pack, (c) anywhere, packs gated | *"b is fine."* | Act is allowed, from the player's own rules. A rule that came from someone else -- an import, a file shared by another player -- arrives with its act **held** (the importer's word, `plan/30`) until the player approves it. Stage 5. |
| 2 | Do conditions reuse the hunt's guard words (`plan/33`)? | *"yes"* | One vocabulary, no second DSL. The guard evaluators move **down** from `cena-behavior` to `cena-model`, where the session can reach them. Stage 2. |
| 3 | Several packs per character, or one profile like hunts? | *"One file, players won't be accessing the file most of the time, we will have a gui editor which can give a category, they can be sorted by category in the file."* | One triggers file. No packs, no profile level. A **category** on each trigger, set by the (future) GUI editor; the file is written sorted by category. **CLAUDE'S READING, TO CONFIRM:** per-character rules live *inside* the one file (§5b), which keeps `plan/12` §6a.2's global -> character override for triggers without a profile level. |
| 4 | Import Wrayth first, with the author's exports as fixtures? What does `case="y"` mean? | *"yes, I assume case="y" means case sensitive, but I'm not positive."* | Wrayth XML is the first importer (Stage 4). `case="y"` is read as case-**sensitive**: the author's assumption, **UNVERIFIED** (§2d gives the evidence both ways). |
| 5 | Sound and OS notifications in M8, or colour/text/routing first? | *"we can hold on sounds."* | Stage 3 (attention) is **held**. **CLAUDE'S READING:** OS notifications were in the same question and are held with sound. The status flag is not attention and stays in Stage 2. **RELEASED 2026-09-27** (author): *"You can do all of stage 3. We already solved sound in vellum fairly sure."* |

**Two binding rules this touches, recorded so they are not re-derived:**

- **`plan/12` §6a.2** says every setting resolves global -> profile -> character. For
  triggers the author has chosen **one file with no profile level** (row 3). The global ->
  character override survives inside the file (§5b). This is the author changing a binding
  rule for one setting, not drift.
- **`plan/12` §6a.3** says every invocation surface resolves to an `Action` through the
  command authority. A trigger's act (Stage 5) is such a surface. No `Action` registry
  exists yet (`grep -rn "enum Action" crates/` finds only `cena-map`'s unrelated one), so
  Stage 5 starts with the simplest action there is: **a line, as if typed**.

---

## 2. What the evidence says

### 2a. VellumFE's engine -- measured 2026-09-26

`reference/VellumFE` at `c1f7953`. **Its history is squashed to one commit**, so there is no
churn record to read; what follows is from the code.

- **One flat rule, 22 fields** (not 26, as `plan/28` §7h says): pattern, scope, colour,
  rewrite, squelch, redirect, sound, rumble, status, and a 15-field alert, all in
  `HighlightPattern` (`reference/VellumFE/src/config/highlights.rs:157-211`).
- **The same rules are compiled three times, with three semantics.** Highlighting checks
  word boundaries (`src/core/highlight_engine.rs:429-443`); squelch and redirect compile
  their own matchers and match substrings (`src/core/messages/routing.rs:126-250`), and
  redirect's literal automaton ignores `case_insensitive` (`:239-242`).
- **Overlap resolution is order-dependent.** Earliest start wins, length ignored, no
  priority field (`highlight_engine.rs:633-671`), and regex matches are gathered in
  `HashMap::values()` order, which can differ run to run. Sounds from matches the overlap
  rule dropped **still fire** (`:453-466`).
- **Squelch runs first** (`src/core/messages/flush_line.rs:393-406`), so a squelched line can
  never sound an alert. And squelch is **not** trust-gated in alert packs, though it hides
  game output as surely as `replace` does (`src/config/alertpacks.rs:148-199` gates only
  `replace` and `redirect_to`).
- **A text rule cannot feed a condition.** `set_status` writes the dashboard
  (`src/core/app_core/state/custom_status.rs:63-91`); `Condition::Indicator` reads
  `gs.status` (`src/core/conditions.rs:393-395`). And a pattern rule's `when` gate is never
  consulted -- the editor says so (`src/frontend/gui/app/editors/highlights.rs:888-892`).
- **A character rule replaces the global one whole**, no field merge
  (`src/config/highlights.rs:551-565`).
- **No VellumFE trigger sends a game command.**
- **Cost**: 505 rules (452 literal) took full parse+process from 31,532 to 59,226 lines/s
  when the engine was optimised (`tests/bench_parse.rs:18-20`) -- about 17 µs a line,
  INFERRED from that figure.
- **Worth keeping**: edge-triggered conditions (first evaluation silent, false -> true only,
  re-arm after staying false; `src/core/app_core/state/alerts.rs:106-123`); one alert per
  rule per line, cooldowns and a cap of 5 (`src/core/alerts.rs:197-237`); case-insensitive
  literals as a **second automaton**, not a flag; the staleness hash over every field the
  engine reads; the content-hash trust gate, and its rationale (`alertpacks.rs:18-27`):
  *"the gate is about authorship, not about the capability itself."*

### 2b. Players -- `research/11`

- **Highlights: 54 people** (`research/11-player-requirements.md:428`). Veterans carry
  1,500+ entries and 25 years of them.
- **Import: 23 people** (`:1209`); Wrayth is the client named most.
- **Global vs per-character: 11 people** (`:432`).
- **The gap Saga leaves: sound and alerts bind only to text** -- 8 people (`:1357`), and
  horibu's *"Saga native sound alerts can only trigger on txt highlights"* (`:372`).
  Notifications, 10 people, with the sharpest ask a *predicate over parsed state* (`:704`).
  Players work round it with a Lich script that prints a line for a highlight to catch.
  **Event and condition triggers are the answer to this, not a nicety.**
- **Routing is not filtering** (R-A2, `:313`); routing combat text to its own window is
  *"the largest UI demand"*, 28 people (`:1042`).
- **"When X, send Y": about 2 people in ten weeks** (`:689`, `:1386`). That figure is about
  reactive *commands* only. It is not evidence against a trigger system; the asks above are
  all trigger-shaped.

### 2c. Lich and the scripts

- `DownstreamHook` sees **raw XML** and returns the line, or `nil` to squelch it;
  `UpstreamHook` the same over typed commands; `Watchfor` matches stripped lines and runs a
  block in a thread (`reference/lich-5/lib/common/downstreamhook.rb`, `watchfor.rb`).
  Census: `DownstreamHook` in 53 files, `UpstreamHook` 37, `Watchfor` 1
  (`inventory/07-script-corpus-api-census.md`).
- Scripts that are triggers in all but name, `reference/scripts/scripts/`: `soundfx`
  (sound), `notify` (toast), `isquelch` (squelch, and imports Wrayth ignores),
  `deepthoughts` (redirect), `commas` and `milkman` (substitute), `briefcombat`
  (condense). **Two send a command**: `autoreact` (`weapon <x>` on a reaction offer) and
  `duskruin_watch` (a dodge verb on an arena line).

### 2d. Wrayth's format

The author's exports are at `E:\misc\Saved Files From Latest Reinstall Yay\Gemstone\SIMU\Wrayth\`
(`Nisugi3.xml`, `NewLayoutWrayth.xml`, `Mnstr.xml`, `YepCock.xml`).

| Section | Element | Nisugi3.xml |
|---|---|---|
| `<strings>` | `<h text color bgcolor [line="y"] [case="y"] [sound="…wav"]>` | 108 (58 whole-line, 2 case, 11 sound) |
| `<names>` | `<h text color bgcolor [case]>` | 112 (19 case) |
| `<ignores disable='n'>` | `<h text>`; `disable` is the section's master switch | 2 |
| `<palette>` | `<i id="0..109" color="#hex">`; a colour `@N` is an index into it | 110 |
| `<presets>` | `<p id color bgcolor …>`: roomName, bold, speech, whisper, thought, … | 9 |

No substitute, trigger or gag elements exist in any of the four files.

**`case="y"`, the evidence both ways:** VellumFE's importer reads it as case-sensitive
(`reference/VellumFE/src/config/wrayth_import.rs:36-38`); Wizard FE's docs have a "Case
Sensitive" checkbox (`reference/wiki_clean/Wizard _front end_.txt:575`); 19 of 112 *names*
carry it, and a case-sensitive name is plausible (a proper noun). Against: stormsync maps it
to `ignore_case` (`reference/lich_repo_mirror/lib/stormsync.lic:822`). Taken as sensitive,
per the author. A live check in Wrayth would settle it.

---

## 3. The model: when, do, for whom

### 3a. When

Three sources and `only_if`:

1. **Text** -- a literal or regex over a finished line, optionally limited to streams, with
   capture groups. Literal is the default, because every one of the author's Wrayth strings
   is a literal.
2. **Event** -- a typed fact a classifier already produced: an incident
   (`crates/cena-model/src/state/incident.rs`), an affliction, a combat outcome (by actor:
   me, my group, anyone), speech by channel and speaker, a death, a departure, the idle
   warning, a failed move. A **closed vocabulary Hydra defines**, as guards are. This is
   `plan/12` §6a.4's *"highlight by actor and event class"*, and §2b's gap.
3. **Condition** -- a guard word (`plan/33`) becoming true: HP below a number, stunned, a
   spell about to expire. **Edge-triggered** on VellumFE's rules (§2a).

**`only_if`** -- one or more guard words -- may sit on any trigger, and is checked. This
said *gate* until Stage 2 was built; the glossary gives that word to the session's check at
the moment it writes (`Gate`), and lists *guard* as not it, so a trigger's is `only_if`.

### 3b. Do

| Group | Responses | Stage |
|---|---|---|
| **Look** | colour, background, bold; on the match, a capture group, or the whole line | 1 |
| **Text** | squelch; substitute (with `$1`); redirect to another stream, moving or copying | 1 |
| **Flag** | set or clear a named status flag, optionally for N seconds; guards can read it, so a text trigger can feed a condition | 2 |
| **Attention** | sound, OS notification, alert banner | 3, **held** |
| **Act** | send a line, as if typed | 5 |

### 3c. For whom

Everyone, or the characters a trigger names (§5b).

### 3d. The words

`plan/05` §8 makes the glossary binding, and it forces two choices:

- **Trigger** is new. The glossary lists "trigger" in `Event`'s *Not* column
  (`crates/cena/src/glossary.rs:115`); that stays true -- an event is not a trigger.
- **What a trigger does cannot be called an "effect"**: `Effect`/`Effects` are the model's
  spell and buff effects (`crates/cena-model/src/effects.rs:102`). **Proposed: response.**
  The author may prefer another word; it is added to the glossary with the first code that
  uses it.

---

## 4. Where it runs: once, in the session, before any viewer

**The hook already exists.** When `route_text` completes a line, `lines_seen` moves, and the
actor already reads the model's line there for the player log -- *"the model's line, not a
second assembly of it"* (`crates/cena-session/src/actor/io.rs:671`). The trigger engine
runs at the same point, once per finished line, and the session publishes the line **with
its responses**.

The rules that follow from that:

- **The model stays the record of what the game said.** Scrollback
  (`crates/cena-model/src/state/streams.rs:162`), the chunk the classifiers read, and the
  player log all keep the game's text unchanged. Substitute, squelch and redirect are
  **display facts carried with the line**; they never change a game fact.
- **One compiled matcher** serves every response kind: one `aho-corasick` automaton for
  case-sensitive literals, a second for case-insensitive ones, one `RegexSet` for regexes.
  VellumFE's three compilations (§2a) are the thing not to repeat.
- **Deterministic order**: priority, then file order (category, then name). Only look
  responses take part in overlap resolution; every other response fires if its trigger
  matched. **Attention fires on the original line even when it is squelched**, so "hide this
  but tell me" is expressible.
- **Headless is first-class.** Everything except rendering happens in the session, so a
  session nobody watches still sets flags and (Stage 5) acts.
- **Multi-session**: each session evaluates its own lines against the rules that apply to
  its character. A thought several characters receive raises attention once (the merged
  streams' one-second rule, `crates/cena-ui/src/merge.rs`) -- recorded for Stage 3.

### 4a. One line assembly -- Stage 1's first step

Today there are **two**: the model's `route_text`, which the classifiers and the player log
read, and `cena-ui`'s `LineAssembler` (`crates/cena-ui/src/lines.rs:33`), which only
Despana's pump uses (`crates/cena-web/src/presentation/pending.rs:115`). Responses must
attach to the line a viewer shows, so the viewer must show the model's line. The
differences, and what happens to each:

| | Model (`route_text`) | Despana (`LineAssembler`) | Resolution |
|---|---|---|---|
| Boundary | `ends_line`, which the parser sets on exactly one text frame per wire line (`crates/cena-protocol/src/parser/emit.rs:27`) | `ends_line`, **plus** a flush at every prompt, **plus** a split at embedded `\n` | **MEASURED 2026-09-26 on the 23 committed fixtures: 0 lines open at a prompt, 0 newlines in a text frame** (`crates/cena-protocol/tests/lines_at_the_prompt.rs`, which also shows the check can fail). The same check over real traffic is the gated `no_line_is_left_open_at_a_prompt_on_real_traffic` in `corpus_replay.rs`, **not yet run**. Until it is, the fixtures say the two agree on text. |
| Room components | room state, not stream lines | lines on stream `id` | stay a rendering of room state; whether triggers see component bodies is an open item (§8) |
| Bounds | none per line; 2,000 lines per stream | 16 KB, 256 runs, 32 pending streams | the bounds move to the published line |
| `;sorter` | -- | a Despana switch | **moved into the session**, before the matcher: VellumFE sorts before it highlights, so each sorted line is matched. BUILT: `crates/cena-model/src/sorter.rs`, applied in `crates/cena-session/src/actor/line.rs` |
| Quiet windows | -- | `Event::Quiet` hides a quiet command's report | unchanged; the viewer still tracks `Event::Quiet` |
| `ifClosed` | declared (`stream_windows`) | stamped by the pump | unchanged |

`LineAssembler` has gone; what is left of it is `cena_ui::story_lines`, one finished line to
the story lines a viewer draws. **CORRECTED 2026-09-26:** this said `WIRE_VERSION` would
move from 1 when a line gained its responses. It did not need to. A run's paint is two
optional fields, `color` and `background`, which is how `WIRE.md` has always grown
(`view.map_location` and `view.group` are optional additive projections, and older servers
omit them); `crates/cena-ui/WIRE.md` records the fields.

---

## 5. The file

### 5a. One file, by name, sorted by category

`<CENA_DATA_DIR>/triggers.toml`, TOML like the hunt's profiles, a table per trigger keyed by
its **name** (names are unique, and a name is what `;trigger set` addresses):

```toml
[trigger."stunned"]
category = "Combat"
text = "You are stunned"
look = { color = "#ff4040", bold = true, span = "line" }

[trigger."ignore-spell-spam"]
category = "Ignores"
regex = '^\w+ gestures\.$'
squelch = true
```

Written sorted by category, then name (author, §1 row 3). **Master switches** per category
and per response kind (all squelches off, all substitutes off) -- the Saga complaint, *"no
master toggle for ignores"*. A rule the file cannot type is **refused by name** on load --
VellumFE logs and skips a bad regex, which is a rule silently gone.

### 5b. Per character -- CLAUDE'S READING of "one file", to confirm

A trigger applies to everyone unless it says otherwise:

```toml
[trigger."stunned"]
characters = ["Nisugi"]            # only these; absent means everyone

[trigger."ignore-spell-spam".for.Dicate]
enabled = false                    # one character's override, field by field
```

The override merges field by field, which is the hunt chain's `overlay` rule
(`crates/cena-behavior/src/hunt/chain.rs:95`) reused, and better than VellumFE's whole-rule
replace (§2a).

### 5c. Editing it without a GUI

The GUI editor is the front door, later. Until then, **`;trigger`**, on the pattern the
author set for the hunt -- *"You can't expect me to test ;hunt if I have to write files by
hand"*: `list`, `show`, `add`, `set`, `unset`, `on`/`off` (a trigger, a category, a kind),
and **`test "<line>"`**, which runs a line through the real matcher and says what would
fire -- VellumFE's `.testline`, and its sorter's rule that *"the transform is a pure
function, so the preview is always truthful."*

**As built, CLAUDE'S, to confirm:**

- `add <name> <words>` makes the words **bold**: a trigger must do something to be read
  (§5d), and bold is the least a look can do. `set` changes it from there.
- `on|off <name>`, `on|off category <name>`, `on|off every <kind>`: the kind words need
  `every`, so a trigger named `squelch` is still a trigger. `remove` and `reload` were added.
- **Every change is read back as Hydra reads it**, and refused with the reason if its
  trigger would be refused; nothing is written then. The file is written sorted by category,
  then name, whole or not at all (a file beside it, then renamed over it).
- **A change reaches this character at once, the others at `;trigger reload` or their next
  start**: one file serves every character, but a command runs in one session, which holds
  no handle on the others. A change reaching every running character is a later step.
  **SUPERSEDED by Stage 6**: every running character reads it again.
- `test` tests a main-stream line, without markup. A trigger limited to another stream
  cannot be tested yet.

### 5d. Decided while building step 2 -- CLAUDE'S, to confirm

| Question | Built | Why |
|---|---|---|
| Does a pattern ignore case by default? | **Yes**; `case_sensitive = true` turns it on | Wrayth's default, on the author's reading of `case="y"` (§1 row 4), and Wrayth is the first import. `VellumFE` defaults the other way, so the field is named for what it turns on |
| One bad trigger among many | **Left out and named; the rest load** | 1,500 highlights should not all go for one regex; `VellumFE`'s silent skip is what §5a refuses |
| A bad `for.<name>` | **Refuses the whole trigger, for everyone** | the alternative runs that character on the copy the player was changing |
| A bad `[categories]` or `[responses]` | refused by name; **every switch on** until fixed | a switch cannot be half-read |
| Squelch with a look or a redirect | **Allowed** | a `for.<name>` can add a squelch to a coloured trigger and cannot remove the colour; which one wins is the matcher's rule (step 3) |
| `stream = "main"` | read as the model's `""` | the player's word for it |
| `characters = []` | refused | leave it out to mean everyone |
| Does a literal match only whole words? | **Yes**; `whole_word = false` matches inside words | Wizard FE's default, documented (`reference/wiki_clean/Wizard _front end_.txt:563-573`), and `VellumFE`'s check (`src/core/highlight_engine.rs:423-437`). **AUTHOR, 2026-09-27**, of Wrayth: *"I don't think it was intended to highlight nisugi out of nisugis if it did work that way."* |
| Where is the boundary needed? | **Only where the literal's own edge is a letter, digit or `_`**, where a regex's `\b` sits | Wizard FE and `VellumFE` need it on both sides whatever the edge. Wizard FE's own `SEND[` example misses a GM's name for it, and a Saga player's `[DemsDen] `, trailing space deliberate, broke when Saga's import turned whole words on (`reference/discord/saga-thread.txt:21114-21116`, `:21198`). Both match here. Wrayth's rule is UNVERIFIED: that player is *"absolutely certain"* Wrayth had no whole-word setting, and §2d's exports carry no attribute for one |

`regex` cannot do lookaround or backreferences (§8 item 6); such a regex is refused with the
crate's own message, which says so.

---

## 6. Stages

Each ends in something demonstrable without a live login.

### Stage 1 -- one line, and the first responses

1. **One line assembly** (§4a): the session publishes each finished line; the equivalence
   diff over the golden corpus comes first; Despana reads the published line; `;sorter`
   moves into the session; `LineAssembler` retires. **BUILT 2026-09-26** in three commits:
   the published line (`7fdcf74`), Despana drawing it (`b190f61`), and `;sorter` in the
   session (`1badb6e`). The corpus half of the equivalence check has not run.
2. **The rule type and the file** (§5): load, validate, refuse by name; categories, master
   switches, `characters`, `for.<name>`. **BUILT 2026-09-26**: the rule in
   `crates/cena-model/src/trigger.rs`, the file in `crates/cena-behavior/src/triggers.rs`,
   12 tests in `crates/cena-behavior/tests/triggers_file.rs` (six guards mutation-checked:
   the field-by-field override, names ignoring case, both switches, the order, the group
   bound). Nothing reads the file at startup yet: that comes with step 4, when a trigger has
   something to do. Writing it sorted comes with `;trigger add` (step 5), the first thing
   that writes it. §5d records what was decided while building.
3. **The matcher** in `cena-model` (§4): two automata and a `RegexSet`, deterministic order.
   **BUILT 2026-09-26**: `crates/cena-model/src/trigger/matcher.rs`, 10 tests in
   `crates/cena-model/tests/trigger_matcher.rs`. Every place every trigger matches, the
   automata searched overlapping; triggers ranked by priority, then file order; the set only
   nominates, with the crit tables' raised DFA budget, and falls back to trying each regex
   (checked: the same hits with the set disabled). Eight guards mutation-checked, and one
   survivor closed by a test it had no input for. Which of two overlapping looks wins is
   step 4's, not the matcher's: it reports both, in rank order.
4. **Look and text responses**: colour, squelch, substitute, redirect. Tested: the model's
   scrollback, the chunk and the player log see the game's text unchanged.
   **The model's half BUILT 2026-09-26**: `Matcher::respond`
   (`crates/cena-model/src/trigger/respond.rs`) turns a finished line into the lines a
   viewer is given, and the line type moved down to `crates/cena-model/src/line.rs`, gaining
   its paint. 10 tests in `crates/cena-model/tests/trigger_respond.rs` over lines the real
   parser finishes; nine rules mutation-checked. How the responses combine is §6a.
   **The session's half BUILT 2026-09-26**: `SessionHandle::set_triggers` puts a character's
   matcher on the session (beside `;sorter`'s switch, so it outlives a reconnect), and each
   finished line -- each sorted line, with `;sorter` on -- is answered before it is published
   (`crates/cena-session/src/actor/line.rs`). The binary reads the file as each character
   starts and says what it left out (`crates/cena/src/triggers.rs`). Tested in
   `crates/cena-session/tests/published_lines.rs`: a squelched swing and a rewritten damage
   line leave the model's scrollback, the player log, the combat event and the creature's
   damage exactly as the game said; triggers see each sorted line. Both wiring points
   mutation-checked. The wire still carries no colour: step 6.
5. **`;trigger`** (§5c), including `test`. **BUILT 2026-09-26**: the changes in
   `crates/cena-behavior/src/triggers/edit.rs` (8 tests in
   `crates/cena-behavior/tests/triggers_edit.rs`), the command in `crates/cena/src/triggers.rs`
   with its words (`triggers/words.rs`) and `test` (`triggers/explain.rs`), 14 tests. Six
   guards mutation-checked. §5c records how it was built.
6. **Despana renders look responses** -- the only frontend until the GUI. **BUILT
   2026-09-26**: a wire run gains optional `color` and `background` (`#rrggbb`);
   `cena_ui::painted` lays a published line's paint over its runs, for Despana now and the
   GUI later; the page refuses anything but `#rrggbb` and sets the colours through the
   CSSOM, which its `style-src 'self'` allows. Tested in `crates/cena-ui/src/lines.rs`,
   `crates/cena-web/src/presentation/tests.rs` and
   `crates/cena-web/assets/tests/session.test.mjs`; four guards mutation-checked. Not yet
   seen in a real browser: the smoke needs a Playwright module, and none was found here
   (global npm, the projects under `G:\dev`); Chrome is installed.
7. **A bench**: the golden fixtures against the author's Wrayth set (~220 rules) and a
   synthetic 1,500. The budget is set from the measurement, with VellumFE's ~17 µs a line
   as the reference point, and 25 sessions in mind. **The harness is BUILT**
   (`crates/cena-model/tests/trigger_bench.rs`, `#[ignore]`d): synthetic sets of 220 and
   1,500, the author's own waiting for Stage 4's importer. **The budget is NOT set**: that
   needs a release run, and the author builds debug for testing (`CLAUDE.md`), so it is
   theirs to run or approve. MEASURED in DEBUG only, for scale and not as a budget, over the
   438 lines the fixtures finish: 0.54 µs a line with no triggers, 5.2 µs with 220, 10.3 µs
   with 1,500 (`cargo test -p cena-model --test trigger_bench -- --ignored --nocapture`).
   `VellumFE`'s 17 µs is a release figure for parse and process together, so not
   comparable.

**Done when** a scripted two-character session shows a colour, a squelch, a substitute and a
redirect, one of them limited to one character, in Despana. **MET 2026-09-26**:
`crates/cena/tests/web_triggers.rs`, two characters' pages over one file, the redirect
Nisugi's alone; without that limit it fails on Dicate's page, checked.

### 6a. How responses combine -- CLAUDE'S, to confirm

| | Built | Why |
|---|---|---|
| Squelch | any one hides the line, whatever else matched; **the line is not published** | answers §8 item 3: a viewer has nothing to draw, and `;trigger test` says what would have hidden a line. A "show squelched" view would need it published, marked; nobody has asked |
| Substitutes | in rank order; one overlapping a substitute already made is skipped. A regex's `$1`/`${1}` fill in; a literal's `$` is text | one answer where two rewrites claim the same words |
| Matching | once, on the game's text; a substitute's output is not matched again | no trigger feeds on another's output |
| Looks | each of colour, background and bold decided alone: priority, then a match or group over a whole-line look, then file order. Bold is on if any look sets it | a whole-line look is the backdrop and a word's look sits on it, unless priority says otherwise |
| A look over substituted words | covers the replacement | the look was on those words |
| Redirect | the best-ranked one decides; `copy` shows the line on both | one line, one destination |
| A substitute's style and link | those of the run it starts in | a substituted name is still the link it was |

### Stage 2 -- events, conditions, flags

1. The guard evaluators move down to `cena-model` (author, §1 row 2); the hunt reads them
   from there. **BUILT 2026-09-26**: `crates/cena-model/src/guard.rs` with `guard/`, moved
   whole with `git mv` and its cap; the hunt keeps its `hunt::guard` path through a
   re-export, so no hunt code changed; the words' tests moved with them
   (`crates/cena-model/tests/guard_words.rs`).
2. Condition triggers, edge-triggered (§3a). **BUILT**: `condition = "<guard words>"`,
   read at each prompt, on `VellumFE`'s three rules
   (`crates/cena-model/src/trigger/edges.rs`).
3. Event triggers over the typed facts, as a closed vocabulary; each word names its source.
   **BUILT**: `event = "<word>"`, eleven words over classifiers the model already runs
   (`crates/cena-model/src/trigger/event.rs`, whose table names each source).
4. `only_if`, checked (§3a's *gate*, renamed). **BUILT**: guard words read against the
   session's state when a line or a condition would fire.
5. The flag response, readable by guards. **BUILT**: `flag = { name, seconds | clear }`,
   in the game state (`crates/cena-model/src/state/flags.rs`), published as `Event::Flag`
   and folded by the hunt; the guard word `flag "<name>"` reads it.

**Done when** a line's trigger sets a flag that a condition and a hunt's guard read, with a
condition firing once on its rise, headless. **MET 2026-09-26**:
`crates/cena-session/tests/trigger_flags.rs` (a squelched line sets its flag; a condition
fires at the third prompt and not the first), `crates/cena-model/tests/trigger_stage2.rs`
(every event word against its classifier's own wire lines, `only_if`, the flag and its guard
word, the edges). Twelve mutants, all caught, three of them in the session.

### 6b. Stage 2 as built -- CLAUDE'S, to confirm

| Question | Built | Why |
|---|---|---|
| The event words | `speech`, `whisper`, `my_attack`, `attacked`, `their_attack`, `departure`, `affliction [<name>]`, `incident [<name>]`, `idle_warning`; incident names are Lich's `messages.rb` keys, affliction names the model's ids | each reads a classifier the model already runs on every line, so no word is a second parser (§ settled: one parser, N classifiers) |
| Not words, and why | a death: the game's `death` stream, which `stream = "death"` reads. A failed move: `state/movement.rs` reads a line only as a move's answer. A creature attacking someone else, an attack by my group: left for when asked | a word that reads ordinary English as a fact is worse than no word |
| An `event` beside `text` or `regex` | narrows them: `event = "speech"` with `text = "Nisugi"` is Nisugi named in speech. Alone, it covers the whole line | the ask in §2b is by speaker and event class, and a line of speech is still words |
| Whose target a word such as `stunned` reads | the game's current target (`dDBTarget`'s value, `state/targeting.rs`) | a trigger aims at nothing; the game's target is the one the player has |
| The hunt's words in a trigger | `once`, `once_here`, `every` refused (they read what a routine sent); `splashy`, `nomagic` refused (they read the map, which the session has not got) | a rule that cannot work is refused, never kept silently false (§5a) |
| When a condition is read | at each prompt, after the chunk is applied | the one moment every frame of a chunk has landed; a timed flag's end is noticed at the next prompt |
| `rearm` | 3 game seconds unless set, `VellumFE`'s `DEFAULT_REARM_SECS` | its reason holds here: health on its line would fire every prompt |
| A condition's memory | beside the matcher, replaced with it: new triggers read silently first; a reconnect keeps it | a reconnect's unknown readings are no readings, so the memory neither fires nor re-arms across one |
| What a condition may do | set or clear a flag (attention is Stage 3, held; act is Stage 5); a line's response is refused | a condition has no line to colour, hide, change or move |
| Where flags live | the game state, set by the session and published as `Event::Flag` only when it changed something; the hunt folds it | every copy of the state must agree, and a behavior folds the session's events; a reconnect keeps them, the player's and not the game's |
| A timed flag's clock | the game's; set before the clock is known, it reads unknown | every other expiry in the model is on the game's clock |
| `;trigger test` | takes every `only_if` to hold and names it; a markup-reading event (`speech`, `whisper`, `departure`) does not see typed text | the command runs inside the session and cannot await its state; saying so beats a wrong answer |
| `[responses] flag = false` | turns every flag off, as the other kinds | the master switches are per kind (§5a) |
| The terminal | does not print flag changes | its rule: lifecycle, notices, sends and retries (`crates/cena/src/watch.rs`) |

### Stage 3 -- attention (held 2026-09-26, released 2026-09-27, §1 row 5)

Recorded when it was held: once per occurrence across characters; cooldowns and a cap
(VellumFE's, §2a); an audio library is a dependency decision, measured like `plan/23` §D1b
measured axum; who plays a sound when no viewer is open.

**BUILT 2026-09-27**: sounds, OS notifications and banners.

- The responses are `sound = "<file or path>"`, `notify = true | "<words>"`, `alert = true |
  "<words>"`, and `cooldown = N` (`crates/cena-model/src/trigger/attention.rs`). `true`
  says the line as the game sent it, or a condition's name; words fill in a regex's `$1`.
  They fire on a squelched line (§4) and from a condition, and `[responses]` switches each.
- The session decides: each call passes its trigger's cooldown, per character, on the
  game's clock (`Cooldowns`), and is published as `Event::Attention`.
- The binary plays: one desk for every character, on a thread of its own
  (`crates/cena/src/attention.rs`). A call another character made within a second is the
  same occurrence and passes silently; the sound is found by path or in `<data dir>/sounds`
  as `VellumFE` finds one; the audio device opens at the first sound. **With no page open,
  it still sounds**: that answers who plays it.
- Wrayth's `sound` imports as the trigger's `sound`, no longer `held`; a sound found nowhere
  is said at load, once, with where to put it.
- The banner is a page's: the pump keeps a character's `alert`s and sends each as an
  `alert` message after the update that carried its line, to the pages open then, never in
  a snapshot (`crates/cena-ui/WIRE.md`). The page shows it as text for 4 seconds, at most 5
  at once, `VellumFE`'s `DEFAULT_DURATION_SECS` and `MAX_CONCURRENT`.

**Done when** a trigger's sound and notification reach the desk once however many characters
saw the thing, and its banner reaches its own character's page after the line and no other
page. **MET 2026-09-27**: `crates/cena/src/attention/tests.rs` (one occurrence, three
characters), `crates/cena/tests/web_alerts.rs` (Nisugi's page told, Dicate's not), the page's
`session.test.mjs`. Twenty mutants across the stage, all caught. **Not heard or seen**: no
sound has been played or notification shown by a test; that is the author's first run.

### 6d. Stage 3 as built -- CLAUDE'S, to confirm

| Question | Built | Why |
|---|---|---|
| The audio library | **`rodio` 0.22**, wav, mp3, ogg and flac | `VellumFE`'s (0.19 there), the author's *"we already solved sound in vellum"*; MEASURED: `Cargo.lock` 248 -> 363 entries across every target, with second copies of `thiserror` 1 and `windows-sys` 0.45; on Linux it links ALSA, so CI installs `libasound2-dev` |
| OS notifications | **`notify-rust` 4.18**, zbus on tokio | `VellumFE` has none to follow; its default brings `async-io`, a second executor, which `plan/23` §D1b rules out; MEASURED: 47 crates in an empty crate, most shared with `rodio` on Windows |
| Where it plays | the binary, not a core crate and not a page | a session nobody watches still sounds (§4, headless first-class); the core crates cross-build for Android and iOS, and the binary does not |
| Cooldown | per character and per trigger, 3 game seconds unless `cooldown` says, in the session | `VellumFE`'s `DEFAULT_COOLDOWN_SECS`; on the game's clock as every expiry in the model is; in the session so the desk and a page agree on what came |
| Several characters, one occurrence | the same trigger, sound and words from another character within a second passes silently | the merged streams' second (`cena_ui::Merger`); the same character twice is two occurrences, which its cooldown already let through |
| A notification's title | `Hydra: <character>`, the first to see it | several characters' one occurrence names the first |
| A sound not found | said once at load and after each change, grouped, with the sounds folder | a Wrayth path from another machine is the likely case |
| The cap on banners | the page's: 5 at once, 4 seconds each | `VellumFE`'s `MAX_CONCURRENT` and `DEFAULT_DURATION_SECS`; the pump keeps the newest 5 between publishes |
| A banner on the wire | a new `alert` message within version 1, never in a snapshot | a page opened later is not shown what it missed; a page from an older process cannot pair with a newer one (the pairing token is per process), so no older page meets the new kind |
| The hub page | shows no banners | each banner is a character's; the hub's merged streams already carry what several characters heard |

### Stage 4 -- import

Wrayth XML first (§2d):

| Wrayth | Hydra |
|---|---|
| `<strings>` `text color bgcolor` | a text trigger, literal, look response; category "Wrayth strings" |
| `line="y"` | the look covers the whole line |
| `case="y"` | case-sensitive (§1 row 4, UNVERIFIED) |
| `sound` | kept, **held** (Stage 3) |
| `<names>` | text triggers; category "Wrayth names" |
| `<ignores>`, `disable` | squelch triggers; category "Wrayth ignores", switched off when `disable='y'` |
| `@N` | resolved through that file's `<palette>` at import |
| `<presets>`, `<palette>`, `<macros>` | **not triggers**: the GUI's theme and keybinds (`plan/28`, `plan/12` §6a.3) |

The fixtures are **cut small** from the author's exports, the way the golden corpus was cut,
not committed whole. Imported triggers carry their origin, so a future shared file's act
responses can arrive held (§1 row 1).

**A lead, UNVERIFIED:** Wrayth can keep highlights server-side, in the login `<settings>`
blob, which Cena parses and drops (`crates/cena-protocol/src/frame/vocabulary.rs:475`,
`Frame::ClientSettings`). `plan/15`'s login capture suggests the server sends it only when
the client asks. If it can be had, import needs no file. **Not pursued in Stage 4**: it
needs a live login to see, which is the author's.

**BUILT 2026-09-27.** The reader and the conversion are
`crates/cena-behavior/src/triggers/wrayth.rs`, the merge `edit::import`, the command
`;trigger import <path>` (`crates/cena/src/triggers/import.rs`). The fixture is
`crates/cena-behavior/tests/fixtures/wrayth.xml`, 13 entries cut from `Nisugi3.xml`, with
two sound paths shortened. **Measured on the author's four exports** (a test run and thrown
away, not committed): every entry imports, nothing refused or noted -- `Nisugi3.xml` 222
triggers, `NewLayoutWrayth.xml` 224, `Mnstr.xml` 10, `YepCock.xml` 224 -- and each file's
triggers build one matcher. Eleven mutants, all caught.

**Done when** a Wrayth export is one `;trigger import` from being on, and importing it again
changes nothing. **MET**: `crates/cena/src/triggers/tests.rs`,
`import_brings_a_wrayth_file_in_and_again_replaces_it`.

### 6c. Stage 4 as built -- CLAUDE'S, to confirm

| Question | Built | Why |
|---|---|---|
| An XML library? | **No**: the dialect is measured (no comments, CDATA or doctype; every element in the four sections closes itself; `'` and `"` both quote; `&apos;`, `&gt;`, `&quot;`) and read by hand, quotes honoured | `hunt/yaml.rs`'s precedent (`plan/05` §-1); the workspace has no XML crate |
| A trigger's name | **its words**: `[LNet]-`, `Maravel`; words twice in one file get `(2)` | what a player looks for in `;trigger list`; `VellumFE`'s `wrayth_merchant_2` slugs lose it |
| Names as one trigger per style, as `VellumFE`? | **No**, one trigger per name | `VellumFE` merges because its engine is multi-literal; here one literal is one trigger, and a name is removed alone |
| Importing a file again | **replaces** what that file brought (`origin = "Wrayth: <file>"`), whatever was changed since | the same file twice is the same file once; a change made since is lost, and the import says how many it replaced |
| A name the player already uses | **left to the player**; the import's is `<name> (Wrayth)` | the player's own rule outranks an import |
| A sound | kept as `held = { sound = "<path>" }`, the trigger's look on | Stage 3 is held; the path is from the original machine, as `VellumFE` notes |
| An entry with no colour Hydra can show (`skin`, a palette miss, not `#rrggbb`) | **left out and noted**, unless the other colour stands | a trigger with no response is refused (§5a); the note says which |
| `<ignores>` | imported, as squelches; `disable` sets the category's switch both ways | `VellumFE` does not import them; the Saga complaint is the master toggle |
| Whole words | Hydra's default (§5d) | Wrayth's own rule is UNVERIFIED, and the author settled the question it raised (§5d): a name inside a longer word was never meant |
| `case="y"` | case-sensitive | the author's reading, §1 row 4, UNVERIFIED; `VellumFE` reads it the same |

### Stage 5 -- act (author, §1 row 1)

- A trigger may send **a line, as if typed**: through the `;` command table first, then to
  the game.
- A new command origin, **`Origin::Trigger`**, beside `Manual` and `Script`
  (`crates/cena-session/src/command/verdict.rs:39`, `:85`), so a log can tell "the player
  typed this" from "a trigger sent this". It queues as `Script` does: it jumps the queue and
  never preempts a behavior.
- **It never counts as attendance.** Attendance is a person (`attendance.rs`); a trigger is
  not one.
- **Never from a rule that came from someone else** until the player approves it: such a
  rule's act arrives held.
- **A trigger cannot feed itself**: re-arm and a rate limit, so one whose command's echo
  matches its own pattern does not loop.

**BUILT 2026-09-27.** `send = "<line>"`, `$1` filled in from a regex, from a line's trigger
or a condition (`crates/cena-model/src/trigger/act.rs`). The session paces it and publishes
`Event::Act`; the binary sends it through the character's `;` command table first, and
otherwise to the game as `Origin::Trigger` (`crates/cena/src/triggers/act.rs`). A trigger
with an `origin` sends only the line its `approved` names (`;trigger approve <name>`,
`crates/cena-behavior/src/triggers/entry.rs`).

**Done when** a trigger's line reaches the game marked as a trigger's, a Hydra command sent
by one runs, neither counts as a person, and a pair answering each other is held back and
said. **MET**: `crates/cena/src/triggers/act.rs`'s test,
`crates/cena-session/tests/attendance.rs` (`a_trigger_sending_is_not_a_person_either`),
`crates/cena-session/tests/trigger_flags.rs` (the pace, said once). Eight mutants, all caught.

### 6e. Stage 5 as built -- CLAUDE'S, to confirm

| Question | Built | Why |
|---|---|---|
| Who sends | the binary, one task per character, through the handle that character's `;` commands are on | the command table is the binary's (`crate::commands`), and the session decides and paces as it does attention |
| As typed | `;` commands first, then the game; `Gate::None`, as a typed line goes | the author's words, *"as if typed"*; a line in roundtime is answered by the game as a typed one is |
| The pace | at most 5 sends in 10 game seconds per character, across its triggers; the rest held back and said, once a window | two triggers answering each other loop with no single trigger's cooldown broken; `VellumFE`'s triggers never send, so the numbers are mine |
| No clock | nothing is sent | a pace nobody can measure is no pace; the game's clock is known from the first prompt |
| Cooldown | shared with the trigger's attention: one admission per trigger for both | a trigger that sounds and sends is one firing, not two |
| From elsewhere | an `origin` holds the send until `approved` names that very line; a new line is held again | `VellumFE`'s *"the gate is about authorship"*, by the line rather than a hash, since the line is what is approved |
| What is said | a Hydra command unknown, or a line the session refused, names the trigger | a trigger acting where no one is watching must still leave a trace |
| The terminal | `-> [trigger] <line>` | `watch.rs` shows every send by its origin |
| `;trigger test` | names a send and does not send it | a test that acted would not be a test |

### Stage 6 -- one change, every character (2026-09-27)

Not in the plan as written: §5c left it as *"a later step"*, and the author took it as the
sixth stage when Stage 5 was done (*"sure"*, to merging `m6-hunt`, renumbering the Ruby
bridge to `plan/46`, and this).

**BUILT 2026-09-27.** A `;trigger` change, an import, an approval or a `reload` is told on
one channel the table owns and hands to each character, as it hands down the hunt's
`Party` (`crates/cena/src/triggers/follow.rs`, `crates/cena/src/play.rs`). Each character's
triggers task -- the one that sends its trigger lines -- reads the file again for itself
and says who changed it: *"Triggers: from Nisugi: `stunned` added, making "You are
stunned" bold. 1 trigger on."* The character it was typed at reads it at once, with
everything there is to say, and is not told twice. No handle is stored anywhere new
(`single_owner.rs`), and nothing is a process global (Rule 5.2).

**Done when** a change typed at one character is on for another, which says who made it,
and the one that typed it is told once. **MET**:
`crates/cena/src/triggers/follow.rs`'s test. Three mutants (its own change read again, a
change not told, a reload not told), all caught.

**CLAUDE'S, to confirm:** the others are told in one line, not the refusals and held sends
the one typing it was shown: with several characters on one screen, those would be said
once per window. `;trigger reload` now reads the file again for every character, so a file
edited by hand needs one reload, not one per character.

**Still open, for the author** (§8 item 4): whether triggers see the room window's
bodies (`Also here:`), where Wrayth's names list is usually seen.

---

## 7. What the rules require

- **Layering**: no new crate edge. The matcher and the event and condition vocabularies are
  in `cena-model` (`regex` is already a dependency; `aho-corasick` becomes a direct one and
  is already in `Cargo.lock` at 1.1.5, pulled in by `regex`, so no new crate). The file and
  `;trigger` live in `cena-behavior`, beside the hunt's profiles and `settings.rs`. The
  session evaluates. `cena-model` is in the mobile set CI cross-builds, so that job is the
  check.
- **File caps** (800 lines): new modules sized to them from the start.
- **Glossary**: *trigger*, the response word (§3d), *category*, and the published line.
- **Tests before trust**: the matcher's overlap order and the edge rule each get a
  mutation run, per the house finding that a test can pass a mutant because its input never
  reaches the code.

---

## 8. Open, to settle while building

1. **The word** for what a trigger does (§3d): *response* proposed.
2. **Per-character in one file** (§5b): Claude's reading of §1 row 3.
3. **Is a squelched line published at all?** Proposed: yes, marked, so `;trigger test` and a
   future "show squelched" view can explain what was hidden. **BUILT the simpler way: no**
   (§6a). `;trigger test` explains without it; publishing it marked waits for the view that
   would read the mark.
4. **Room components** (the room window): do triggers see their bodies, in Stage 1 or later?
5. **Wrayth's word boundaries**: Wizard FE's docs have a "Not on Word Boundary" option
   (`reference/wiki_clean/Wizard _front end_.txt:563-575`), implying whole-word by default;
   Saga's import defaulted to whole-word and broke matches. The fixtures and one live look
   settle it. **Hydra's own rule is BUILT** (§5d: whole words, the boundary only at an edge
   that is a letter). What remains is Wrayth's, for the importer: whether `John` highlights
   inside `Johnny` in Wrayth is one live look.
6. **Lookaround and backreferences**: the `regex` crate has neither. Lich's scripts use
   lookaround in 486 places across 85 files (`inventory/09-hot-path-measurements.md:66`),
   but highlight sets are overwhelmingly literal. A trigger that needs them is refused by
   name; a second regex engine only if a real rule needs it.
