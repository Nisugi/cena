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

~~**GemStone sends no `<c>` ready signals at handshake.**~~ **WRONG -- corrected 2026-09-18 by
live A/B.** This claim came from Saga research plus inference, the spike was edited to match,
and the edit was never run against the server. The author ran it both ways with a
`CENA_SKIP_C=1` toggle:

| `<c>` sent | Result |
|---|---|
| yes | PASS -- **2,347 bytes**: the whole login burst, room, exits, inventory, `exposeContainer` |
| no | PASS -- **163 bytes**: stops at `<settingsInfo>` |

**Both log in, so the signals are not required -- but without them the client never receives
the room**, and Milestone 1's criterion 2 is "renders a room". VellumFE sends them
unconditionally with no DragonRealms branch (`reference/VellumFE/src/network.rs:689-694`).
**Send them.** Full account in `plan/10` §4.7.

> Recorded because this was the THIRD copy of the false claim -- `plan/10` and the spike were
> corrected the same day and this one was missed. A fact asserted in three places is corrected
> in three places or not at all.

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

`:514-518`, **quoted exactly as the captured file has it**:

```
Tags followed by newlines suppress newline output except for:
* <code><a></code> (hyperlinks)
* <code>
</code> (bold start)
```

> **The second exception is UNVERIFIED, and an earlier draft of this section hid that.**
> It read *"`<a>` (hyperlinks) and `<pushBold>` (bold start)"* as a blockquote -- which looks
> like a quotation and is not one. Line 518's `<code>` element is **empty**: its contents were
> stripped when the wiki was captured, exactly as `<a>` on 517 would have been had it not been
> escaped. `<pushBold>` is a plausible guess at the eaten tag, not something the source says.
>
> This is `CLAUDE.md`'s citation hazard running the other way. That rule was written after a
> citation to a *nonexistent path* manufactured a false negative; here a citation resolving to
> an *empty element* was read as though it resolved to text. **A source can fail by saying
> nothing as easily as by not existing.**
>
> Settle it against <https://gswiki.play.net/Wrayth_protocol> before implementing suppression:
> the live page will have the tag the capture ate. Do not implement the second exception from
> this file.

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

~~**Not yet implemented from this same paragraph: `<d>` nested inside `<a>`, outer wins.**~~

> **IMPLEMENTED -- this note was stale, corrected 2026-09-18.** VERIFIED by execution against
> `Parser::parse_line`:
>
> ```
> <a exist='123' noun='sword'>a <d>sharp</d> sword</a>
>    Text link=Exist { id: "123", noun: "sword" }  "a "
>    Text link=Exist { id: "123", noun: "sword" }  "sharp"     <- inner <d> does NOT override
>    Text link=Exist { id: "123", noun: "sword" }  " sword"
> ```
>
> The link stack in `parser/markup.rs` keeps the outermost link for every run, so the rule
> holds without special-casing. A bare `<d>` on its own still yields `DirectText`.
>
> **A stale "unimplemented" is worse than no note**: it invites someone to "fix" working code,
> and it is why this correction is recorded rather than deleted.

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

**Most of the protocol is not story text at all** (author, 2026-09-18). MEASURED: of the
**52 `Frame` variants**, exactly **three** carry displayable text -- `Text`, `Prompt` and
`Component` (`grep -cE '^    [A-Z][A-Za-z]*' crates/cena-protocol/src/frame.rs` -> 52). The
other 49 are state, dialog controls, window declarations, indicators and timers. The story
window is the *minority* destination, not the default with exceptions.

That is worth stating plainly because it reframes what the parser is for. Read as "text with
markup mixed in" -- the natural framing for a client that renders a scrolling stream -- the
state tags look like decoration. Read correctly, the wire is **a state feed that happens to
include some narration**, which is the framing Cena needs: its consumers are behaviors and
(eventually) an LLM controller, neither of which reads prose.

> **CORRECTED immediately (author, 2026-09-18).** An earlier draft of this paragraph said the
> story window is "the *minority* destination, not the default". **That is wrong about routing.**
> Story/main IS the default, because *routed text falls back to it when its window is not open*,
> and the protocol says so explicitly through `ifClosed`. The 52-vs-3 measurement is about how
> many variants carry text at all; it says nothing about where text goes.

**`ifClosed` is how a stream declares that fallback**, and the wiki documents four behaviors
(`:73-80`):

| Declaration | When the window is closed |
|---|---|
| `ifClosed` absent, `styleIfClosed` set | falls through to main **wrapped in that style** -- the inline-thoughts look |
| `ifClosed='<window>'` | routed to another window, **chaining** if that one is closed too (voln -> thoughts -> main) |
| `ifClosed=''` (empty) | the stream is a **duplicate**: the server also sends the line to main, so the closed copy drops harmlessly |
| neither attribute | falls through to main, unstyled |

The third row is the room duplication described below: the live capture carries
`<streamWindow id='room' ... ifClosed='' resident='true'/>`, and `ifClosed=''` is precisely the
server saying *"this content also goes to main."* The two room shapes are therefore not an
oddity of the room -- they are this general rule, instantiated.

**Cena PRESERVES `ifClosed` and no consumer reads it.** (An earlier draft of this paragraph
said Cena "drops it entirely"; that was wrong -- the drop-nothing rule held.) VERIFIED:

```
StreamWindow { id: "room", ..., attrs: [..., ("ifClosed", ""), ("resident", "true")] }

grep -rn "ifClosed" crates/ --include=*.rs   ->  3 hits, ALL doc comments or test strings
```

So the value is one field access away; what does not exist is anything that acts on it. Not an
M1 gap -- there is no window manager to route to yet -- but it is the reason **a consumer must
not assume a line it sees on main was *only* on main.**

**The duplication is live and per-move.** From a named corpus file, one room change: the room
description and the object list each arrive **twice** -- once inside `pushStream id='room'`,
once as styled main-window prose. `pushStream id='room'` occurs **516, 486 and 539 times** in
three named files. A consumer counting "objects in the room" from `Frame::Text` double-counts
on every single move, and `ifClosed=''` is the only wire marker saying the second copy is a
copy. `GameState` is already immune because it folds only the window form (below, and `crates/cena-model/src/state.rs`); the exposure
is for any future consumer reading text, **and for an LLM controller fed the text stream.**

**The wire uses the quoted form exclusively.** Measured across twelve named files: **7,246
`ifClosed=''` against 0 bare `ifClosed=`.** That matters because the wiki writes the Copy
declaration bare (`:78`, `:83`) and `text.rs`'s attribute reader rejects unquoted values -- a
KNOWN LIMIT documented there. On this evidence **that limit does not need closing for this
attribute**, and the 46-call-site change its own comment estimates is not justified. Recorded
so nobody re-derives the alarm from the wiki's spelling, as this project already did once.

`dialogData` is the clearest bulk example: a single movement in the live capture carried
`combat`, `minivitals`, `injuries` and `Buffs`, none of it story text, and §2.6's incremental
merge rule applies to all of them.

**Components are a ROUTING signal, not a content type**, confirming against live traffic.
Generally: *anything arriving as `<component>` or `<compDef>` is meant for a **supplemental
window**, as opposed to the story window.* Every id in the committed fixtures
fits -- `room desc`, `room objs`, `room players`, `room exits`, `sprite`; none is story prose.

That matters because **the same content is often sent twice, in both shapes, and the two
are not interchangeable**:

| Shape | Window | Means |
|---|---|---|
| `<compDef id='room desc'>` | room window | **where the character is** |
| inline text styled `<style id="roomDesc"/>` | story window | **what the character saw** |

Verified from a live session: a `look` emits **only** the story form -- zero `component`
frames -- while movement emits both. The window form stays *structured* (`room desc`,
`room objs`, `room players` each separate); the story form is *flattened*, with objects
appended into the prose.

**The asymmetry is load-bearing.** Abilities that look into another room write the story
form **without** the window form, which is exactly how a client knows the character did not
move. A model that folds story text into current-location state turns every scry into a
phantom relocation -- a bug that only appears for players who use those abilities.

`compDef` is therefore truth not just for location but for **creatures, objects and players
in the room, and objects named in the room description**. When `cena-model` grows past M1's
room/hands/roundtime/vitals scope (`plan/12` §7.1), the window feed is the only place those
are cleanly separable. Enforced today by
`crates/cena-model/tests/room_is_where_you_are.rs`.

---

### 2.7 What the corpus says about update semantics — three rules, all already satisfied

From the wiki audit of 2026-09-18. Recorded because **each of these looks like a bug from the
code alone**, and a future reader who "fixes" one would break working behaviour.

**`dialogData` merges by control id; it does not replace.** The wiki (`:166`, `:173-174`) says
a re-sent `dialogData` carries only the controls that changed. The corpus shows two distinct
patterns, and both are handled:

| dialog | pattern | count |
|---|---|---|
| `Buffs`, `Cooldowns`, `Active Spells`, `Debuffs` | `clear='t'` then a **full** refill, on one line | 409/409, 193/193, 150/150, 145/145 |
| `minivitals` | **one bar per tag**, never cleared | -- |

So a wholesale-replacing consumer would drop health/stamina/spirit on every mana tick.
`cena-model`'s `GameState` uses a merge-by-id map (`state.rs`, `self.vitals.insert`), which is
correct **by construction**. Do not "simplify" it into a replace. The feared phantom-empty
buff list cannot happen: `clear='t'` pairs 1:1 with its refill.

**`component` is not a delta.** The wiki calls `compDef` "full replacement" and `component`
"incremental replacement" (`:97-98`), which reads like a diff. Measured: **every `component`
body on the wire carries the complete line** (`You also see ...` in full), and the five empty
`<component id='room players'></component>` occurrences each precede a room header or teleport
-- they genuinely mean "the roster is now empty". So "incremental" means **this one named part,
leaving the other parts alone**, as against `compDef` which redefines all five together. Both
are full replacements *of their own part*, and `GameState`'s overwrite is right for both.

> Distribution, one named file: `compDef` room desc/exits/objs/players/sprite **516 each**
> (a complete set per room change); `component` room objs **1,543**, room players **782**
> (in-room deltas). UNVERIFIED whether a `component` ever appends rather than replaces --
> one character's traffic. That is the single assumption here most worth re-testing.

**The room-change sequence is not fixed.** `:120` describes a fixed order -- `nav`, then
components, then `compass`, then `streamWindow`, then `resource`. The wire disagrees:
`tests/fixtures/room.xml` runs `nav` -> `streamWindow` -> `compDef`x5 -> `resource` ->
`compass`. **Never write code that depends on that order.** Cena is order-independent, which
is correct and should stay that way.

---

### 2.8 Parsed but not modelled — the standing list

All VERIFIED present in the vocabulary and reaching a consumer; none is modelled, and none
needs to be for Milestone 1. Recorded so the next reader finds them here rather than
rediscovering them from the wiki:

| Wiki | What it is | Why it will matter |
|---|---|---|
| `:28` | `cli` command dictionary; `@` = the object's noun, `#` = its exist id | the **substitution rule is protocol**, not UI; `MenuResponse` has no dictionary to resolve against |
| `:293` | `%id%` substitution in `cmd` | an **outbound** obligation -- Cena must substitute before sending |
| `:445-463` | `flag` and ten named settings | **three control whether the room title/description are sent at all** |
| `:496-498` | `monopolize` | a state machine, not a flag |
| `:507-508` | `pushInputState` / `popInputState` | input-mode stack |
| `:429` | `nomenu` | a **negative answer** to a menu request, not an absence |
| `:243` | `injuries-{existID}` | an appraised target's dialog -- now handled in `state.rs`, see §2.7 |
| `:137` | `stow` | a **symbolic** container id, not a literal one |
| `:363-373` | indicator vocabulary | ten ids listed; see below |
| `:400-402` | `output class="mono"` | fixed-width regions |

**Two notes for the wiki itself, which is the incomplete side here:**

1. `:363-373` lists ten indicator ids. **`IconPOISONED`, `IconDISEASED` and `IconBLEEDING` are
   on the wire and absent from that list.** Cena passes ids through verbatim (VERIFIED:
   `<indicator id='IconPOISONED' visible='y'/>` -> `StatusIndicator { id: "IconPOISONED",
   active: true }`), so nothing breaks -- but the primary source is missing three.
2. `<a char= game=>` (`:317-318`) is documented and appears **0 times** in sampled traffic. It
   is the reason `<a>`'s old `DirectText` fallback was dangerous in general rather than in one
   case: a documented-but-unseen attribute is exactly what a fabricating fallback turns into a
   command nobody authored. See `LinkKind::NotActionable`.

---

## 2a. Character status: three channels, and only one of them has an offset

**AUTHOR, 2026-09-18.** Established during Milestone 1's close, from the
author's knowledge plus one pasted live burst.

> **AUTHOR:** *"all of the protocol facts exist in lich, just have to dig them
> out."*
>
> Which is the right frame for this whole section. **Nothing below is new
> knowledge** -- every one of these facts is in `reference/lich-5`, and each
> subsection cites the file. What this section is, is a **map**: where the
> answer lives, and what it settles for Cena. It is not a substitute for
> reading the source, and it should not be quoted in place of one.
>
> It is also a note to self. Three of tonight's four findings came from the
> author correcting a design in progress, and all three were sitting in
> `lib/constants.rb`, `lib/gemstone/infomon/status.rb` and `lib/common/
> xmlparser.rb`. `CLAUDE.md` already says to read VellumFE first for anything
> it implements; the same holds for Lich on protocol semantics. **Dig first,
> then ask about what is genuinely ambiguous.**

### 2a.1 The prompt is a DISPLAY, and it gives onset only

`<prompt time='...'>` carries status letters before the `>`:

```
W Webbed   I Immobilized   i Invisible   P Prone    S Stunned
s Sitting  J Joined        K Kneeling    U Unconscious
H Hidden   C Calmed        R In round-time delay
! Losing HP to bleed/disease/poison      DEAD Dead
```

> **AUTHOR:** *"the prompt is a display. It can be used to determine onset but
> not to determine offset because a prompt isn't sent just because it ended."*

**This is the load-bearing fact.** A prompt arrives when something *happens* --
a command is sent, output is emitted. Nothing announces a condition *ending*,
so the last prompt seen keeps saying `R>` until the next one arrives, which
only comes when you act, which is the thing you were waiting to do.

Reading a prompt flag as a live gate is therefore a **deadlock built out of a
status flag**, and it is exactly the design this correction stopped. Cena's
prompt fixtures all read `>` (6/6, all cut from idle moments), so nothing in
the test suite would have caught it.

### 2a.2 `<indicator>` is the real mechanism, and it has both edges

> **AUTHOR:** *"The actual mechanism is indicator for most of those."*

`<indicator id= visible=>` -> `Frame::StatusIndicator { id, active }`, already
parsed (`parser/thin.rs:109`). `visible='y'` and `visible='n'` mean an
indicator announces **onset and offset**, which is precisely what the prompt
cannot do.

Lich's `ICONMAP` (`reference/lich-5/lib/constants.rb:72`) names eleven:

```
IconKNEELING  IconPRONE   IconSITTING  IconSTANDING  IconSTUNNED  IconHIDDEN
IconINVISIBLE IconDEAD    IconWEBBED   IconJOINED    IconBLEEDING
```

Note that map's letters are **GSL prompt codes, a different alphabet** from
the Wrayth prompt above -- `IconSTUNNED` maps to `I` there and `S` here. Do not
read one as the other.

### 2a.3 The ones with no indicator

Comparing the thirteen prompt codes against the eleven indicator ids:

| Prompt code | Indicator? |
|---|---|
| `W` `P` `s` `S` `K` `H` `i` `J` `DEAD` `!` | yes |
| `I` Immobilized, `U` Unconscious, `C` Calmed, `R` roundtime | **no** |

> **AUTHOR:** *"There are some that are not an indicator."*

`reference/lich-5/lib/gemstone/infomon/status.rb` is where Lich lays out the
split, and it is worth reading in full. Two shapes:

```ruby
def self.stunned?          # indicator-backed: one line
  XMLData.indicator['IconSTUNNED'] == 'y'
end

def self.calmed?           # text-derived: parse AND confirm
  Infomon.get_bool("status.calmed") && (Effects::Debuffs.active?('Calm') || ...)
end
```

**That `&&` is the whole lesson.** A text-derived condition has the same offset
problem as a prompt flag -- text says it started and never says it stopped --
so Lich refuses to trust its own parse alone and requires the debuff still be
listed. `bound?`, `calmed?`, `cutthroat?`, `silenced?`, `sleeping?` and
`thorned?` are all this shape.

It is the same pattern as "send from text, confirm from state" (`plan/16` §2),
arriving from the other direction.

### 2a.4 Roundtime, which has neither

`R` has no indicator, so roundtime is the one condition with **no stateful
channel at all**. It must be computed. From a live burst pasted by the author:

```
<c>search
<roundTime value='1789775126'/>You don't find anything of interest here.
Roundtime: 3 sec.
<prompt time="1789775123">R&gt;</prompt>
```

`1789775126 - 1789775123 = 3`, matching "Roundtime: 3 sec." exactly. So:

- **`roundTime value` is an absolute END time** in server epoch seconds
- the prompt in the same burst carries the server's time **now**
- their difference is the duration, **computed entirely in server time**

That subtraction is the useful part: it cancels clock skew and needs only one
truncation's worth of error, rather than comparing two absolute clocks. Cena
can start a local stopwatch of known length instead of asking "is it 1789775126
yet?" -- which it cannot answer reliably, since both values are whole seconds
and the true boundary falls somewhere inside one.

**UNVERIFIED:** whether `value` means the end of the *start* of that second or
somewhere within it. The residual error is under one second either way.

### 2a.4b The blob: tag at the front, prose at the back, prompt terminates

> **AUTHOR, 2026-09-19:** *"when a character performs an action that gives
> roundtime, there is usually a `<roundTime>` or `<castTime>` tag at the
> beginning of the exchange and the Roundtime prose at the end. Every blob ends
> with a prompt."*

This is the layout of §2a.4's own example, stated as a general rule, and it is
the **unit a combat consumer reads**. MEASURED across six files in
`E:\Gemstone\dev\lich-5\logs\GSIV-Nisugi\2026\09\xml`, taking a blob to be the
text between two prompts:

| File | Blobs with `Roundtime:` prose | ...also carrying the tag | Prose-only |
|---|---|---|---|
| `2026-09-01_15-13-56` | 244 | 244 | 0 |
| `2026-09-01_16-30-27` | 261 | 261 | 0 |
| `2026-09-01_17-54-01` | 256 | 256 | 0 |
| `2026-09-01_19-10-58` | 255 | 255 | 0 |
| `2026-09-02_09-52-10` | 299 | 299 | 0 |
| `2026-09-04_09-15-47` | 397 | 397 | 0 |

**1,712 blobs, zero exceptions.** Also measured: **zero** prompts fall between a
tag and its prose, which is what makes "same blob" the right description rather
than "nearby".

Command that produced it:

```sh
awk '/<prompt/ { if(p){tot++; if(t)both++} t=0;p=0; next }
     /<roundTime|<castTime/ { t=1 }
     /Roundtime: [0-9]+ sec/ { p=1 }
     END { print tot, both }' <file>
```

**Why this is worth a section.** The tag and the prose can be **tens of lines
apart** -- 36 to 50 in the sampled file -- because Lich flushes the previous
action's aftermath (a worn-inventory refresh, a room update, a dialog rebuild)
between them. So *line proximity is not the relationship*; blob membership is.
A consumer that paired them by distance would mispair under load, and one that
required adjacency would find them never adjacent.

> **HOW THIS GOT RECORDED.** M2's `combat_exchange.xml` fixture was first cut
> **across this boundary** -- it began at the exchange prose, so it carried the
> closing `Roundtime: 3 sec.` with no opening tag, and its test comment claimed
> the tag "arrives on its own elsewhere". That inverted the rule: a cut artifact
> was written down as the wire's own division.
>
> The mechanical cause is worth keeping, because it is a §-2 failure with a
> specific shape. Two of my own measurements disagreed, and **the one I believed
> was the broken one**: a line-window search reported "no tag in 1946..1992"
> while the tag sat at line **1945**, one line outside a window I had chosen
> myself. A window search that returns nothing does not announce that its
> bounds were wrong -- it manufactures an absence, exactly as the dead citation
> path in `CLAUDE.md` manufactured a false negative about `styleIfClosed`.
> **Before concluding a thing is absent, check that the search could have found
> it.**
>
> The per-blob measurement above is the version that does not depend on a
> window, which is why it is the one in the table.

### 2a.4a MEASURED, 2026-09-18: the first capture from Cena's own client

`E:\Gemstone\data\cena_logs6-09-18
erten-*.bytes`, 87,494 bytes, ~75 seconds of live
play driven by the binary's six-`search` capture. **Parsed by Cena: 2,344 frames, 0 unknown
tags** -- the first time the parser has been run against traffic this client produced rather
than a Lich archive.

**1. `roundTime value` is the EXACT end instant.** Three independent roundtimes, and in every
one the first plain `>` prompt lands precisely on `value`:

| `roundTime value` | prompt at onset | stated | first `>` again |
|---|---|---|---|
| 1789775819 | 1789775809 | 10 sec | **1789775819** |
| 1789775824 | 1789775821 | 3 sec | **1789775824** |
| 1789775833 | 1789775827 | 6 sec | **1789775833** |

This settles §2a.4's UNVERIFIED note: `value` is not "somewhere inside that second". A
comparison of `game_time_now() < roundtime_end` is exact, not approximate.

**2. `R>` tracks live state -- while prompts are arriving.** 28 `R>` against 30 plain `>`, and
the flag flips off on the prompt at the end second, not the one after. But those prompts exist
only because the `look` behavior was firing once a second. **An idle client receives no prompts
and therefore sees a stale `R>` indefinitely**, which is the author's onset/offset point
holding exactly, and the reason `game_time_now()` (`plan/17` §3) is the mechanism rather than
the flag.

**3. Indicators arrive as ONE bulk declaration at login.** All ten of Lich's `ICONMAP` ids on a
single line, `IconSTANDING` the only `visible="y"`, and **no further indicator traffic in 75
seconds** -- none of the conditions changed, so none was re-sent. They are state declarations,
not an event stream: absence means unchanged, never "not happening".

**4. Attribute quoting is NOT uniform.** `<indicator id="IconSTANDING" visible="y"/>` uses
double quotes; `<roundTime value='1789775824'/>` uses single. A grep or matcher that assumes one
form silently finds nothing -- which happened while reading this very capture, and was caught
only because a broader search contradicted the narrow one. The parser is unaffected (it handles
both), but anything that greps the corpus must not assume.

**4a. A quote inside an attribute value: the server is well-formed, by two
different mechanisms.** Established 2026-09-19 while checking whether Lich's
`XMLCleaner.clean_nested_quotes` repair needs porting. It does not, and the
reason is worth recording because the obvious guess is wrong in both directions.

The hazard is real — possessive names are everywhere in this game, from
`Imaera's Lace` to `jack-o'-lantern` to `Ta'Vaalor` — and a naive
"value ends at the first matching quote" reader truncates them silently. But the
server never produces that input. It does one of two things:

| Mechanism | Example | Where |
|---|---|---|
| **Escape as `&apos;`** | `<d cmd='forage Imaera&apos;s Lace'>Imaera's Lace</d>` | single-quoted attrs |
| **Switch to double quotes** | `<label value="Ta'Vaalor Environs"/>`, `subtitle=" - Widowmaker's Road"` | everywhere else |

Note that both appear *on the same line*: the `&apos;` is in the attribute while
the display text carries the literal apostrophe. `text::attribute` decodes
entities, so `cmd` arrives as `forage Imaera's Lace` — sendable verbatim.

MEASURED, across two corpora:

- **0** malformed attribute regions in **11,576,123** tags from 42 of the
  author's 504 deliberate foraging sessions
  (`E:\Gemstone\data\forge data\forge_sessions\*\raw.xml.log`) — the corpus most
  likely to contain possessive herb names, since 14 of the survey's 716 forage
  names carry an apostrophe (`forge_survey/report/forage_changes.json`).
- **0** malformed, **1,637** well-formed double-quoted-value-containing-`'` in
  **995,407** tags across 8 characters in the log archive. By attribute:
  `text` 1014, `subtitle` 412, `noun` 136, `value` 39.

> **A METHOD NOTE, because it cost two wrong answers.** A first pass reported 48
> malformed tags. All 48 were a regex artifact: `id="1" path=" in #64863953"`
> matched as one span across two attributes. **A malformation survey must walk
> attributes, not pattern-match across them** — the check that produced the real
> answer parses `key="value"` pairs in sequence and reports only a region where
> that walk cannot continue.
>
> A fix was also written and reverted. Ending a value at "the last quote followed
> by whitespace, `/` or `>`" passes a test for `title='Tsetem's Items'` while
> making `id` swallow the rest of the tag, because a *later* attribute's
> terminator satisfies an *earlier* attribute's scan. Tests for the malformed
> case went green while 995,407 tags' worth of working parsing broke — the
> falsification was scoped to the defect and not to what already worked.

Lich's other repairs were measured at the same time and are likewise **not
ported**: open-ended `<component>`/`<dynaStream>`, dangling closes, the
`<d cmd="transfer … nerves">` truncation, `<settingsInfo  space not found >`, and
`...wait N seconds.` room-component buffering all occurred **0 times**. Cena's
`Frame::MalformedTag` already types the cases those regexes repair, which is the
signal Lich had to rebuild deliberately when it moved from strict REXML to
permissive Ox (`reference/lich-5/lib/games.rb:322-355`).

**One Lich strip is worth a second look, for the opposite reason.** It deletes
the bell character (`fix_invalid_characters`, `games.rb:315-325`). Every
occurrence in the sample — 3 characters, 6 bells — wraps the idle-kick warning:

```
\x07YOU HAVE BEEN IDLE TOO LONG. PLEASE RESPOND.\x07
```

`MAX_UNATTENDED_LOSSES = 2` exists *because* the game idle-kicks, and the
supervisor currently infers that from repeated unattended losses. The server
announces it in advance, in plain text. Neither Lich nor Vellum reads it as a
signal — this is a fact they discard, not a port. Recorded here rather than
built: it belongs to the supervisor, not the parser.

### 2a.5 What this means for `GameState`

Every frame above is **already parsed and already discarded**:

| Frame | Parsed at | Stored in `GameState`? |
|---|---|---|
| `StatusIndicator` | `parser/thin.rs:109` | **no** |
| `Prompt { time }` | `parser/dispatch.rs:293` | text only; **time dropped** |
| `Buffs`/`Cooldowns`/`ActiveSpells` rows | `frame.rs:229` | **no** |

`plan/12` §7.1 scoped M1's `GameState` to room, prompt, hands, roundtime and
vitals, which was right for M1. It is now the bottleneck: the protocol layer
runs well ahead of the model, and everything in `plan/16` waits on the model
catching up.

---

## 2b. `crtrStatus` gained health, and two new flags — 2026-09-19

**The server widened this tag while M3 was being built.** The author supplied
`C:\Gemstone\lich-5\logs\GSIV-Nisugi\2026\09\2026-09-19_18-47-47.xml`:

> *"looks like they pushed an update to the crtrStatus feed to include hp"*

MEASURED in that capture — **437 `crtrStatus` tags, and all 437 carry `health`
and `maxhealth`**, which no earlier capture does. (182 is the number of *lines*
containing one; several lines carry a whole room's worth, so count the tags:
`grep -oE '<crtrStatus [^/]*/>' <file> | wc -l`.) Three attribute names are new
against the set recorded for the M2 fixture (`hostile`, `dead`, `stunned`,
`prone`, `rooted`, `flying`, `inferior`, `immobile`):

| New | Seen on |
|---|---|
| `health`, `maxhealth` | every occurrence |
| `ascended` | 331 occurrences, **all 331 also `hostile`** |
| `rider` | 19 occurrences |
| `hovering` | 2 occurrences |

**`health` is signed.** `health="-10"` appears on dead creatures; the observed
range is −10 to 900. A `u16` would wrap that.

### Cena needed no change, and that is the point

VERIFIED by feeding all three new shapes through today's parser: each produced
`Frame::CreatureStatus { id, attrs }` with every new attribute present, zero
unknown tags, and the negative health intact — **including inside a
`<component>` body**, which only works because of the fix committed the day
before (`plan/12` §3a's "reopen" signal, 98.8% of occurrences).

That is `§3a`'s *"the parser lifts the identity because that is structure; it
does not map flag names, because it does not know what a flag means"* being
tested by a live server change rather than by argument. A parser that had typed
the flag set would have needed an edit and a release; one that carries `attrs`
raw needed neither.

**What this does NOT settle:** whether `health` is a percentage or an absolute.
`maxhealth` values vary per creature, which argues absolute, but that is
INFERRED. A consumer that displays a creature's health needs the answer; the
parser does not.

### 2b.1 The canonical flag vocabulary — mined, not inferred

> **AUTHOR, 2026-09-19:** *"I was talking about crtrStatus statuses"* ... *"But
> like you said, they all should work regardless?"*

Both halves matter and they answer different questions.

**The flags work regardless — VERIFIED, not assumed.** All 24 canonical flags
below were fed through the parser at once, inside a `<component>` body, with
`health="-10"`: **24 of 24 survived** into `attrs`, 27 attributes total, zero
unknown tags, `AscensionBoss`'s CamelCase intact. So a new flag needs no parser
work, which is the property `attrs` raw exists for.

**But the LIST is not in the wire, and neither are its semantics.** The capture
that prompted this carried 13 of the 24; inferring the vocabulary from traffic
would have missed 11. `reference/lich-5/lib/common/creature/creature_base.rb`
(748 lines) has all of it, and reading it first would have replaced an hour of
measurement.

#### Two disjoint tables, and the split is semantic

EXTRACTED from `creature_base.rb:112-146`. Left column is the XML attribute,
right is Lich's canonical name where it differs.

**`CRTR_STATUS_FLAGS` — 13 transient states** (`:112`):
`immobile`→*immobilized*, `webbed`, `sleeping`, `disoriented`, `stunned`,
`rooted`, `calmed`→*calm*, `kneeling`, `prone`, `sitting`, `flying`, `hovering`,
`hidden`.

**`CRTR_CLASSIFICATION_FLAGS` — 11 relationship/kind facts** (`:135`):
`hostile`, `disengaged`, `dead`, `sympathetic`, `ascended`, `inferior`,
`AscensionBoss`→*ascension_boss*, `MiniBoss`→*mini_boss*, `challenging`,
`rider`, `mount`.

So the three attributes §2b recorded as unexplained are **classifications, not
statuses**: `ascended`, `rider`, and `mount`; `hovering` is a status. Four names
are renamed between wire and canonical, including two CamelCase.

#### The tag is a SNAPSHOT, and the clearing is deliberately scoped

`creature_base.rb:647-668` (`sync_crtr_status`), the two facts a capture cannot teach:

> *"The tag is a full snapshot of what is currently active, not a delta. A
> missing flag, or a flag set to "0", means inactive even if it was active a
> moment ago, so absent known flags are cleared rather than ignored."*

And the scoping, which is a bug someone already hit:

> *"Scoped to `CRTR_STATUS_FLAGS` on purpose: `@status` has two writers, this
> feed and the combat parser reading messaging. The feed is a full snapshot only
> of the states it reports, so it owns removal for exactly those — and must not
> touch the rest. blind, poisoned, natures_decay and the other message-only
> effects never appear in the tag; reconciling them here would clear them on the
> next one and make them impossible to model."*

MEASURED against `STATUS_DURATIONS` (`creature_base.rb:71`, 23 entries): **17
statuses exist only in messages and never in the tag** — `amputated`, `bind`,
`blind`, `breeze`, `crippled`, `dazed`, `entangle`, `hypnotism`, `limb_favored`,
`mass_calm`, `poisoned`, `roundtime`, `silenced`, `sleep`, `slowed`, `sunburst`,
`web` — and only **6** overlap with it (`calm`, `hidden`, `immobilized`,
`prone`, `stunned`, `webbed`). A consumer that cleared every status on each tag
would wipe those 17 on the next creature update.

Ten of the 23 carry a duration and 13 are message-cleared. **Do not port the
ten durations as facts.**

> **AUTHOR, 2026-09-19:** *"some of those are good and some are bad. for example,
> entangled is from tangle weed, but the time I believe actually comes from the
> critical it does."*

Correct, and the table half-admits it: every timed entry is commented
**"typical"**, and the block's own note says where the real number lives —
*"`roundtime` is set with an explicit duration by the combat processor (critical
tables report it in seconds); these defaults apply when a source supplies no
duration of its own."*

So these are **fallbacks for when no better source spoke**, not durations. The
better source is the crit table, and Cena already has it: `crit_tables.tsv`
carries `roundtime` and `stunned` columns per entry, typed as
`CritEntry::roundtime` and `CritEntry::stunned` (rounds, with `STUN_UNKNOWN` for
the one entry that says "stunned" without saying how long).

The rule for Cena, therefore: **a crit-derived duration comes from the crit
entry; a spell-derived one from the spell; the constant is the last resort.** A
consumer that took `entangle => 10` as truth would expire a Tangleweed early or
late depending on the crit that landed, and the wire never said 10.

#### Absent means different things in the two tables

`crtr_flag?` (`:680`) answers `false` for an unseen classification, because *"live XML
flags are always-sent booleans"* — but `crtr_flags?` (`:693`) exists separately to
distinguish *"the feed said not hostile"* from *"the feed has said nothing"*,
and their example is exact: *"A ridden mount carries a bold creature link but
never a `<crtrStatus>` of its own."* That is `StatusInfo::is_known`'s
distinction (`status.rs:88`) arrived at independently, which is some evidence it
is the right shape.

**All of this is CONSUMER knowledge.** None of it belongs in `cena-protocol`,
which is why the parser needed no change — but it is due in full when the
creature consumer is built, and it is not rediscoverable from traffic.

## 2b.2 PSM lists come in TWO header forms, and Lich reads only one

The `<psm> list all` commands (`cman`, `feat`, `armor`, `shield`, `weapon`,
`ascension`) are six of the fifteen `Infomon.sync` issues. VERIFIED against the
archive that the game emits **two** different block headers for the same data:

```text
<Name>, the following Combat Maneuvers are available:     <- CMAN LIST
<Name>, your Combat Maneuvers are as follows:             <- CMAN INFO
```

MEASURED in `GSIV-Nisugi/2025/03/xml/2025-03-20_06-02-48.xml`: one `available:`
header and **four** `as follows:` headers (two Combat Maneuvers, two Ascension
Abilities). Both are followed by the identical `Skill / Mnemonic / Ranks / Type
/ Category / Subcategory` table.

**Lich matches only the first.** `infomon/parser.rb:29`'s `PSMStart` requires
`the following ... are available:`, and `grep -c "as follows"` over the whole
file returns **0**. So `CMAN INFO` output is silently dropped: the rows that
follow would parse fine under `PSM` (`:30`), but `@psm_cat` is never set and no
accumulator is opened, so nothing is written.

### The terminators differ too

```text
   Subcategory: all                          <- LIST, 37 lines after the header
Available Ascension Abilities Points: 21     <- INFO, 15 lines after
```

`PSMEnd` (`:31`) is the literal `/^   Subcategory: all$/`, three leading spaces.
The `INFO` form has no such trailer at all.

**The suspected mutex wedge is UNPROVEN, and worth stating as such.** The
reasoning runs: a block that opens on a header and commits on `PSMEnd` would,
given a header with no trailer, hold `Infomon.mutex` forever. But the `INFO`
header does not match `PSMStart`, so no block opens and nothing wedges. It
would only bite if an **`ASCENSION LIST`** (the `available:` form, which does
open a block) also lacked the trailer -- and MEASURED: across the five archive
files carrying PSM output, `the following Ascension Abilities are available:`
appears **zero** times. We have never captured one. Recorded as a hazard to
check before porting, not as a defect.

### The mnemonic column IS the table's `short_name`

This settles a question the port had open. Lich writes PSM keys from **the
game's mnemonic column** (`parser.rb:362`, `match[:command]`), never checking it
against its own table, while the learn/unlearn path writes
`PSMS.find_name(...)[:short_name]` and `PSMS.assess` reads the same. Three
writers, and a suspicion that they disagree.

MEASURED across the five files: **27 distinct Combat Maneuver mnemonics, 27/27
matching `cman.rb`'s `:short_name` exactly** (`acrobatsleap`, `cmovement`,
`exsanguinate`, `sidebyside`, `unarmedspec`, `vaultkick`, and 21 more). Eight
Ascension mnemonics likewise match `ascension.rb`, including the two irregular
ones -- `trandest` for `transcend_destiny` and `slblessings` for
`spiritual_lore_blessings`.

A suspected `cman.predator` vs `cman.predatorseye` collision for Predator's Eye
is **REFUTED**: Lich's own spec fixture
(`spec/lib/gemstone/infomon_spec.rb:394`) is a verbatim capture reading
`Predator's Eye       predator        3/3   Martial Stance`, so all three
writers produce `cman.predator`.

Worth keeping: the `Skill` column is truncated to 20 characters on the wire
(`Spiritual Lore - Ble`) while `Mnemonic` is not, which supports the reading
that the three 15-character short names (`resistdisintegr`, `resistdisruptio`,
`twohandedweapon`) are a game-side column-width limit rather than typos.

## 2c. The `skill` table prints skills and spell circles in one shape

MEASURED in `dev/lich-5/.../2026-09-01_20-24-11.xml`, and it settles an
ambiguity Lich resolves only by match order.

Both look like `  <name>....|  <numbers>`:

```text
  Two Weapon Combat..................|     312     212
  Perception.........................|     302     202
  Minor Spiritual....................|      40
  Ranger.............................|     162
```

**The discriminator is the field count**: a skill line carries **two** numbers
(bonus, then ranks); a spell-circle line carries **one** (rank). Lich's `Skill`
regex requires both and `SpellRanks` requires one, and `parser.rb` tries `Skill`
first (`:303` before `:312`) — so the order is load-bearing and undocumented.
Cena should key on the count, which is the actual fact.

### Bold marks an enhancive here too

The same signal as the `info` stat table (§2a.4b's sibling finding):

```text
  Two Weapon Combat..................|   <pushBold/>  312<popBold/>     <pushBold/>212<popBold/>
  Armor Use..........................|     302     202
```

So the reassembly rule applies to this block as well: a bolded row is several
frames with one `ends_line`, and `bold_depth` says which numbers are enhanced
without a regex.

### The 46 skill names are only written down in one place

`attributes/skills.rb:39` has the 46 **symbols**; the 46 **display names** the
wire prints exist only in `attributes/enhancive.rb:42-89`'s `SKILL_NAME_MAP`.
EXTRACTED and cross-checked: the two agree exactly, 46 for 46, no drift either
way. Against the wire, 27 of 29 names matched character-for-character — the
other two were the spell circles above, and the 19 unmatched table entries are
skills this Ranger has never trained.

Worth stating because the spellings are not mechanical: `Two-Handed Weapons` is
hyphenated while `Multi Opponent Combat` is not, and the lores use ` - ` as a
separator. Transcribing them by hand would have introduced errors that only a
live `skill` command would reveal.

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

> **Added 2026-09-18 from the wiki audit.** Three questions its skeptic could not settle, each
> scoped to a 12-file sample of one character on one instance over two days -- **not** to the
> corpus. Ask the author before searching: the archive is 49.55 GB and a recursive grep does
> not finish.
>
> **PARTLY ANSWERED 2026-09-18** by an author-supplied capture with every stream window
> turned on: `C:/Gemstone/lich-5/logs/GSIV-Nisugi/2026/09/2026-09-18_15-49-13.xml`,
> 3.3 MB, replayed through `Parser` -> **89,847 frames, 0 unknown tags.**
>
> * **0a is SETTLED, on 2026 traffic.** Every one of 722 `<component id='room objs'>` bodies
>   begins `"  You also see"` -- the complete line, never a fragment. 183 empty
>   `<component id='room players'></component>` mean an empty roster. `component` is **not**
>   a delta, confirming §2.7 against the recent data the audit lacked.
> * **0b is still open, and now better scoped.** Zero `ifClosed='<window>'` and zero
>   `styleIfClosed` in the whole capture. But only **four** streams ever carried text --
>   `inv` (315 pushes), `room` (47), `reserve` (2) and `Spells` (132 paired) -- so `thoughts`,
>   `speech`, `logons` and `death` never opened. **The windows being on is not enough; the
>   events have to happen.** Routed remains the one documented behaviour with no live evidence.
> * **0c is partly answered:** `<objective>` appears once; `group`, `nomenu`, `monopolize`,
>   `flag` and `pushInputState` are all zero. Consistent with the audit's sample.
>
> Two streams the wiki's list (`:55-71`) does not name, both declared with real attributes:
> `reserve` ("Reserved Items", `ifClosed=''`) and the `*_SFHELP` family
> (`spell_SFHELP`, `character sheet_SFHELP`, `save='true'`, no `ifClosed`) -- so **contextual
> help opens a stream per topic**. A rosetta table must therefore treat its entries as a
> seeded default, never as a closed set.
>
> The capture also confirmed the paired-`<stream>` fix on the author's own traffic:
> **132 `<stream id="Spells">` rows**, all of which landed in `main` untagged before it.

> **THE TABLE, MEASURED.** A second capture the same day
> (`2026-09-18_15-32-06.xml`) declares **15 streams and all four behaviours**:
>
> | behaviour | streams |
> |---|---|
> | Copy (`ifClosed=''`) | `room`, `inv`, `Spells`, `announcements`, `bounty`, `loot`, `reserve`, `society`, `speech` |
> | Exclusive (`styleIfClosed`) | `thoughts` -> `'thought'`, `familiar` -> `'watching'`, `ambients` -> `''` |
> | Unspecified (neither) | `main`, `logons`, `death` |
> | **Routed** (`ifClosed='<window>'`) | **still none** |
>
> `thoughts`/`'thought'` and `familiar`/`'watching'` match the wiki's own examples at `:76`
> exactly. **Routed remains the one documented behaviour never observed on the wire**, across
> both captures and twelve corpus files.
>
> **A near-miss worth recording, because the wrong answer was already written down.** The
> author saw 20+ `* <name> joins the adventure` lines in VellumFE's Arrivals tab, and none of
> them were in the XML log. The counts looked damning:
>
> ```
> logons    declared=1   pushed=0        room  declared=92  pushed=92
> death     declared=1   pushed=0        inv   declared=2   pushed=393
> thoughts  declared=1   pushed=0
> speech    declared=2   pushed=0
> ```
>
> This document briefly recorded that as evidence the archive is "systematically blind to four
> streams", probably because Lich consumes them before logging. **That was wrong.** The `.log`
> and the `.xml` had simply diverged: the XML sink rolled to a new file at **15:49:13** while
> the `.log` kept appending to the original until **16:59:37**. A death at 16:57:42 was in the
> `.log` and in no XML file because **the XML for that stretch is a different file**, not
> because anything filtered it.
>
> Two lessons, both already this project's recurring failure:
>
> 1. **Check the file boundaries before theorising about content.** `ls -la` and the last
>    `prompt time=` would have settled it in one command; instead a mechanism was invented for
>    an artifact.
> 2. `declared=N pushed=0` on its own means only *"this capture window saw no traffic for that
>    stream"*. It is not evidence about the protocol. The four streams above are the ones that
>    fire rarely, so they are exactly the ones a short or mistimed capture will miss.
>
> **BUT the underlying observation survives the correction.** Re-checked against the file that
> genuinely covers the death (`2026-09-18_16-56-01.xml`, 16:56-17:00, death at 16:57:42): the
> lines are **absent**, and `death`/`logons`/`thoughts`/`speech` are not even declared in it.
> The author saw them in VellumFE at that moment. So content IS reaching the frontend and not
> these logs.
>
> **This is a property of Lich's logging, not of the protocol**, and it is out of scope for
> Cena, which is not proxying through Lich. It is recorded for one reason only: the corpus in
> `E:\Gemstone\data\log archive` was produced the same way, so **a zero there is weak
> evidence for anything on these four streams.** This document has already treated "0
> occurrences in the corpus" as proof of absence more than once.

> 0a. **Is `component` ever a true delta?** Every body sampled carried the complete line, which
>     is why §2.7 says `GameState`'s overwrite is correct. A `component` that *appends* would
>     flip that from a documentation note into a live bug. **The single assumption here most
>     worth re-testing**, ideally against a 2026 file rather than the 2024-11 sample.
>
> 0b. **Does `ifClosed='<window>'` (the Routed behaviour) ever occur?** Twelve files, zero
>     instances -- the one of the wiki's four behaviours with no live evidence at all. The
>     chaining rule it describes (voln -> thoughts -> main) is therefore unverified.
>     `tools/stream_table.py` is built to answer this from a session with every window open.
>
> 0c. **How often do `<a char=>`, `nomenu`, `monopolize`, `flag`, `menu`/`mi`,
>     `objectives`/`group` and `pushInputState` really appear?** All zero in the sample, all
>     labelled "latent" on that basis. `<objective>` in particular is suspect:
>     `crates/cena-protocol/src/tags.rs` already records 374 occurrences from 2026-06 onward,
>     a period the sampled files cannot see.

1. **The 34 undocumented tags.** For each, is it real wire traffic, a Vellum invention, or a
   third-party injection? The corpus (`E:\Gemstone\data\log archive`, 10,849 `.xml`, 49.55 GB)
   answers this directly — and note the trap already found there: `<vellumImg>` is injected by
   VellumFE and **never sent by the game**
   (`reference/VellumFE/src/core/inline_image.rs:5`). At least one Vellum invention is already
   known to exist, so the others are not safe to assume.
2. **Newline suppression** (§2.1) — unimplemented. Needs a golden that would go red without it.
3. ~~**`<d>` inside `<a>`, outer wins** (§2.4) — unimplemented.~~ **DONE -- it always was.**
   VERIFIED by execution 2026-09-18; see §2.4. The item was never true.
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

---

## 2b. MEASURED: what the login burst does and does not re-send

**Measured 2026-09-18 across all seven of Cena's own logins** (`E:\Gemstone\data\cena_logs`),
counting tags before the first client command. **Unanimous, 7/7.** Each was a **cold login** after
an unclean disconnect -- see the caveat below, which is what makes these recordings the right ones
for Milestone 2 and the wrong ones for a linkdead resume.

| Fact | In the login burst? | Where it actually arrives |
|---|---|---|
| Room **description** (`compDef id='room desc'`) | **YES**, 1 | the burst |
| Vitals (`progressBar` health/mana/stamina/spirit) | **YES**, 10 | the burst |
| `playerID` | **YES**, 1 | the burst |
| Inventory (`<inv>`) | **YES**, 7 | the burst |
| Window/layout definitions (`streamWindow`, `openDialog`, `cmdButton`) | **YES**, ~50 | the burst |
| **Room id (`<nav rm=>`)** | **NO** | after the first command |
| **Hands (`<left>`/`<right>`)** | **NO** | after the first command |
| **Roundtime (`<roundTime>`)** | **NO** | only when one is incurred |
| **Status indicators (`<indicator>`)** | **NO** | after the first command |
| **Effect dialogs** (Active Spells, Buffs, Debuffs, Cooldowns) | **NO** | after the first command |

### CAVEAT: how those seven sessions ENDED, and what it does not prove

> **AUTHOR, 2026-09-18:** *"but how did the program end for you those times? was it a quit or like
> an altf4 disconnect?"*

Checked, and the answer changes what the table above can be claimed to show.

**All seven ended with `lifecycle Closed`** -- the binary's own `session_cancel.cancel()` at the
end of `main`. And VERIFIED that **Cena never sends `quit`**: `grep` finds no `quit` anywhere in
`crates/cena/src/main.rs` or the session actor, and the last client line in every log is a `look`
or a `search`. The socket simply closed.

So from the **game's** side every one of these was an abrupt drop -- much closer to an alt-F4 than
to a clean logout. Which cuts two ways:

- **It strengthens the reconnect relevance.** These *are* recordings of what the game sends after
  an unclean disconnect, which is exactly Milestone 2's case. It is not the quiet-logout path.
- **It weakens any claim about a LINKDEAD resume.** Every gap between runs was 102-4153 seconds
  (measured), far past any grace window, so each login was a **fresh character session**. Nothing
  here shows what the game sends when reconnecting *inside* a linkdead window, where the character
  is still in the world and the server may restore more.

**UNVERIFIED and worth knowing before the reconnect ladder is tuned:** whether a reconnect within
the linkdead grace period re-sends more than a cold login does. If it does, the invalidation set
is smaller on a fast reconnect than the table above implies -- and the honest default is to treat
every reconnect as cold until measured, because assuming a resume is the direction that produces
stale beliefs.

### Why this settles `plan/12` §5.2's invalidation contract

§5.2 classifies facts as **Invalidated / Retained / Suspect** on reconnect and says the invalidated
ones become `Unknown` "until re-observed". That was a design assertion; it is now a **measurement**,
and the measurement matches it closely:

- The facts §5.2 calls **Invalidated** -- roundtime, current room, hands -- are exactly the ones the
  login burst does **not** carry. A reconnected session genuinely does not know them, and
  `Unknown` is the only honest value.
- The facts the burst **does** re-send arrive unprompted, so they self-heal without a re-sync.

**The distinction is not tier or importance -- it is whether the fact is PUSHED at login or PULLED
by asking.** Everything in the burst is pushed. Everything absent from it is a response to a
command, and stays unknown until something asks.

### Why Vellum keeps state on disconnect and Cena must not

VERIFIED in `reference/VellumFE/src/frontend/gui/app/server_pump.rs:392`: on
`ServerMessage::Disconnected`, Vellum sets `connected = false`, clears pending launches, and
re-renders. **It does not invalidate room, vitals or effects.**

That is correct for Vellum and wrong for Cena, and the reason is architectural rather than a
difference of opinion:

> **AUTHOR, 2026-09-18:** *"cena is the full shebang. it's the one controlling everything so it's
> not vellum disconnecting from lich, etc. It would be a full reconnect and the game would send us
> the connection stuff."*

Vellum disconnects from **Lich**, which stays logged in and holds the game session. The character
never left the world, so Vellum's state is stale only in the sense that it stopped receiving
updates -- reconnecting to the same live session makes it current again.

Cena **is** the login. A disconnect ends the game session, and reconnecting is a fresh
`K/A/M/F/G/P/C/L` producing a new character session. Anything the new session does not re-send is
not stale, it is **unobserved**.

Lich, which is also the full client, does invalidate: `Inventory.reset!` exists specifically for
"a session reset / reconnect" and drops snapshots and mirrored containers so that "no stale
container mirror survives" (`reference/lich-5/lib/common/inventory.rb:1014-1045`).

### The parser hazard Lich names explicitly

`reference/lich-5/lib/games.rb:432`:

```ruby
# strip_xml's multiline carry is a process-global; clear it here so a
# fragment left open before a reconnect/session reset does not bleed
# into the next session.
$strip_xml_multiline = {}
```

**Cena has exactly this buffer**: `Parser::pending` (`crates/cena-protocol/src/parser/read.rs`),
which holds an unterminated trailing fragment until its `
` arrives. A connection dropped
mid-tag leaves bytes there, and the next generation's first read would be concatenated onto them
-- producing one corrupt frame at the start of every reconnect.

Nothing clears it across generations today. **A reconnect must reset the parser**, and this is the
kind of defect that appears only on a real mid-tag disconnect, which is to say rarely and
confusingly.
