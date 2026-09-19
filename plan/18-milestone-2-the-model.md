# 18 — Milestone 2: closing the gap between the parser and the model

**Status: DRAFT, unapproved.** Scope answered by the author 2026-09-18; the step order and
the boundaries below are proposals until they are.

---

## 0. The measurement that defines this milestone

The parser is not the problem. Measured 2026-09-18:

| Layer | Count | Command |
|---|---|---|
| Wire tags handled | **107** | `grep -oE '"[a-z]+"' parser/{dispatch,thin}.rs tags/arms.rs \| sort -u \| wc -l` |
| `Frame` variants produced | **52** | `grep -cE '^\s{4}[A-Z][A-Za-z]*' cena-protocol/src/frame.rs` |
| `Frame` variants **consumed by `GameState`** | **12** | `grep -rhoE 'Frame::[A-Za-z]+' cena-model/src \| sort -u \| wc -l` |

**Forty variants are parsed correctly, published as events, and then dropped.** That is the
milestone in one line.

It was misdiagnosed first. `plan/12` §8 calls M2 "frame vocabulary breadth", and I read that
as "the parser is thin" — it is not, and `tests/every_tag_is_observable.rs` already enforces
drop-nothing at the parser. The breadth that is missing is one layer up.

### What the author's first live session showed, and what each symptom actually is

The session of 2026-09-18 23:45 is the evidence, and every visible defect is a *model* gap
rather than a parse failure:

| Symptom on screen | Real cause |
|---|---|
| `[inv]   a` / `[inv] pebbled grey leather doublet` | printer split at a link boundary — **FIXED**, `5ffca05` |
| `[Spells] Ranger Base:` as loose text | `Frame::Spell` parsed, no field holds it |
| services table, premium notice, news inline | no stream routing; everything lands in main |
| `[room desc]` / `[room objs]` / `[room exits]` as three tagged lines | `Room` has `description` and `exits`, no objects/creatures/players |
| `!! 99 events dropped from the ring` | `EVENT_CHANNEL_BOUND = 256` vs a larger login burst |

---

## 1. Scope, as the author set it

Two questions, answered 2026-09-18:

1. **How much to model?** → *"Port Lich's model wholesale."*
2. **Streams?** → *"Model streams now."*

Then a third, once the size was measured:

3. **Lich's 97,060 lines split into wire-driven and text-scraped. Both, or wire first?**
   → **Wire-driven first.**

That third question was worth asking, because the directory listing hides the split:

| | Lines | How it is fed |
|---|---|---|
| Crit tables | ~72,000 | pure data (`plan/13` §4a already blesses the port; `crit.rs` started) |
| Wire-driven model | — | structured XML attributes |
| Text-scraped model | ~25,000 incl. **160 regexes** in `infomon/parser.rb` | running `info`/`skills`/`exp` and matching prose |

**M2 takes the wire-driven half.** Text-scraping needs the ~15-command Infomon sync that
`plan/12` §7.1 puts Out, and its first consumer is M6.

### The boundary moved once it was measured

The author's instruction was *"use the resources for clues, but do everything the right way,
don't follow blindly."* Applying it moved the line **in our favour**:

`<dialogData id='expr'>` carries `field_exp`, `max_field_exp`, `ascension_exp`, `exp`,
`until_next` and the level **as attributes** (VERIFIED in the 23:45 capture).
`<dialogData id="injuries">` names all sixteen body parts the same way. Lich scrapes both out
of prose with regexes *and* reads the dialog (`common/xmlparser.rb:725-735`).

So experience, injuries, stance and encumbrance are **wire-driven**, not text-scraped, and
they come into M2 — without a single regex. Reading Lich's directory names would have put
them in the deferred half; reading the wire put them here.

---

## 2. What gets built

Grouped by what the wire gives us, ordered by what the live session actually sent.

### 2a. Streams — the routing layer (author's explicit ask)

`Frame::{StreamPush, StreamPop, StreamPopForced, StreamResume, StreamWindow, ClearStream}`
all parse today and none is modelled. Measured in the 23:45 session: four windows (`main`,
`room`, `inv`, `reserve`) and three `pushStream` ids.

`GameState` gains a per-stream text buffer, so thoughts, deaths, familiar output and the
login dump stop being spliced into room prose. This is the foundation M4's frontend renders
from; doing it now is what stops every live run being unreadable in the meantime.

**Not** a window manager. A `BTreeMap<String, Vec<Runs>>` and the routing rule — layout is
`cena-ui`'s.

### 2b. Dialogs — the biggest single group

Measured in one session: `combat` ×10, `minivitals` ×8, `expr` ×5, `injuries` ×2, plus
`stance`, `encum`, `espMasterData`, `quick*`.

| Dialog | Becomes |
|---|---|
| `expr` | experience: level, field exp, ascension exp, until-next, mind state |
| `injuries` | per-body-part injury and scar, 16 parts, named on the wire |
| `stance` | stance |
| `encum` | encumbrance |
| `combat` / `quick*` | deferred — UI affordance, not character state |

`ActiveSpells` already lands via `plan/17`'s `Effects`; this adds the rest.

### 2c. Room contents

`Room` has `id`, `description`, `exits`. The wire also sends objects, creatures and players
as `<component id='room objs'>` etc. This is what a `travel` behavior will eventually need
to answer "did I arrive", and what makes a room render as a room.

### 2d. Inventory

`Frame::{Container, ContainerItem, ClearContainer, DeleteContainer, InventoryManager,
InventoryViewItem}` — six variants, none modelled. Worn and held items, by container.

### 2e. The noun registry

`<a exist="347174333" noun="doublet">` — every interactable carries a stable id. Lich and
Vellum both keep this; it is what makes "the doublet" resolvable to a thing. Cheap, and every
later behavior depends on it.

---

## 3. What is deliberately NOT in M2

| Out | Why |
|---|---|
| stats, skills, PSMs, society, bounty | text-scraped; needs the Infomon sync (`plan/12` §7.1, Out) |
| the 160 `infomon/parser.rb` regexes | ditto, and first consumer is M6 |
| crit tables (~72K lines) | pure data, zero consumers until combat exists |
| condition *evaluation* | `plan/17` §6 already put this Out; Vellum's `conditions.rs` is 1,010 lines of frontend concern |
| window layout | `cena-ui`'s, at M4 |

---

## 4. Method

Per `CLAUDE.md`: **read Vellum first, then Lich, then the corpus.** Vellum is the reference
*port* (same language, same servers, same author); Lich is the reference for what the wire
*means*; the 49.55 GB archive is the tiebreaker.

Per the author, 2026-09-18: *"use the resources for clues, but do everything the right way,
don't follow blindly."* §1's expr/injuries finding is the shape that is expected to recur —
where Lich carries a text path for historical reasons and the wire has since offered a
structured one, take the structured one and record the divergence.

Every step ends green: `cargo test --workspace`, clippy, fmt, and the arch tests. The corpus
tier (`CENA_CORPUS`) runs over real traffic for anything touching the parser.

---

## 5. Proposed step order

Ordered so each step is independently demonstrable, and so the author's next live run is
more readable than the last.

| Step | What | Live-visible? |
|---|---|---|
| 1 | **Streams**: routing + per-stream buffers | **yes** — the login dump stops polluting the room |
| 2 | **Room contents**: objects, creatures, players | **yes** — a room renders as a room |
| 3 | **Dialogs**: expr, injuries, stance, encum | yes, once something displays them |
| 4 | **Inventory**: containers and items | yes |
| 5 | **Noun registry**: `exist=`/`noun=` | no — foundation for later |
| 6 | **Event ring**: the 99 dropped events | yes — the warning stops |

Steps 1 and 2 are the ones that change what the author sees on the next run, which is why
they are first.

---

## 6. The open question this does not settle

**`EVENT_CHANNEL_BOUND = 256` is too small for a login burst** (MEASURED: 99 dropped). Raising
it is one constant; the real question is whether a subscriber that needs *every* frame should
be a broadcast subscriber at all, or whether the model is the only thing that must not miss
frames. Step 6, and it wants a decision rather than a number.
