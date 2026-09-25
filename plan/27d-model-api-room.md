# 27d — Room, creatures and what is hiding

The room you are in, who is in it, what left, and what is there that you cannot
see.

Read [`27`](27-model-api.md) first. This is `GameState::room`,
`GameState::creatures`, `GameState::overwatch` and `GameState::targeting`.

---

## The room — `state/room.rs`

| Field | Wire source | Notes |
|---|---|---|
| `id` | `Frame::RoomId` (`<nav rm=>`) | **`Option<String>`, not `String`.** A bare `<nav/>` means *"arrived, and this room has no UID"*. It was `String` via `unwrap_or_default()`, producing `id: ""` — an empty string a consumer cannot tell from a UID (review MO-10) |
| `title`, `description` | `Component{room desc}`, `StreamWindow` subtitle | Description is a parsed `Runs` body, not raw XML. Vellum stores the inner XML verbatim (`parser.rs:803`); `Runs` is the fix |
| `exits` | `Frame::Compass` | `Some(vec![])` when the game said "no exits" — **`Some`, even when empty**: the server SAID so |
| `creatures`, `objects`, `players` | `Component{room objs/players}` | `None` = unobserved, empty = observed empty |
| `meta` | `Frame::RoomMeta` | |

**Criterion 2** (`plan/12:537`) is *"renders a room description and prompt from
typed frames, never raw text"*. The failure it rules out is a consumer scanning
display text for `"Obvious exits:"`. Exits come from `Compass::directions`; the
id from `RoomId`. Both are stated outright by the wire and neither is
recoverable from prose without guessing.

> **A room id can collide.** `XMLData.room_id` is an **MD5 of the room's text**
> when the room has no UID (`fog.rb:230`), so two unmapped rooms that read the
> same share an id. `GameState::arrivals` exists for that: a counter that says
> *did we actually move*, which `fog::moved_from` consumes.

## Creatures — `state/creatures.rs`

| Field | What it holds |
|---|---|
| `instances` | Everything known about each creature, by `exist` id |
| `roster`, `previous_roster` | Who is in the room now, and who was |
| `pending_status`, `pending_links` | A `<crtrStatus>` or link that arrived before its partner |
| `death_watch` | Creatures an event touched, watched until a room refresh flags them dead |
| `seen_to_leave` | Who left, and which way — the third hiding condition |

### A creature joins the roster only when two things agree

A bolded `room objs` link **and** a `<crtrStatus>` vouching for it. Bold is the
wire's own mark for a creature (`room.rs:207`); an unbolded `<a exist>` is an
item or a player.

### Status — `creature/status.rs`

**22 classifications**: `Immobilized`, `Webbed`, `Sleeping`, `Disoriented`,
`Stunned`, `Rooted`, `Calm`, `Kneeling`, `Prone`, `Sitting`, `Flying`,
`Hovering`, `Hidden`, `Hostile`, `Disengaged`, `Dead`, `Sympathetic`,
`Ascended`, `Inferior`, `Challenging`, `Rider`, `Mount`.

> **`<crtrStatus dead="1">` is the authority on death**, and this is a
> correction the author made:
>
> > *"the earliest signal is crtrStatus which will have the death=1 tag at the
> > start of the blob and the death line is usually towards the end."*
>
> I had measured "44 death lines, 0 carrying a `<crtrStatus>` on the same
> **line**" and concluded prose was the earliest death signal. The measurement
> was about lines and the claim was about blobs, which is not the same
> question. **A number measured at one granularity is not evidence for a
> conclusion at another.**

## Departures — `state/departure.rs`

**Read off the markup, not matched against prose**, and that was the author's
correction to a design in progress:

> *"the room would be giving a link with the id"*, and *"leaving a room
> indicates a direction in the `<d>`. arriving in the room does not. hiding in
> the room does not."*

MEASURED over eight combat logs, counting lines that carry a creature link:

| shape | lines | carry a direction `<d>` |
|---|---|---|
| departure | 195 | **195** |
| arrival | 142 | **0** |
| hide | 38 | **0** |

So `classify` requires a **bolded** creature link *and* a `<d>` whose text is
one of the eleven directions — the list is Lich's (`creature.rb:858`) and
includes `out`, which is not a compass point and whose absence there once made
every such line unmatchable.

Both facts are in the markup: the creature from its `<a exist=>` id (twice — the
pronoun is a link too), the direction from a bare `<d>`. **No bestiary lookup,
no placeholder expansion**, no ambiguity between two creatures of the same name,
and a creature whose flee line nobody recorded is handled as well as one in the
table.

> The bold requirement was **found by mutation**: dropping it left every test
> green, because no test had an unbolded `exist` link beside a direction. That
> combination is real — a dropped item, a room object, a player.

## The hiding inference — `vanished_unaccounted`

The author's rule, all three conditions:

> *"but gone just means not in the room, doesn't mean hid. We have creature
> arrival and leaving messaging though."*

**Gone, not dead, and not seen to leave.** A two-condition version was written,
found wrong and deleted — a creature that simply walked out satisfies both.

**Still evidence rather than proof.** A creature can leave by a route nothing
recorded, and this reports it as unaccounted for; a consumer should treat the
answer as *"worth looking"* rather than *"it is certainly hiding"*.

## Targeting — `state/targeting.rs`

What the game says you can attack: the `combat` dialog's `dDBTarget` list.

`parse_ids()` splits `content_value` on commas per `xmlparser.rb:776`.
`is_targetable()` returns **`Option<bool>`** — an empty list the game HAS stated
is different from never having been told (`plan/12` §5.2), which is what the
private `stated` flag is for.

> **AUTHOR:** *"if you look at lich dDBTarget is part of the equation how it
> determines an npc is hostile or not, before the hostile flag of course."*

`GameState::hidden_targets()` is the other half: something the game says you can
attack, that is not in the room you can see.

## Overwatch — `state/overwatch.rs`

Where something was last seen hiding. Prose-driven, and it pairs with
`vanished_unaccounted` rather than duplicating it: **the inference lives on the
roster**, which owns the facts it reasons over (Rule 2.2a).

> An earlier doc here linked `Overwatch::vanished`, **a method that was never
> written**. Fixed 2026-09-22 — a dead intra-doc link reads as a promise the
> code does not keep.

## Creature messages — `state/creature_message.rs`

The bestiary's 7,125 lines of twelve kinds, joined onto every creature by
`Creature::messages_of`: 1,972 attack, 1,094 death, 1,067 flee, 991 arrival,
710 decay, 559 description, 281 spell prep, 162 trigger, 113 stun break,
101 search, 68 stand, 7 ambient. A match of an attack or a trigger also names
which attack, or which effect (`bind`, `web`) is coming.

> **CORRECTED 2026-09-24.** This read *"The bestiary's 3,863 lines: 1,094
> death, 1,067 flee, 991 arrival, 710 decay"*. That was the whole table because
> the first port dropped the other eight kinds at extraction, with attack
> messaging dropped on the grounds that nothing read it. The author: *"The
> reason it was all there was multiple reasons, one of which is a
> comprehensive beastiary, the other is the messages have uses just because
> they haven't been made apparent yet. For example when get to implementing
> kswole's behaviors that requires their casting prep line."* The extractor
> now keeps everything and aborts on a key it does not read
> (`crates/cena-model/tools/extract_creatures.rb`). The counts below are the
> first port's, kept as its record.

**NOT WIRED, and deliberately named as such.** Nothing calls it. The
arrival/departure job it was built for belongs to `departure.rs` (the table
above), and `<crtrStatus dead="1">` is the authority on death. What survives is
recognising death and decay **prose as prose** — which a log reader, transcript
or highlight rule wants, and which nothing else can do.

Wiring it into `Creatures` now would mean choosing between it and `<crtrStatus>`
as the authority on one creature's death, with no caller to justify the
decision. Rule −1: it stays a tested classifier with a stated purpose.

> **1,164 of the 3,863 lines carry a placeholder**, so two thirds of flee lines
> cannot be compared as text. `{target}` is **absent from Lich's
> `PLACEHOLDER_MAP`**, so its 24 lines are unmatchable there too; here both it
> and `{weapon}` are wildcards, a deliberate divergence recorded rather than
> hidden.
>
> The sweep test renders every templated line with every form of every
> placeholder it carries — 16,342 renderings. **550 failed on the first run and
> the renderer was wrong, not the matcher**: it substituted one word for every
> placeholder, producing *"lumbers north, ... each of north strides"*.

## Reconnect

| | |
|---|---|
| `room` | **contents cleared** — who is standing there is not durable |
| `arrivals` | **kept** — a session counter; resetting it would fake a "did not move" |
| `creatures` | **partly** — what combat did to a creature is still true of it; who is in the room is not |
| `overwatch`, `targeting` | **cleared** |
