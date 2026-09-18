# The Wrayth protocol — the game stream

**Source of record:** [`reference/wiki_clean/Wrayth protocol.txt`](../reference/wiki_clean/Wrayth%20protocol.txt) —
a local copy of <https://gswiki.play.net/Wrayth_protocol>, 32,466 bytes / 556 lines, retrieved 2026-08-17.

> **Path corrected 2026-09-18.** This header previously read `research/Wrayth protocol.txt`,
> which **does not exist** (`test -f` → NO; `find . -iname '*wrayth*'` locates the file under
> `reference/wiki_clean/`). The wrong path was load-bearing: at least one later reader searched
> it, got nothing, and concluded the wiki *did not document* `styleIfClosed` and `group`. It
> documents both (§6.7). A citation that silently resolves to nothing manufactures false
> negatives, which is the `plan/05` §−2 failure mode in its purest form.

> **This file is an exception to the `reference/` rule.** `CLAUDE.md` treats `reference/` as
> read-only clones to port *from*, and `research/` as rationale that is never instructions.
> This one is a **protocol reference we implement from**, which puts it in the same class as
> `plan/10-eaccess-spec.md`. It lives in `reference/` because that is where the author put it;
> it is cited from `plan/` because that is what it is. Line cites below are into that file.

`plan/10` covers **login** (EAccess/SGE, `eaccess.play.net:7910`). This covers what arrives on
the **game socket afterwards** — the stream `cena-protocol` parses.

---

## 1. Why this document changes things

The frame vocabulary was ported from VellumFE because `plan/13` §4a says to port where the
knowledge lives in code. That was right, and this wiki does not overturn it. But a
reimplementation from a *port* inherits the porter's blind spots, and this is the primary
source that exposes them.

**Measured 2026-09-18**, comparing `crates/cena-protocol/src/tags.rs` against this document:

| | Count | |
|---|---:|---|
| Tags in Cena's `KNOWN_WIRE_TAGS` | **126** | Vellum's 116 **+10**, dropping none |
| Tags in Saga 0.9.9's recognizer | **118** | Simutronics' own client (§6) |
| Tags in Saga 0.9.1's recognizer | **116** | `+action +reward +task`, `−group` |
| Tags in VellumFE's `KNOWN_WIRE_TAGS` | **116** | not the "~130" `CLAUDE.md` claimed |
| `ParsedElement` variants in VellumFE | **63** | not the "61" `CLAUDE.md` claimed |
| Tags the wiki writes in literal `<tag>` form | **48** | |
| Wiki tags absent from Cena's table | **4** | and none is a real gap — see below |
| Saga tags absent from Cena's table | **0** | were 3; added 2026-09-18 (§6.2) |

> **The 126 figure is the reconciled set, measured 2026-09-18.** It was 123 before the Saga
> dig; `closeContainers`, `room` and `task` were added and nothing was removed. §6.2 carries
> the set difference and the reasoning.

> **Both `CLAUDE.md` figures were wrong** and have been corrected there. They were restated
> from memory rather than measured, which is the failure `plan/05` §−2 exists to prevent.
> ```
> awk 'NR>37 && NR<405' src/parser.rs | grep -cE '^\s{4}[A-Z][A-Za-z]*'      -> 63
> awk 'NR>=175 && NR<=192' src/parser/text.rs | grep -oE '"[^"]*"' | wc -l   -> 116
> ```

```
python -c "..."   # extract KNOWN_WIRE_TAGS, diff against <tag> matches in the wiki text
  -> WIKI LITERAL TAGS: 48
  -> DOCUMENTED IN WIKI, ABSENT FROM OUR TABLE (4): code, dialog, w, window
```

All four are non-gaps, VERIFIED by reading their context:

- **`<w>`, `<dialog>`** — `stgupd` tags, which travel **client → server** inside
  `<!-- CLIENT -->` markers (`:529` region). Cena parses the **inbound** stream. Out of scope
  until Cena writes settings back.
- **`<code>`** — the wiki's own HTML markup, not wire traffic.
- **`<window>`** — a prose placeholder in `ifClosed='<window>'` (`:73` region), not a tag.

**So Cena's table is a superset of the documented vocabulary.** That is the correct direction
to err: Vellum's table was built from real traffic and caught tags the wiki never documented.

### 1.1 The 7 tags Cena added beyond Vellum — audited

Cena's table is Vellum's **plus 7, minus none**. Each was checked against both the wiki and a
stratified corpus sample (271 files, `ls */*/*/xml/*.xml | awk 'NR%40==1'`):

| Tag | In wiki | In corpus | Verdict |
|---|---|---|---|
| `style` | yes (`:394`) | **50,774** hits | **REAL.** Heavily used. |
| `c` | yes | **429** hits | **REAL.** |
| `FEStart` | yes | 0 | **REAL, setup-phase.** Wiki documents it; logs begin after setup. |
| `streamBox` | yes (`:248-270`) | 0 | **REAL, dialog control.** Fed by `dynaStream`. No dialog renderer yet. |
| `clearDialogData` | **no** | 0 | **UNATTESTED.** |
| `map` | **no** | 0 | **UNATTESTED.** |
| `mapInfo` | **no** | 0 | **UNATTESTED.** |

Four are confirmed by the primary source. **Three are attested by neither** — but note that
zero corpus hits is *not* proof of absence: Lich logs begin after the setup phase, which is
exactly where `FEStart` lives and why it also shows zero. Keeping an unattested tag in the
table is harmless (it makes the parser *more* tolerant, never less); **removing** one would be
the risky act. Leave them, and record here that their provenance is unknown.

> **Updated 2026-09-18 after the Saga dig (§6).** A third source now bears on this table and
> it moves **none** of these seven. Saga 0.9.9 names none of them — the full Cena-only set is
> `FEStart LichWebUI c clearDialogData group map mapInfo streamBox`, measured by
> `comm -13` (§6.2) — so `clearDialogData`, `map` and `mapInfo` stay attested by Cena's own
> dispatch arms alone, and their provenance is still unknown. That Simutronics' own client
> does not name a tag is **not** evidence the wire lacks it: `c` is the player's own command
> echo (Saga renders its input itself, as Vellum does, so it never needs to recognize the
> echo) and `LichWebUI` is a third-party injection the wiki explicitly anticipates at
> `:531-532`. **Keep all seven.** The traffic runs the other way for the Saga-only names,
> which are now in the table; see §6.2.

**The wider point stands:** a number of Cena's tags appear nowhere in this document. Each is
real-but-undocumented traffic, a Vellum invention, or a third-party injection — the wiki notes
at `:529-532` that third-party tools inject through the ordinary grammar and are
"indistinguishable from Simutronics traffic by design". At least one Vellum invention is
already known (`vellumImg`, §5), so none is safe to assume.

---

## 1.2 The handshake banner decides how much protocol you receive

**CONFIRMED by the author, 2026-09-18.** This is the single highest-leverage fact on this page,
because it determines what the rest of the document is even describing.

```
/FE:WRAYTH /VERSION:1.0.1.28 /P:WIN_UNKNOWN /XML
```

**That string is what unlocks the extended feed.** There is no other negotiation — no capability
exchange, no option command. The server keys on the banner alone, and serves a *reduced* feed to
anything that identifies as an older frontend.

The extended feed carries at least `<pulse>`, `<exposeContainer>`, and `<inventoryManager>` in
reply to `_inventory manager`. Live-verified 2026-08-12 by changing VellumFE's direct-mode banner
and observing the server begin sending them.

**Independently corroborated here.** `crates/cena-protocol/tests/fixtures/login_setup.xml` — a
real login capture — has the server echoing the version straight back:

```xml
<settingsInfo  client='1.0.1.28' major='19325' crc='3328816843' instance='GS4'/>
```

Three consequences, each load-bearing:

1. **It keys on the banner the *game* sees, not on direct mode.** A proxy that sends a Wrayth
   identity gets the extended feed through the proxy too. Cena connects directly and sends its
   own banner, so this is ours to get right.
2. **A stale banner silently reduces the protocol.** Nothing errors. Tags simply never arrive,
   and a parser built against that feed looks complete while missing whole families. This is the
   worst kind of bug: invisible, and it makes every downstream measurement wrong.
3. **`plan/10` §7.4 already specified this** in four places. Only `spike/eaccess-spike` was stale,
   sending the Lich-era `/FE:STORMFRONT /VERSION:1.0.1.26`. **Corrected 2026-09-18.**

**GemStone sends no `<c>` ready signals at handshake.** DragonRealms does. The spike sent two,
copied from Lich's DR-capable path; removed. DragonRealms is deferred (`plan/12` §9d), so the
DR handshake is not Cena's problem yet.

### 1.2a The corpus was recorded on the extended feed — VERIFIED

This had to be checked, because if the 49.55 GB corpus had been captured on the *reduced* feed,
every tag census taken from it would understate the protocol, and the golden fixtures would be
built against a feed Cena will never see.

Stratified sample, 73 files (`ls */*/*/xml/*.xml | awk 'NR%150==1'`):

```
client='1.0.1.28'   20 echoes, and NO other version
<pulse>             303
<exposeContainer>    61
<inventoryManager>     0
```

**The corpus is extended-feed traffic.** The `inventoryManager` zero is not a gap: it is a
*response* tag, sent only in reply to `_inventory manager`, which Lich never issues. Absence of a
request explains absence of a reply — this is the same reasoning that explains `FEStart`'s zero
(§1.1) and it is why "0 corpus hits" never by itself means "not real".

## 2. The rules this document states, that a port cannot tell you

### 2.1 Newline suppression — NOT IMPLEMENTED, and it is a rendering bug

`:514-518`:

> **Tags followed by newlines suppress newline output except for:** `<a>` (hyperlinks) and
> `<pushBold>` (bold start).

`grep -rn 'newline\|suppress' crates/cena-protocol/src/` finds only buffer-management comments.
**Cena does not implement this rule.**

This is a *correctness* rule, not cosmetics: getting it wrong emits blank lines the real client
does not, so output looks subtly wrong forever rather than crashing once. The two exceptions
are the whole difficulty — a blanket rule is easy and wrong.

### 2.2 Quoting is inconsistent *by design*

`:13`, restated at `:526-527`:

> The same tag may arrive with **single or double quotes** depending on which server system
> emitted it, so parsers must accept both.

This is the protocol's authors stating the permissive-parsing requirement outright. It is no
longer an inference from Lich's use of Ox, or from Vellum's hand-rolled lexer. `plan/05` Rule
2.2 and the parser's tolerance of malformed input are **confirmed by the primary source.**

### 2.3 Timers are absolute epoch end-times, not durations

`:377`:

> `roundTime` and `castTime` report an **epoch end time, not a duration**. Compute the
> remaining span as `value − prompt time`, using the most recent `<prompt time>` as the clock
> reference.

A model that stores "roundtime: 5" is wrong. It stores an end time, and the clock reference is
the **prompt**, not the local machine — which matters because the local clock drifts and the
server's does not. `plan/12` names clock offset as one of four values needing a single owning
field; this is why.

### 2.4 `<d>` executes its contents when `cmd` is absent

`:328`:

> When clicked, executes the command specified in the `cmd` attribute (**or the tag contents if
> `cmd` is omitted**). Commonly used for compass directions but works for any command. A `<d>`
> may be nested inside an `<a>`; **the outer command wins.**

This independently confirms a fix already made. A port-fidelity review measured **31,838 bare
`<d>` against 9,138 with `cmd=`** — 78% of direct links, and every room exit in the game — and
found Cena was emitting `Direct { cmd: "" }`, an empty command. It now emits
`LinkKind::DirectText`, which types the rule this paragraph states rather than faking an
attribute the wire never sent. VERIFIED by running the parser on
`<compDef id='room exits'>Obvious exits: <d>east</d>, <d>out</d></compDef>`.

**Not yet implemented from this same paragraph: `<d>` nested inside `<a>`, outer wins.**

### 2.5 Unknown ids fall back, they do not fail

`:530`:

> A frontend that sees an unknown dialog id should **fall back to a generic dialog renderer**
> rather than assume the id is unknown-bad.

This is the author's drop-nothing rule, stated by the protocol documentation. It is not merely
a Cena preference — the protocol is *designed* for extension by ids it does not know, because
third-party tools inject through the same grammar (`:531-532`: `UberBar`, `UberBounty`,
`FlareLog`, `InfoCenter`, `TargetWindow`, `Missing Spells`, and Lich's `<LichWebUI/>`).

### 2.6 Streams, components and dialogs — the three id-keyed systems

`:9-11`:

- **Streams** are named text channels. Text between `pushStream`/`popStream`, or inside a
  paired `<stream>…</stream>`, belongs to that channel; everything else belongs to `main`.
- **Dialogs** ship positioned controls, and updates are **incremental** — a re-sent
  `dialogData` carries **only the controls that changed**, merged by control `id`. A model that
  replaces state wholesale on each `dialogData` is wrong.
- **Ids are identity** throughout: streams, dialogs, containers, creatures (`exist`), rooms
  (`nav rm`).

Also `:523-524`: **negative `exist` ids are normal** for rooms and NPCs (e.g. `-11225598`). A
parser that treats `exist` as unsigned is wrong.

---

## 3. What this confirms about the M1 slice

`plan/12` §7.1 scopes M1 to room + prompt + vitals. This document names exactly what those need:

| M1 concern | Tags | Wiki |
|---|---|---|
| Room | `nav rm`, `compDef`/`component` (`room desc`, `room objs`, `room players`, `room exits`, `sprite`), `compass`/`dir`, `roommeta`, `resource` | `:94-135` |
| Prompt | `prompt time` | `:353` |
| Vitals | `dialogData id='minivitals'` → `progressBar` (`health`, `mana`, `stamina`, `spirit`) | `:162-306` |

**Progress bar ids**, `:271-281`: `health`, `health2`, `mana`, `spirit`, `stamina`,
`pbarStance`, `encumlevel`, `mindState`, `nextLvlPB`.

---

## 4. Out of scope, recorded so it is not rediscovered

- **`stgupd`** (`<w>`, `<dialog>`, `<panels>`, `<font>`, …) — **client → server** window-state
  upload, inside `<!-- CLIENT -->` markers. Cena parses inbound only, for now.
- **The dialog control vocabulary** — `cmdButton`, `dropDownBox`, `editBox`, `radio`,
  `upDownEditBox`, `streamBox`, and the layout/anchor system (`top`/`left`/`align`/`anchor_*`,
  and `justify`'s bit-encoding at `:248-270`). Cena has no dialog renderer. The tags parse; the
  layout is not modelled.
- **The `cli`/`menu` coordinate dictionary** (`:27-42`, `:400` region) — right-click menus are
  coordinate lookups against a `cmdlist` dictionary, and `<menu>` carries **no labels**; the
  client resolves `coord` → label + command template. Needed for point-and-click, not for M1.

---

## 5. Open, and settleable against the corpus

1. **The 34 undocumented tags.** For each, is it real wire traffic, a Vellum invention, or a
   third-party injection? The corpus (`E:\Gemstone\data\log archive`, 10,849 `.xml`, 49.55 GB)
   answers this directly — and note the trap already found there: `<vellumImg>` is injected by
   VellumFE and **never sent by the game**
   (`reference/VellumFE/src/core/inline_image.rs:5`). At least one Vellum invention is already
   known to exist, so the others are not safe to assume.
2. **Newline suppression** (§2.1) — unimplemented. Needs a golden that would go red without it.
3. **`<d>` inside `<a>`, outer wins** (§2.4) — unimplemented. Measure its frequency first.
4. **Does the wiki match the live wire in 2026?** It documents `roommeta`, which the corpus
   shows rolling out from GST during 2026 — so the wiki is current to at least that change.
   Treat it as authoritative-but-dated: **the corpus is the tiebreaker**, being two years of
   what the server actually sent.

---

## 6. Saga — Simutronics' own client as a third source

**Added 2026-09-18.** A dig into Saga, Simutronics' official Electron client for GemStone IV
and DragonRealms, at `C:\Gemstone\saga-research` (outside every git repo, gitignored twice).
Versions **0.9.1** (2026-08-07) and **0.9.9** (2026-09-06).

### 6.0 The licensing constraint — read this before using this section

Saga is **proprietary**: its `package.json` declares `"license": "UNLICENSED"`. The research
folder's own README states the rule, and it binds Cena exactly as it binds VellumFE:

> Read them to learn how the protocol behaves. **NEVER copy code or data** from here into
> VellumFE (GPL-3.0). **Facts about the wire protocol are fine; their implementation is not.**

**What that permits and forbids, concretely.** Permitted: tag names, attribute names, value
enumerations, message shapes, numeric meanings, sequencing rules — the observable behaviour of
a server Cena also talks to. Forbidden: source code minified or otherwise, data files, and
line-by-line transcription of an algorithm. Every fact below is stated as a protocol
observation in this document's own words. **No Saga fragment appears in any Cena source file
or anywhere in `plan/`,** and nothing under `C:\Gemstone\saga-research` was modified. The
`credentials.enc` and `passwords.enc` files, and everything under `AppData\Roaming\saga`, were
not read.

### 6.1 Why Saga outranks the wiki, and why the corpus still outranks Saga

The wiki is documentation *about* the protocol. Saga is a client that **consumes the same feed
Cena consumes**, written by the people who emit it — the emitter's own account of what it
sends. Where the two disagree, Saga is the better authority on server behaviour.

**But Saga is still a client, and a client's recognizer is a statement about the client.** The
corpus (`E:\Gemstone\data\log archive`, 10,824 `.xml`, 49.55 GB, Oct 2024 → Sep 2026) is two
years of what the server *actually sent*, and it remains the tiebreaker. §6.5 is a case where
it overrules Saga outright, and §6.2 is a case where it overrules a Saga *removal*.

Ordering, in full: **corpus > Saga > wiki.**

### 6.2 The reconciled tag set — Cena 123 → **126**

Measured by set difference on 2026-09-18. Saga's 0.9.9 recognizer holds **118** names; Cena's
`KNOWN_WIRE_TAGS` held **123**:

```
comm -23 saga_sorted.txt cena_sorted.txt   -> closeContainers room task
comm -13 saga_sorted.txt cena_sorted.txt   -> FEStart LichWebUI c clearDialogData
                                              group map mapInfo streamBox
```

**Three added** (`crates/cena-protocol/src/tags.rs`, each with a provenance comment):

| Tag | Attested by | Corpus (1,547-file sample) | Verdict |
|---|---|---:|---|
| `closeContainers` | Saga 0.9.1 **and** 0.9.9 | 0 | **ADDED.** Simutronics names it. |
| `room` | Saga 0.9.1 **and** 0.9.9 | 0 | **ADDED.** Distinct from `roomDesc`. |
| `task` | Saga 0.9.9 only (**new** since 0.9.1) | 0 | **ADDED.** See §6.4. |

VERIFIED that a zero here is weak evidence: §1.1 already records why, and `FEStart` sits in
this table on exactly that footing. Simutronics' own client naming a tag is stronger
attestation than Lich-era capture coverage, which begins after setup and never issues the
requests that provoke several response tags.

**Nothing was removed, and one removal was specifically declined.** Saga **dropped `group`**
from its recognizer between 0.9.1 and 0.9.9. Cena keeps it, because the corpus shows `group`
is live and current — **434 occurrences** in the 1,547-file sample, in **two distinct shapes
that never co-occur**:

```
322  <group id='X' type='X' state='X' cmd='X' name="X">    protocol, inside <objectives>
 43  <group id='X' type='X'/>                              protocol
  9  <group id='X' type='X' state='X' expires='X' cmd='X' name="X">
 52  <group id="X" open="X">                               client settings
  8  <group id='X' open='X'>                               client settings
```

By file date, the two populations are **disjoint and consecutive**: the settings form runs
2024-10 → 2026-01, the `<objectives>` form starts **2026-06** and is current (2026-06 ×1,
2026-07 ×4, 2026-08 ×43, 2026-09 ×5). The settings form is the one the wiki documents at
`:476`/`:482` as a `stgupd` container (`<group id='Left' open='t'>`), which Cena already
consumes whole as part of the settings region (§4).

> **A correction to how this was first argued.** An earlier writeup offered `group` as proof
> that "two live schemas coexist concurrently." They do **not** coexist — no file contains
> both. The conclusion (keep `group`) is right; that reasoning for it was not. The real
> argument is simpler: **the objectives form is the current one, and it postdates the Saga
> version that dropped the tag.** A client removing a name from its recognizer is a statement
> about the client, never about the wire.

**Removal is the risky act.** A superset parser is more tolerant, never less. Nothing was
removed and no entry's provenance was downgraded.

### 6.3 `<reward>` was silently destroying its payload — FIXED

**The one genuine bug this dig found in Cena.** `parser/thin.rs` mapped `reward` and
`celebration` to the same `ActiveEffect { category, id, text, time }` shape.

The wire sends none of those fields. VERIFIED over a 1,547-file corpus sample (stride-7 of
10,824 — a stride no earlier miner used, sanity-checked at 3,627,979 `<prompt>` to rule out
the silent-zero failure mode described in §6.8):

```
624 <reward>, ALL 624 of the form  <reward type='X' amount='X'/>
  0 carrying id=          0 carrying time=        type: fame 312, experience 312
```

Every reward therefore parsed to `ActiveEffect { category: "reward", id: "", text: "",
time: None }` — **an empty husk, with `type` and `amount` dropped entirely.** VERIFIED by
running the parser on `<reward type='fame' amount='20000'/>` and reading that exact output.

This is a **drop-nothing violation** (`plan/05` Rule 2.2), and a worse one than a missing
handler: an unmodelled tag reaches `Frame::UnknownTag` carrying its raw bytes, so nothing is
lost. A handler that *looks* like it is handling the tag destroys it silently.

**Fixed** by removing `reward` from that arm so it falls through to `WindowHints`, which
carries `attrs` whole. Saga's parser reads a third form, `<reward type='custom' label='…'/>`,
which the corpus sample does not contain; an attribute bag needs no change to carry it if it
arrives. Regression test:
`crates/cena-protocol/tests/fixed_defects.rs::a_reward_keeps_its_type_and_amount_instead_of_becoming_an_empty_husk`,
**VERIFIED RED** on reverting the fix.

`celebration` keeps the `ActiveEffect` arm. It has **0 corpus hits** and no handler in Vellum,
so its shape is unattested by any source; it was not measured and so was not changed.

### 6.4 `<objectives>` — the full child vocabulary

Saga's parser reads a richer `<objectives>` tree than Cena models. Stated as protocol shape:

- `<objectives action=…>` wraps `<objective>` elements; `action` distinguishes a full refresh
  from an incremental one.
- `<objective>` carries an id, a type, a state, a name, a description and a location, and
  optionally a cadence, an expiry and a cooldown-until. **Type** is one of quest, bounty or
  society. **State** is one of active, offered, available, complete or cooldown. **Cadence** is
  daily, weekly or monthly. `expires` and `cooldownuntil` are **epoch seconds** — consistent
  with §2.3's rule that this protocol states absolute end times, not durations. A `location`
  value of `variable` means "no fixed location", not a place called variable.
- `<task>` carries progress text, an optional count and max, optional units, and a `done` flag.
- `<reward>` carries a type and amount, or a type of custom with a label (§6.3).
- `<action>` carries a type (accept, abandon or complete), a command to send, and a tooltip.
- A `<prompt>` arriving mid-capture **aborts the capture**. Cena's parser already forces stream
  closure at `<prompt>` for the same reason.

**`<task>` on the wire is far rarer than its prominence suggests:** 4 files of 10,849 in a
full-corpus scan by the author, and **0** in the independent 1,547-file sample. It currently
reaches the user as `Frame::UnknownTag` with its raw bytes, so **nothing is lost today** — this
is a modelling gap, not a drop-nothing gap. Deferred; see §6.9.

> **A correction.** An earlier writeup claimed `<objectives>` "loses every child". It does not.
> `objectives` is *deliberately* excluded from `is_paired` with the reason stated at
> `crates/cena-protocol/src/parser.rs:342-346`, so children dispatch individually:
> `<objective>` and `<action>` keep their full attributes via `WindowHints`, and `<task>`
> surfaces as `UnknownTag` with raw bytes.

### 6.5 `crtrStatus health` — the corpus overrules Saga

Saga's changelog lists a `health` attribute on `<crtrStatus>`. **The wire does not send one.**
VERIFIED, 1,547-file sample:

```
235,476 <crtrStatus>, 0 carrying health=
attrs by frequency: exist 235476, hostile 201106, inferior 147378, ascended 43268,
  prone 36660, stunned 20553, dead 19000, rooted 8326, flying 7696, disoriented 4917,
  sitting 4068, challenging 3354, sleeping 3042, MiniBoss 2172, hovering 1947,
  immobile 1852, rider 872, sympathetic 583, disengaged 233, kneeling 34, mount 7,
  webbed 2, calmed 2
```

The Saga research README flags this itself — "future-proofing, not evidence the live feed sends
it". **This is the ordering in §6.1 doing its job:** a client may recognize more than the
server emits, and only the corpus can tell you which. Cena needs no change; `crtrStatus` keeps
its attributes as a raw bag, which is exactly what makes a future `health` arrive for free.

### 6.6 `<streamWindow>` was dropping 12 of its 15 attributes — FIXED

Cena lifted `id`, `title` and `subtitle` and discarded the rest. Census, 1,547-file sample:

```
1,176,686 <streamWindow>   title 1176686, id 1176686, location 1169402, target 1163813,
  subtitle 1142932, resident 605219, ifClosed 602513, scroll 24207, save 14513,
  appearance 14507, timestamp 2706, styleIfClosed 1353, nameFilterOption 902,
  width 1, height 1
```

`location` and `target` ride **~99%** of these tags. **Fixed** by adding an `attrs` field to
`Frame::StreamWindow` carrying the tag whole, the same bag pattern `crtrStatus` and `roommeta`
already use. Regression test:
`fixed_defects.rs::a_stream_window_keeps_the_attributes_beyond_id_title_and_subtitle`.

**This is a gap against the wiki, not a Saga discovery** — and it is honest to say so. The wiki
lists all 13 documented attributes at `:46` and devotes a named section to
`ifClosed`/`styleIfClosed` at `:73-76`. Cena had the source and did not act on it; Saga is what
prompted the re-read.

### 6.7 `styleIfClosed` — the wiki was right, a reader was wrong

A writeup in this dig claimed the wiki "does not document `styleIfClosed`". **It does**, five
times, including the section at `:73-76` defining the semantics. The reader searched
`research/Wrayth protocol.txt` — **a path that does not exist** — got no hits, and read the
absence as evidence. The file is at `reference/wiki_clean/Wrayth protocol.txt`, which this
document's own header cited wrongly until 2026-09-18.

The semantics, from the wiki at `:74-76`: each stream declares what happens to its text when
its window is closed. `ifClosed` names a destination; `styleIfClosed` names a style applied to
text that falls through. With `ifClosed` absent and `styleIfClosed` set, text falls through to
the main window wrapped in the named style — the wiki's examples are `thought` for thoughts and
`watching` for familiar, which match the values found live in the corpus.

**The lesson is procedural, and it is why this section exists:** a citation that resolves to
nothing does not fail loudly, it manufactures a false negative. `plan/05` §−2 requires citing
`file:line`; this is the reminder that the cite must also *resolve*.

### 6.8 A methodology warning that cost real findings

Several claims in this dig were produced by piping a relative-path file list into a command run
from a different working directory. `grep` reported "No such file" for every entry and the
pipeline returned **0**, which was then written up as "0 occurrences" — evidence of absence,
manufactured from a path bug.

**Any zero in a corpus census is untrustworthy unless the same run also produced a non-zero
count for a known-common tag.** Every census in this section was sanity-checked that way: the
stride-7 run reported **3,627,979 `<prompt>`** before any other number was believed.

### 6.9 Deliberately deferred

Facts recorded here and **not** implemented, each with the reason:

- **`<task>` modelling** (§6.4). 4 files of 10,849; reaches the user intact as `UnknownTag`
  today, so nothing is lost. The name is now in `KNOWN_WIRE_TAGS`. Model it when the Bounty
  behavior needs it, with a real fixture from one of those 4 files.
- **The rest of the `<objectives>` vocabulary** (§6.4) — enumerated states, cadences, epoch
  fields. `plan/12` §7.1 scopes M1 to room, prompt and vitals; objectives are a Bounty-behavior
  concern. `WindowHints` carries the attributes until then, so nothing is dropped.
- **`<reward type='custom' label=…>`** (§6.3). Attested by Saga, **0 hits** in the corpus
  sample. The `attrs` bag carries it if it appears; modelling a shape with no local evidence is
  the speculation `plan/05` §−1 forbids.
- **`crtrStatus health`** (§6.5). The corpus says it does not exist. Adding a field for it
  would be implementing a changelog entry against two years of contrary evidence.
- **Type-ahead pacing, multibox and performance behaviour.** Saga's send-queue policy is
  **client policy, not protocol** — it belongs in `plan/12` if anywhere, never here. Recorded
  in the research folder; out of scope for this document by definition.
- **Newline suppression** (§2.1) and **`<d>` inside `<a>`** (§2.4) remain open. Saga bears on
  neither.
