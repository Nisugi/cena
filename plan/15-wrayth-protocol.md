# The Wrayth protocol — the game stream

**Source of record:** [`research/Wrayth protocol.txt`](../research/Wrayth%20protocol.txt) — a local copy of
<https://gswiki.play.net/Wrayth_protocol>, 32,466 bytes / 556 lines, retrieved 2026-08-17.

> **This file is an exception to the `research/` rule.** `CLAUDE.md` says `research/` is
> "rationale and evidence only. Never instructions." That stands for every other file there.
> This one is a **protocol reference we implement from**, which puts it in the same class as
> `plan/10-eaccess-spec.md`. It lives in `research/` because that is where the author put it;
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
| Tags in Cena's `KNOWN_WIRE_TAGS` | **123** | Vellum's 116 **+7**, dropping none |
| Tags in VellumFE's `KNOWN_WIRE_TAGS` | **116** | not the "~130" `CLAUDE.md` claimed |
| `ParsedElement` variants in VellumFE | **63** | not the "61" `CLAUDE.md` claimed |
| Tags the wiki writes in literal `<tag>` form | **48** | |
| Wiki tags absent from Cena's table | **4** | and none is a real gap — see below |

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
