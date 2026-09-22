# 27f — Streams, windows and messages

Where the game's text goes, what happens to a stream whose window is closed,
and who said what.

Read [`27`](27-model-api.md) first. This is `GameState::streams`,
`stream_windows`, `messages`, and the chunk machinery under them.

---

## The routing rule: read it off the frame

`TextFrame::stream` is stamped by the parser on **every run**, so there is no
`current_stream` cursor in the model to keep in sync.

> `VellumFE` carries one and has to fix it up on pop, on resume and on prompt —
> three places that can disagree, and its pop sets `"main"` unconditionally
> before `StreamResume` corrects it. **Reading it off the frame is the same
> routing with the state removed.**

So `StreamPush`, `StreamPop`, `StreamResume` and the prompt barrier need **no
handling in the model at all**. They are still published, for a renderer that
wants to know a window opened.

## What clears a buffer: `clearStream`, and nothing else

MEASURED over 16 files across 4 characters: of **2,722** `pushStream` tags,
**2,721** are immediately preceded by a `clearStream` of the same id. The wire
idiom is `streamWindow` → `clearStream` → `pushStream` → content → `popStream`.

**The single exception is the one that matters**: a `thoughts` push with no
clear. ESP accumulates over a session, so clearing on push would throw away
every thought but the newest.

Honouring the wire's own clear gets snapshot behaviour for `room` and `inv`
*for free* and accumulation for `thoughts` **without a per-stream exception
list**. Vellum needs one: its `perception` buffer is *"NOT cleared on
pushStream"* and its `sprite` component is exempt from an unchanged-check
because *"the game sends it EMPTY on every room change"*.

`MAX_STREAM_LINES = 2_000` — a scrollback depth, not a protocol fact, and fixed
rather than configurable per Rule −1.

## Closed-window routing — `state/stream_windows.rs`

**This is the newest piece and it started with a duplicate the author pasted:**

```text
<pushStream id="speech"/><preset id='speech'>You <a ...>say</a></preset>, "Yep."
<popStream/>
<preset id='speech'>You <a ...>say</a></preset>, "Yep."
<prompt time="1790052928">&gt;</prompt>
```

The same sentence twice. **Not a defect in the game** — `speech` is declared
`ifClosed=''`, and the wiki says what that means
(`reference/wiki_clean/Wrayth protocol.txt:78`): *"The stream is a duplicate —
the server also sends the same line to main, so the closed window's copy drops
harmlessly."* Nothing in Cena read the attribute.

Four behaviours, MEASURED over 208 logs / **157,220 `<streamWindow>` tags**:

| behaviour | declaration | streams |
|---|---|---|
| `Drop` | `ifClosed=''` | `room` 77,411 · `society` 1,087 · **`speech` 201** · `inv` 162 · `Spells` · `loot` · `reserve` · `announcements` · `bounty` · `charprofile` |
| `Styled` | `styleIfClosed='x'`, no `ifClosed` | `thoughts` → *thought* · `familiar` → *watching* |
| `Main` | neither | `main` 77,497 · `death` 86 · `logons` 86 |
| `Route` | `ifClosed='<window>'` | **none** — UNVERIFIED, built from the wiki |

> **`ifClosed=''` and an absent `ifClosed` are opposite** — drop the text versus
> show it in main — so the module lives on telling them apart. `Attrs` is
> `Vec<(String, String)>`, so empty survives as `Some("")` and absent is `None`.
> Collapsing them with `unwrap_or_default()` would drop every `death` and
> `logons` line; **mutation-verified: that change kills 7 tests.**

`ambients` inverts the usual shape — `ifClosed` absent, `styleIfClosed`
**present and empty** — which the wiki's table does not name. Read as `Main`:
an empty style is no style.

**`Route` chains**, so a cycle among closed windows hangs without a visited set.
Mutation-verified: the mutant was killed by timeout at exit 143.

### Whose job is what

Whether a window is **open** is a frontend fact, so it is a parameter rather
than state. The model reads the declaration; the viewer applies it.

That split is forced by the transport: the web hub encodes **one** message and
broadcasts it to every viewer, so a resolved destination would mean re-encoding
per client. `StoryLine.closed` carries the declaration instead
(`cena-ui/src/view.rs`), and `placeLine()` in the browser decides.

## Two censuses, corrected in opposite directions

**`charprofile` — the census OVER-claimed.** It was listed among pushed ids, so
a profile reader was built on that buffer. MEASURED: **2 `exposeStream`, 0
`pushStream`.** All 11 tests failed on an empty buffer.

**`speech` — the census UNDER-claimed.** The author's live paste carries
`<pushStream id="speech"/>`; MEASURED over the same 208 logs, that string
appears **0 times** while `preset id='speech'` appears **203**. The corpus runs
2026-09-01→12 and the paste is 09-22, so the push is newer than those logs or
depends on a client setting they do not carry.

> **A census describes the traffic it read, and traffic changes.** A wider run
> finds **8** pushed ids, not 6: `room` 77,575 · `inv` 33,378 · `society` 1,087
> · `reserve` 91 · `bounty` 86 · `thoughts` 12 · `ambients` 4 ·
> `announcements` 2. **Neither list is closed**, and what handles that is the
> routing rule rather than any list: the stream id is read off the frame, so a
> push of an uncensused id needs no code change.

## Messages — `state/message.rs`

**Not a port.** MEASURED: Lich has no speech classifier.

`Channel` is `Speech` or `Whisper`, decided by the `<preset id=>` the wire
already sends. A line with neither preset is **not a message** — treating an
unknown preset as speech would invent one.

- **`speaker` is `None` for your own speech**, which the game writes without a
  link: `<preset id='speech'>You say</preset>, "..."`. MEASURED: 16 of 206.
- **`You recite arcane incantations`** and its kin are *spell-casting emotes*,
  not speech.
- MEASURED over 208 live logs: **203 `speech` and 3 `whisper`**. A first pass
  counted *"192, all speech"* — wrong, because the pattern missed a form.

**Reconnect: kept.** A cooldown is a claim about *now* and stops being true
while we are gone; **a message is a record that someone said something at a time
when we were listening, and that stays true forever.**

## Chunks — `state/chunks.rs`

The prompt-to-prompt blob. `route_text` holds runs until `ends_line` says a
line is finished, because **a frame boundary is not a line boundary**: the
parser emits one run per markup boundary, so `  a` + `<a>pebbled grey leather
doublet</a>` is two frames of one line.

The chunk is what the combat tracker classifies ([`27c`](27c-model-api-combat.md))
and what the player log holds until it can tag its lines.

## Afflictions — `state/afflictions.rs`

**Six text-only statuses**, with Lich's patterns from `parser.rb:87-102`:
`Bound`, `Calmed`, `Cutthroat`, `Silenced`, `Sleeping`, `Thorned`.

**No indicator exists for any of them**, which is why they are cleared on
reconnect while the ten indicator-derived statuses are kept: nothing would ever
correct a stale one.

Mid-line cutthroat is checked first — Lich's one unanchored pattern.

## Unknown tags — `state/unknown.rs`

Criterion 8: *"unknown tags survive to display rather than panicking"*. An
unknown tag is **recorded into the state**, not merely not-crashed-on, so a test
can assert it reached something a user would see. *"It did not panic"* is not
Rule 2.2's *"as text and a log"*.

**Reconnect: kept.** A tag the parser could not model is a fact about the
**protocol**, most useful across a reconnect rather than least.

MEASURED: **126 tags** in `tags.rs`, and **zero unknown or malformed frames**
across the whole golden corpus.
