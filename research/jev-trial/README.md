# Jev trial — Job 1: which way does this exit go?

**Run 2026-09-21.** Brief: [`plan/22-jev-trial.md`](../../plan/22-jev-trial.md).
This answers one question — is Jev good enough at classifying exit direction to be worth
wiring in — and nothing here changes the map, the converter or Hydra.

**Recommendation: use it, above a stated probability of 0.99.** On the full 553-item
test that gate is right **247 times in 250 (98.8%)** and covers 45% of exits; restricted
to exits already known to be vertical it is **167/167** (VERIFIED, §10–§11). Below the
gate, send to the author for review.

Six prompts were run, over two answer keys: **300 exits known to be vertical** and
**253 known to be flat** (§11). Which prompt to use depends on what a rule has already
established — see §5. §7–§11 cover everything tried, including four ideas measured and
dropped.

**The single most useful finding** is not a score: the author's definition of "same
floor" — a bridge keeps you on your plane even though it is raised — is not what a model
assumes, and stating it in one paragraph moved overall accuracy 5.7 points (§11).

Jobs 2 and 3 were not attempted.

---

## 1. The API — VERIFIED by calling it

The brief's endpoint was right; its request and response description was not. Corrections:

| `plan/22` says | Actually (VERIFIED) |
|---|---|
| `POST https://openrouter.ai/api/alpha/decisions` | correct |
| model `typesafe/jev-1.13` | correct; server reports `typesafe/jev-1.13-20260917` |
| "returns one of them **with a probability**" | returns `choice`, a **`confidence`**, *and* `probabilities` across every option. Confidence is a second, separate axis |
| choices are a set/list | `criteria` is an **object**: option name → description of when it applies |
| cost must be estimated from the price sheet | `usage.cost` is returned **per call** |

`state` accepts a structured JSON object, not only a string, so both rooms are sent as
named fields rather than concatenated prose.

Source: OpenRouter's OpenAPI spec, fetched raw rather than through a summarising fetcher
(`curl -s https://openrouter.ai/docs/api/api-reference/alphadecisions/submit-a-decisions-questions-and-answers-request.md`).
Two summarised fetches of the marketing pages returned no schema at all; the raw
`llms.txt` docs indexes for both openrouter.ai and docs.typesafe.ai did.

### One exact request

Item 1 of the key, `go oak tree`, room 228 → 8665 — Town Square Central up into the
treehouse. Only the two `description` strings are abridged, with `…`, for width;
everything else is verbatim. Key redacted.

**This is the v1 request**, shown because it is the first call made and the brief asks
for one exact pair. The recommended v4 request differs: no `exits_line`, two options
instead of three, and a `known_elevation` field on ground-level rooms. See
[`ask_v4.py`](ask_v4.py).

```http
POST https://openrouter.ai/api/alpha/decisions
Authorization: Bearer <OPENROUTER_API_KEY>
Content-Type: application/json
```
```json
{
  "model": "typesafe/jev-1.13",
  "state": {
    "command_typed": "go oak tree",
    "room_departed": {
      "title": ["[Town Square Central]"],
      "description": ["This is the heart of the main square of Wehnimer's Landing.  The impromptu shops …"],
      "exits_line": ["Obvious paths: northeast, east, southeast, southwest, west, northwest"],
      "area": "the town of Wehnimer's Landing"
    },
    "room_arrived": {
      "title": ["[Wehnimer's, Treehouse]"],
      "description": ["This sturdy wooden platform, built of planks and wedged into a fork of the oak's …"],
      "exits_line": ["Obvious paths: down"],
      "area": "the town of Wehnimer's Landing"
    }
  },
  "questions": {
    "direction": {
      "type": "choice",
      "instructions": "A character in room_departed types command_typed and arrives in room_arrived. Relative to where they started, did they go up, go down, or stay on the same floor?",
      "criteria": {
        "up": "Taking this exit moves the character to a higher floor, level or elevation than the room they started in.",
        "down": "Taking this exit moves the character to a lower floor, level or elevation than the room they started in.",
        "same floor": "Taking this exit keeps the character at the same elevation. It leads sideways, not up or down."
      }
    }
  }
}
```

### Its exact response

Verbatim from the cache, field order as returned:

```json
{
  "model": "typesafe/jev-1.13-20260917",
  "answers": {
    "direction": {
      "type": "choice",
      "choice": "up",
      "probabilities": { "up": 1, "same floor": 0, "down": 0 },
      "confidence": 1
    }
  },
  "usage": { "input_tokens": 922, "output_tokens": 39, "cost": 3.8724e-05 },
  "id": "gen-dec-1789968870-52hCvUsWlEmIy5tNzLr2",
  "provider": "TypeSafe"
}
```

## 2. The answer key — and a correction to the brief's count

`plan/22` cites **172** such exits (138 up, 34 down). MEASURED here: **300** (228 up,
72 down).

```powershell
python build_keys.py "E:\Cena\reference\mapdb\map-1789942730.json" results\job1_key.jsonl
# rooms=36838 key=300 up=228 down=72
# baseline(always up)=76.0%
```

The difference is the filter. The brief drew 172 from 2,397 exits that *look* vertical.
[`build_keys.py`](build_keys.py) does not require the forward command to look like
anything: any non-compass, non-Ruby exit A→B whose return leg B→A is a bare `up` or
`down` has a known answer, whatever it is called. That admits `go depression`,
`go fog`, `go promontory`, `go canopy-covered shop` — 132 distinct commands, all
genuine movement exits (inspected).

This matters for scoring: the larger key has a **lower** prior, so the bar is
**76.0%**, not the brief's 80%. More items and a harder test.

UNVERIFIED: I did not reproduce the brief's 172, so I cannot say the two are consistent.
I can only say the 300 here are each backed by an explicit `up`/`down` return leg.

## 3. Results — VERIFIED

**This section reports v1, the first prompt**, because it is what the threshold table was
first built from and the analysis in §7 refers back to it. **For the result that stands,
see §10.** The summary of all four runs is the table there.

```powershell
python ask.py   results\job1_key.jsonl results\job1_answers.jsonl
python score.py results\job1_answers.jsonl results\job1_key.jsonl
```
Full output: [`results/job1_score.txt`](results/job1_score.txt).

| | |
|---|---|
| items | 300 |
| **accuracy** | **88.7%** (266/300) |
| baseline, always answering `up` | 76.0% |
| lift over baseline | +12.7 points |

### Accuracy by stated probability — the threshold table

This is the number the brief asks for, and it decides where a proposal can be trusted.

| probability of chosen option | n | correct | accuracy | cumulative at or above |
|---|---|---|---|---|
| 0.00–0.50 | 16 | 6 | 37.5% | 88.7% (n=300) |
| 0.50–0.70 | 36 | 24 | 66.7% | 91.5% (n=284) |
| 0.70–0.80 | 13 | 9 | 69.2% | 95.2% (n=248) |
| 0.80–0.90 | 28 | 26 | 92.9% | 96.6% (n=235) |
| 0.90–0.95 | 24 | 21 | 87.5% | 97.1% (n=207) |
| 0.95–0.99 | 43 | 41 | 95.3% | 98.4% (n=183) |
| **0.99–1.00** | **140** | **139** | **99.3%** | **99.3% (n=140)** |

**The probabilities are calibrated and monotonic apart from one dip** (0.90–0.95 at
87.5%, n=24 — small enough to be noise). A stated probability is usable as a gate,
which is the whole case for a decision model over a chat model.

`confidence` tracks the same shape and is not obviously better; at 0.99–1.00 it gives
99.2% on 125 items versus probability's 99.3% on 140. **Gate on probability** — it
covers more items at the same accuracy.

### Where it goes wrong

| expected → chosen | n |
|---|---|
| up → up | 210 |
| down → down | 56 |
| down → up | 15 |
| up → down | 11 |
| up → same floor | 7 |
| down → same floor | 1 |

**26 of 34 mistakes are polarity inversions** — Jev knows the exit is vertical and picks
the wrong end. It almost never mistakes a vertical exit for a flat one (8 of 300).

**The weak spot is room pairs that share a title** (MEASURED: `climb chimney` between two
rooms both called `[Zaerthu Tunnels, Chimney]`, four `[Castle Darkstone, Guardtower]`
pairs, `[Caligos Isle, Dockside Stairway]`):

| | n | correct | accuracy |
|---|---|---|---|
| the two rooms have different titles | 264 | 239 | 90.5% |
| **the two rooms share a title** | **36** | **27** | **75.0%** |

That is the expected failure. When both ends are named identically and the descriptions
are near-symmetric, nothing in the text says which is above the other, and Jev is
guessing. **INFERRED:** a rule-based pass should claim identical-title pairs before Jev
sees them, or they should always go to review.

### Twenty mistakes with their room text

In [`results/job1_score.txt`](results/job1_score.txt), each with both titles and the
return command, so a person can judge whether the key or the model was wrong. Two worth
naming:

- **`17249 → 17250 'climb ladder'`, p=1.00** — the only confident mistake in the whole
  run, and the one item that keeps the 0.99 band off 100%. `[Citadel, Icehouse]` →
  `[Citadel, Darkroom]`; you climb a ladder and the map says you went *down*. Either the
  map is wrong or this is a genuinely counter-intuitive room. **This one is worth the
  author's eye** — it is the single data point that sets the ceiling of the trusted band.
- **`22105 → 22100 'go staircase'`, p=0.97** — Bard Guild Entry Hall → Landing. "Landing"
  reads as upstairs; the map says down.

`plan/22` notes Lich's text is stale for ~5% of rooms, so some of the 34 are the map's
fault, not Jev's. I did not adjudicate them — that needs someone who knows the rooms.

## 4. Cost — VERIFIED

Summed from `usage.cost` on every response, not estimated.

| | |
|---|---|
| run | items | input tokens | output tokens | cost |
|---|---|---|---|---|
| v1 | 300 | 225,681 | 11,708 | $0.009479 |
| v2 | 300 | 297,381 | 11,704 | $0.012490 |
| v3 | 300 | 301,279 | 11,709 | $0.012654 |
| v4 | 300 | 286,579 | 9,300 | $0.012036 |
| v3 on the 553-item key | 553 | 556,365 | 21,761 | $0.023367 |
| v5 on the 553-item key | 553 | 634,891 | 21,810 | $0.026665 |
| **total** | 2,306 | 2,302,176 | 87,992 | **$0.096691** |

The two 553-item runs reused 300 cached answers between them, so only the 253 new items
were paid for on the first.

v2's longer criteria and guidance cost about 240 extra input tokens per call — 32% more
per item, or $0.0000416 against v1's $0.0000316. Irrelevant at this scale.

v4 is the cheapest per item as well as the most accurate: two options instead of three
means fewer output tokens (9,300 against v3's 11,709).

Extrapolating v4 to all 2,397 vertical-looking exits: **INFERRED ≈ $0.096**. Cost is not
a consideration for this job. Four full runs of the trial cost under five cents.

Latency was not instrumented. UNVERIFIED.

## 5. Recommendation

**Use Jev for job 1, above a stated probability of 0.99.** Which prompt depends on what
a rule has already established — this is the main thing to get right:

| situation | prompt | evidence |
|---|---|---|
| a rule has **proven the exit is vertical**, only the direction is open | **v4**, two options | **167/167** at p≥0.99, 56% coverage (n=300) |
| the exit **might be flat** — the general case | **v5**, three options | **247/250 (98.8%)** at p≥0.99, 45% coverage (n=553) |

Do not use v4 where flatness is possible: with only `up` and `down` offered, a flat exit
gets a confident wrong answer and no way to say otherwise.

A workable ladder, for whoever wires it in:

1. **Rule-based passes first.** Explicit `up`/`down`, then paired returns, then §8's
   strict ground fill. Jev sees only what they leave.
2. **Establish that the exit is vertical before asking the binary question** (§10). Where
   that is not established, ask the three-option form and treat `same floor` as a real
   answer — noting it is unmeasured.
3. **Do not send the destination's exits line** (§9). It is a distractor, and it inflates
   any measurement taken against a key built from return legs.
4. **p ≥ 0.99** → propose the value.
5. **p < 0.99** → propose, marked for review. 0.95–0.99 is 97.2%: good, not good enough
   to write unreviewed.
6. **Identical-title pairs** → always review, whatever the probability (75% in v1).

**Caveat on the 167/167.** It is one run of 300 items. A perfect score on a 167-item
subset is consistent with a true error rate near 1 in 200, not with infallibility. Treat
0.99 as a well-evidenced gate, not a guarantee, and keep the review path.

### What shape of record the results suggest

`plan/22` asks for this and forbids inventing the format, so this is a suggestion only.
Each proposal wants: the two room ids, the command, the proposed direction, `source:
llm`, the **stated probability**, and the model id as the server reported it
(`typesafe/jev-1.13-20260917`, not the requested `typesafe/jev-1.13` — they differ, and
a re-run after a model update should be distinguishable). The probability is what makes
a diff reviewable in priority order, so it belongs in the file, not just in a log.

## 6. Files

| file | what |
|---|---|
| [`build_keys.py`](build_keys.py) | map JSON → job 1 answer key |
| [`ask.py`](ask.py) | key → Jev → answers, cached (v1 prompt) |
| [`ask_v2.py`](ask_v2.py) | the same, with the v2 criteria and guidance |
| [`ask_v3.py`](ask_v3.py) | v2 minus the exits line, plus the ground flag |
| [`ask_v4.py`](ask_v4.py) | v3 with a two-option question — **use where the exit is known vertical** |
| [`ask_v5.py`](ask_v5.py) | v3 plus the span/plane rule — **use in the general case** |
| [`build_keys2.py`](build_keys2.py) | map → the 253-item FLAT answer key (§11) |
| [`ground.py`](ground.py) | flood fill deriving ground-level rooms (§8) |
| [`score.py`](score.py) | answers + key → the tables above |
| `results/job1_key.jsonl` | 300 items with known answers |
| `results/job1_answers.jsonl` | Jev's answer for each, v1 |
| `results/job1_answers_v2.jsonl` | Jev's answer for each, v2 |
| `results/job1_answers_v3.jsonl` | Jev's answer for each, v3 |
| `results/job1_answers_v4.jsonl` | Jev's answer for each, v4 |
| `results/job1_score.txt` | full score output v1, incl. the 20 mistakes |
| `results/job1_score_v2.txt` | full score output v2 |
| `results/job1_score_v3.txt` | full score output v3 |
| `results/job1_score_v4.txt` | full score output v4 |
| `results/job1_flat_key.jsonl` | 253 exits known to be flat |
| `results/job1_combined_key.jsonl` | the 553-item key: vertical + flat |
| `results/job1_answers_combined.jsonl` | v3 on the 553 items |
| `results/job1_answers_v5.jsonl` | v5 on the 553 items |
| `results/job1_score_combined.txt` | full score output, v3 on 553 |
| `results/job1_score_v5.txt` | full score output, v5 on 553 |
| `results/ground.json` | 2,703 room ids derived as ground level (§8) |
| `results/cache/` | raw responses, keyed by SHA-256 of the request body — **gitignored** |

Re-running `ask.py` costs nothing and re-asks nothing: every answer is cached under a
hash of exactly the request body that produced it. Change the prompt and the hash
changes, so the cache cannot silently serve a stale answer for a new question.

**The key** was read from `$env:OPENROUTER_API_KEY` and is not written to any file, log,
cached request or commit. `research/jev-trial/.env` and `results/cache/` were added to
`.gitignore` before the first call.

## 7. What the model is given, and whether more would help

The question that drove the second run: **are we giving it all the pertinent
information?** The map has 15 distinct room fields. MEASURED:

```powershell
python -c "import json,collections; d=json.load(open(r'E:\Cena\reference\mapdb\map-1789942730.json',encoding='utf-8')); c=collections.Counter(); [c.update(r.keys()) for r in d]; print(c.most_common())"
```

| field | rooms with it | sent? | why |
|---|---|---|---|
| `title`, `description`, `location` | 36,786 / 36,528 / 35,891 | **yes** | all variants of each, in full |
| `paths` (the exits line) | 36,667 | **v1/v2 yes, v3 no** | it leaks the answer, and removing it *improved* results — §9 |
| `wayto` (destination's) | 36,838 | **no — must not** | it *contains the answer*. The key is built from the return leg being `up`/`down`; sending it is handing over the key |
| `image_coords` | 21,838 | **no** | positions on a flat drawing, not elevation — a floor above can be drawn anywhere on the page, or on another image. See below |
| `tags` | 27,486 | no | overwhelmingly `meta:forage-sensed` and herb names. Noise for this job |
| `timeto` | 36,838 | no | 0.2 for essentially every exit in the key. No signal |
| `terrain`, `climate` | 13,893 | no | `plan/22` forbids it, and rightly |

**So we are already sending nearly everything that bears on the question** — and §9
shows we were sending slightly too much. No gain in this trial came from adding map
fields. They came from better instructions (v2) and from *removing* a field (v3).

One field was later **added**: a derived `known_elevation: "ground level"` marker, from
§8's flood fill. It is not in the map; it is computed.

### `image_coords` — tested and rejected

The hypothesis was that Lich draws each floor as a band on one image, so the y
coordinate encodes elevation. It correlates, but not for that reason: comparing y
between the two rooms of each key item agrees with the answer **78.2% of the time**
(140 of 179 where both rooms share an image and differ in y).

Tempting, because on Jev's 34 v1 mistakes it was right 17 times against 5 wrong. But
**the author identified the flaw before it was built on**: these are coordinates on a
*flat* map image. A mapper may draw an upper floor beside, below, or on a separate
image entirely. The 78% is an artefact of common drawing habits, not a fact about the
world, and it disagrees with 15 of the 140 answers Jev got right at p≥0.99. **Not
used.**

### Ground floor is not *stored* — but it can be derived

Asked directly: does the map tell us which rooms are at ground level? **No.** MEASURED —
no field records a floor, level or storey. Searching every tag and location for
floor-words returns 110 tags, essentially all false positives (`meta:storyline` matching
"story"; `warrior guild floor` meaning a physical floor surface). This is the premise of
the exercise: `plan/21` §3f wants floors precisely because the map has none.

A *single-room* heuristic does not recover it either: "a room with fewer compass exits
is higher" scores **64.6%** (106 right, 58 wrong), below the 76% baseline. **Not used.**

**But a flood fill does.** That is the author's idea and it works — see §8. This section
originally concluded ground level was "not cheaply derivable"; that was wrong, and it
was wrong because it tested one weak heuristic and generalised from it.

### The answer key leaks, and it turned out not to matter

A real flaw, found while investigating the above. In **245 of 300 items** the
destination's `paths` line names exactly one vertical direction, and it is the inverse
of the answer — because the key selects items whose return leg is `up`/`down`, and the
exits line advertises that same exit. That line **is being sent**.

So accuracy was checked on the **55 items where no giveaway is present** (the
destination lists both up and down, or neither):

| subset | n | v1 | v2 | baseline |
|---|---|---|---|---|
| giveaway present | 245 | 89.0% | — | 78.0% |
| **no giveaway (clean)** | **55** | **87.3%** | **90.9%** | **67.3%** |

**The leak is not being exploited.** Accuracy barely moves, and the *lift over baseline*
is wider on the clean subset (+23.6 points for v2) than on the leaky one (+11). At
p≥0.99 the clean subset is **18/18** for v2. A model copying the exits line would
collapse here; this one does not. The headline number stands.

### v1 → v2: what changed

v2 rewrote the three option descriptions in GemStone's own vocabulary (pit, shaft,
chimney, dais, porch, landing, wall walk) and added guidance aimed at the two measured
failure modes: read the *departed* room for whether the feature is above or below, and
do not read a shared title as meaning "same floor".

| | v1 | v2 |
|---|---|---|
| accuracy | 88.7% | **90.0%** |
| clean subset | 87.3% | **90.9%** |
| `same floor` confusions | 8 | **4** |
| p≥0.99 | 139/140 | 139/140 |

**Read this as a modest, not-yet-proven win.** v2 fixed 10 of v1's mistakes but broke 6
of v1's wins — net +4 on 300 items, which is within the churn a prompt change produces.
The `same floor` improvement (8→4) is the clearest real effect.

### Where the remaining 30 mistakes actually live

Reading them, they are three kinds, and only one is the model's fault:

1. **The text does not contain the answer.** `go porch` — a porch is a step up from the
   street and the map agrees, but neither description says so. `go dais` likewise. These
   need world knowledge about how tall things are, not more fields. Jev's low
   probabilities here (0.44, 0.59) are it correctly reporting ambiguity.
2. **The key is arguably wrong.** `go ironwood bridge`, Ferry Landing → South Dock: two
   river docks, both at water level. The key says `up` only because the return leg is
   `down`. Jev said `same floor` at p=0.70 and is probably right. `plan/22` warns Lich's
   text is stale for ~5% of rooms.
3. **Genuine losses.** `climb chimney`: the destination says *"the only way to travel up
   or down within this vertical passage…"* and the origin says the cavern *"ends in a
   pit"* — a pit is below you. Jev answered `up` at p=0.91. The evidence was in the text
   that was sent.

**INFERRED:** the ceiling on this job is disambiguation and key quality, not context.
Before tuning further, the 30 mistakes should be hand-checked to see how many are the
map's fault — otherwise later work is tuning against a noisy ceiling.

## 8. Ground level: derived, and worth having

The author's proposal: take a room known to be ground level, and flood fill outward
through every exit that cannot change elevation. A door, arch or compass move keeps you
on the floor you are on; a staircase or ladder does not. Taking a portmaster lands you
at ground level too.

**This works, and [`ground.py`](ground.py) implements it.** It corrects §7's claim that
ground level is "not cheaply derivable" — that was measured against a *single-room*
heuristic (compass connectivity, 64.6%), and a flood fill is a different and better
idea.

### Seeds

328 rooms whose title contains a street word (`town square`, `street`, `road`, `plaza`,
`courtyard`, `crossing`, `gate`) **and** which have 3+ compass exits, i.e. sit on a real
street grid. `street_seeds()`.

### Two fills, and the precision/coverage trade

The answer key is the validator. A vertical exit joins two rooms at different heights,
so **if a fill marks both ends of a key item as ground level, the fill is wrong.** That
gives a free, objective error count.

| fill | crosses | rooms marked ground | key items touched | contradictions |
|---|---|---|---|---|
| **strict** (default) | compass moves and `out` only | **2,703 (7.3%)** | 21 | **0** |
| loose (`--loose`) | also `go <thing>` where thing is not a vertical word | 11,895 (32.3%) | 72 | **10 (14%)** |

```powershell
python ground.py "E:\Cena\reference\mapdb\map-1789942730.json" > results\ground.json
# seeds=328 strict=True ground=2703 of 36838 (7.3%)
```

**The strict fill is exact.** A compass move never changes floor, so the rule holds
without exception across all 300 known-vertical exits.

### Why the loose fill fails, and what it teaches

The 10 contradictions name the problem exactly:

```
20337 -> 20338  'go door'         [Abandoned Tavern, Taproom]  -> [Abandoned Tavern, Cellar]
 2381 ->  2382  'go fog'          [Magical Burrow, Illusionist Way] -> [Underground Tunnel]
29624 -> 29627  'go jagged crack' [Estate, Destruction]        -> [Estate, Crypt]
```

**`go door` into a cellar.** The command vocabulary does not encode elevation: `go door`
is flat thousands of times and descends here. No wordlist can separate those two cases,
because the difference is in the *rooms*, not the command.

That is not a defect in the idea — it is the boundary of what a rule can do, and it is
precisely the gap Jev is for. **INFERRED:** the right division of labour is the strict
fill for what rules can prove, and the model for the rest.

### One inference that does NOT follow

"Origin is ground level" does **not** imply "the exit does not go down". MEASURED: of
the 10 single-seed items where the origin was ground, 8 were `up` and **2 were `down`**
(`1251 -> 1252 'go tunnel'`, `25659 -> 25656 'go stairway'`). You can descend from the
street into a cellar or tunnel. The v3 prompt says so explicitly, to stop the model
over-applying the flag.

### What the flag was worth

**Not much yet, because it is rare.** Only 21 of 300 key items touch a ground-level
room. On those, v2 scored 20/21 and v3 scored 21/21 — a single item, far too few to
claim the flag caused anything.

It is kept because it is *free and exact*, and because coverage grows with better seeds.
The honest statement is: **sound, validated, not yet load-bearing.**

## 9. Removing the exits line — the change that actually worked

§7 found that the destination's exits line gives the answer away in 245 of 300 items.
v3 **stops sending it**.

Expected: a drop, since real information goes too. **Measured: an improvement**, and the
whole of it lands in the band the recommendation depends on.

| | v1 | v2 | v3 |
|---|---|---|---|
| exits line sent | yes | yes | **no** |
| ground flag | no | no | yes |
| accuracy | 88.7% | 90.0% | **90.3%** |
| **p≥0.99** | 139/140 | 139/140 | **149/149 — 100%** |
| coverage at that gate | 46.7% | 46.7% | **49.7%** |

Isolating the two changes: on the **279 items with no ground flag**, where the only
difference is the missing exits line, p≥0.99 went from **124/125 to 134/134**. More
items in the trusted band, and no errors in it.

**So the exits line was a distractor, not evidence.** "Obvious paths: down" invites the
model to reason about the destination's own exits instead of the two descriptions, and
removing it made the confident answers strictly better. Taking information away improved
the result — the opposite of the intuition that started §7.

Caveat, stated plainly: **headline accuracy moved 90.0% → 90.3%, and v3 fixed 9 of v2's
mistakes while breaking 8.** That part is noise on n=300. The p≥0.99 result is the real
finding, and it is the one that matters for a gated recommendation.

## 10. The third option was a trap — and removing it is the single biggest win

The author's observation: **every item in the key is truly `up` or `down`.** The key is
built from exits whose return leg is an explicit `up`/`down`, so `same floor` can never
be the right answer. Offering it gives the model a way to be automatically wrong.

MEASURED, on answers already in hand:

| run | times it chose `same floor` | all wrong? | accuracy if that option is ignored |
|---|---|---|---|
| v1 | 8 | yes | 88.7% → 90.3% |
| v2 | 4 | yes | 90.0% → 90.7% |
| v3 | 9 | yes | 90.3% → **92.7%** |

So v4 asks a **two-option** question: `up` or `down`, with the instruction stating that
one of them is true. The prediction from v3's own probabilities was 92.7%. **Measured:
exactly 92.7%.**

| | v1 | v2 | v3 | **v4** |
|---|---|---|---|---|
| options offered | 3 | 3 | 3 | **2** |
| exits line sent | yes | yes | no | no |
| accuracy | 88.7% | 90.0% | 90.3% | **92.7%** |
| clean subset (§7) | 87.3% | 90.9% | 87.3% | **90.9%** |
| **p≥0.99** | 139/140 | 139/140 | 149/149 | **167/167 — 100%** |
| coverage at that gate | 46.7% | 46.7% | 49.7% | **55.7%** |

**This is the only change in the trial that is clearly not noise.** v4 fixed 8 of v3's
mistakes and broke 1. Every other version traded roughly evenly (v2: +10/−6; v3: +9/−8).

### But it is only legitimate because of how the key is built

Forcing a binary choice **manufactures accuracy** if some exits really are flat. Several
of the nine v3 declined on look genuinely level:

```
go dock    [Ravelin, River's Edge]        -> [Ravelin, Riverside Dock]     p(same floor)=0.57
go altar   [Oleani's Temple, Sanctuary]   -> [Oleani's Temple, Altar]      p(same floor)=0.91
go trail   [Fethayl Bog, Ivory Pillar]    -> [Fethayl Bog, Lone Grave]     p(same floor)=0.82
```

Forced to choose, v4 got **7 of those 9 right**, all at moderate probability (0.55–0.95)
and **none in the trusted band** — it guesses reasonably and correctly signals that it is
guessing. That is the behaviour wanted.

**The rule this establishes:** the binary question is correct *only* where a rule has
already established the exit is vertical — which is exactly the key's construction, and
exactly the pipeline in §11 step 1. **Do not ask the binary question of an arbitrary
non-compass exit.** `go door` between two shops is flat, and a two-option prompt would
force a wrong answer with no way to say so. Those need the three-option form, and
`same floor` is a real answer there.

**UNVERIFIED:** this trial never measured the three-option prompt on exits that are
genuinely flat, because the key contains none. A second key — non-compass exits where
*neither* room has a vertical return leg — would be needed to trust `same floor`
itself. That is the main gap left.

## 11. The bigger test — 553 items, and what it cost the headline

§10 left one gap: the trial had never tested a genuinely **flat** exit, so `same floor`
was unmeasured. This closes it.

### There is no larger vertical key

MEASURED first, because it bounds everything: **300 is every exit in the map whose
return leg is an explicit `up`/`down`.** Deduplicated, across all 76,944 non-script
exits. The earlier tests were not a sample of a bigger set — they were the whole set.

### Key 2: exits known to be flat

Same trick, inverted. An exit whose return leg is an explicit **compass direction** puts
both rooms on one plane. [`build_keys2.py`](build_keys2.py) builds it: **253 items**
after dropping commands that name a floor-changing feature (stairs, ladder, cellar,
shaft), which may be vertical with a return leg the map never recorded.

Combined: **553 items, baseline 45.8%** — far harder than the vertical-only 76%, and
balanced enough that a lift means something.

### What the definition of "flat" is — the author's, not the physical one

> *"going on to a bridge isn't really changing elevations, even though in the reality
> sense you are, but we're not changing floors, we still end up on the same plane"*
> — the author, 2026-09-21

This is load-bearing and it is **not** what a model assumes by default. A bridge is
raised above what is under it, so a physical reading calls it vertical. The rule here is
about **floors**, not height: you walk out onto a span and you are on the level you
left.

**So spans are FLAT everywhere in this trial.** To be unambiguous, since it is easy to
state backwards:

| | how a span is treated |
|---|---|
| `build_keys2.py`, the flat key | **flat.** 67 of the 253 items are spans, labelled `same floor`. No span word appears in its `FLOOR_CHANGING` filter |
| `ask_v5.py`, the prompt | **flat.** The `same floor` text names bridge, footbridge, plank, pier, dock, walkway, catwalk and causeway explicitly |
| `ground.py`, the flood fill | **flat.** Span words were removed from its `VERTICAL` list for consistency. This changed nothing — the default strict fill crosses only compass moves and `out`, and never consults that list. Re-run after the edit: **2,703 rooms, identical set** |

Had spans been treated as vertical instead, the flat key would have been **196 items
rather than 253** — the 63 span items would have been dropped as suspect. They are kept.

### The result, and the honest cost

Two prompts over the same 553 items. **v5 is v3 plus one paragraph defining `same
floor` to include spans.**

| | v3 (three-option) | **v5 (+ the span rule)** |
|---|---|---|
| overall accuracy | 82.5% | **88.2%** |
| baseline | 45.8% | 45.8% |
| flat items (n=253) | 73.1% | **88.9%** |
| vertical items (n=300) | **90.3%** | 87.7% |
| p≥0.99 overall | 185/191 (96.9%) | **247/250 (98.8%)** |
| p≥0.99, flat only | 36/42 | **110/112** |
| p≥0.99, vertical only | **149/149** | 137/138 |
| `go bridge` items | 22 of 68 flat errors | **46/50 correct** |

**One paragraph moved overall accuracy 5.7 points.** Flat errors fell from 68 to 28, and
`go bridge` — which alone was a third of all flat errors — went to 46/50.

**The cost, stated plainly: vertical accuracy fell 90.3% → 87.7%, and the perfect
149/149 became 137/138.** Telling the model that raised spans are level makes it
slightly readier to answer `same floor` when the answer is really `up`. That is a real
regression, not noise, and it is the price of the gain on flat exits.

### The six confident errors were the KEY's fault, not the model's

v3's confident mistakes are worth reading, because every one is on the flat key:

```
go mound  [Orcswold, Old Road]   -> [Orcswold Moor, Mound]    key=same floor  chose=up
go mound  [Orcswold, Low Moors]  -> [Orcswold Moor, Mound]    key=same floor  chose=up
go pass   [Cairnfang, East Valley] -> [Rocky Pass]            key=same floor  chose=up
go path   [Rumor Woods]          -> [Rumor Woods, Ascent]     key=same floor  chose=up
```

**You climb a mound.** A room called *Ascent* is up. The key calls these flat only
because the map records a compass return leg and never recorded a vertical one. **The
label is wrong and Jev is right** in at least four of six cases.

**INFERRED:** key 2 is dirtier than key 1. Key 1's label comes from an explicit `up`/
`down` the map actually states; key 2's comes from the *absence* of one, which is weaker
evidence. Read the 88.9% flat figure as a floor, not a ceiling.

### What this does to the recommendation

The earlier 167/167 was real but **narrow**: it measured only exits already known to be
vertical. On the full problem — where an exit may be flat — the same gate gives
**98.8% over 45% of items**, not 100%.

Both numbers are true of different questions. §12 states which to use when.

## 11a. What this trial has NOT shown

Written after a deliberate "what are we missing" pass. In order of how much each matters.

1. **Every item tested is one a rule already answers.** Both keys are built from an
   explicit return leg, so by construction Jev would never be asked about them. The exits
   it *would* be asked about are the ones with no such leg. MEASURED: **3,178**
   vertical-looking exits have no rule-given answer, and **none were tested**. They may be
   systematically harder — the map is explicit where the geometry is obvious. **The
   scores above are UNVERIFIED as estimates for that population.**
2. **There is a label-free check for it.** Of those 3,178, **2,482** have a return leg
   that is also vertical-looking. A→B and B→A must be opposite, so asking both and
   counting disagreements measures the error rate with no answer key. Roughly 5,000
   calls, INFERRED ≈ $0.20. This is the next test worth running.
3. **The 0.99 gate was chosen on the data it is scored on.** No held-out set. Item 2
   fixes this too.
4. **Repeatability is untested.** The cache makes re-runs free, which also means no
   question was ever asked twice. Whether Jev is deterministic is UNVERIFIED.
5. **`out` is not strictly flat.** MEASURED: 229 `out` exits have a way back naming a
   floor-changing feature (`go hole`, `go steps`, `go tower`; some are false matches such
   as `go oak door`). Climbing `out` of a hole goes up. In practice it is harmless —
   refusing those exits shrinks the ground set from 2,703 to 2,702 — but the rule "compass
   and `out` never change floor" in §8 is stated too strongly.
6. **Portmasters as ground seeds** — the author's suggestion in §8 — was never built.
7. **Identical-title pairs improved** under v4: 33/36, and 13/13 at p≥0.99, against 27/36
   in v1. The "always review" rule in §5 may be stricter than needed; n=36 is small.
8. **The brief's item 4 is only half met.** `score.py` prints the titles of twenty
   mistakes, not the room text. The text is in the key files, joinable by room id.
9. **Latency was never measured.**
10. **`plan/22` still carries the errors §1 corrects.** Not edited: the brief limits this
    work to `research/jev-trial/` and `.gitignore`.

## 11b. The symmetry test — the exits Jev would really be asked about

§11a item 2, run. [`sym.py`](sym.py) takes every room pair joined by a vertical-looking
exit in **both** directions with no explicit `up`/`down`/compass leg: **1,241 pairs,
2,482 questions**, v5 prompt. A→B and B→A must be opposite, so an inconsistent pair
contains at least one wrong answer. No answer key is involved.

```powershell
python sym.py build "E:\Cena\reference\mapdb\map-1789942730.json" results\sym_pairs.jsonl
python sym.py ask   results\sym_pairs.jsonl results\sym_answers.jsonl
python sym.py score results\sym_answers.jsonl      # -> results\sym_score.txt
```

**Result: 864 of 1,241 pairs consistent — 69.6%.** This population is much harder than
the keyed one, as §11a suspected.

| gate: the LOWER of the pair's two probabilities | pairs | consistent | coverage |
|---|---|---|---|
| any | 1,241 | 69.6% | 100% |
| ≥ 0.7 | 828 | 81.0% | 66.7% |
| ≥ 0.9 | 467 | 91.4% | 37.6% |
| ≥ 0.95 | 307 | 94.5% | 24.7% |
| **≥ 0.99** | **136** | **96.3%** | **11.0%** |

What this says:

- **The 0.99 gate still holds, on data it was not chosen on.** 96.3% pair consistency
  implies a per-answer error near **1.9%** (INFERRED, assuming the two errors are
  independent) — in line with the 98.8% measured on the keyed test.
- **Coverage collapses.** Only **32.1%** of individual answers reach p≥0.99 here (796 of
  2,482), against 45–56% on the keyed exits. Ungated, the implied per-answer error is
  about **19%**.
- **The errors are a bias toward `up`.** 217 pairs answered `up` in both directions,
  against 83 `down`/`down`. All five pairs inconsistent at p≥0.99 on both sides are
  `up`/`up`, four of them `go stairs`/`go staircase` in both directions.
- **The hard case is the same command both ways**: 1,039 of the 1,241 pairs, 68.6%
  consistent, against 74.8% for the 202 pairs with different commands. `go stairs` from
  either end gives the model nothing but the two descriptions.

**Limit of the method:** consistency is necessary, not sufficient. A pair answered
`up`/`down` when the truth is `down`/`up` passes. The rates above are upper bounds on
accuracy.

**What it suggests for wiring in** (INFERRED, not tested): ask both directions as a
matter of course. Accept a pair only when the two answers are opposite; where exactly one
side is ≥0.99 (660 pairs, 53.2%), the other side disagrees 14.2% of the time, and those
go to review rather than being settled by the confident side.

Cost of this run: 2,482 calls (2,456 in the full run, 26 in a 20-pair smoke test),
**$0.118737**. Trial total: **$0.215428**. The provider returned HTTP 529 (overloaded)
at 8 parallel workers; 4 workers with a retry ran clean, 0 errors.

## 11c. The game's indoor/outdoor flag — exact, free, and no help to job 1

The author's observation: the wording of the exits line is the game's own flag.
**"Obvious exits" is indoors, "Obvious paths" is outdoors.** MEASURED: 159 of 165
shop-tagged rooms say "exits". Map-wide: 20,451 indoors, 15,894 outdoors, 284 rooms with
both wordings across variants, 209 with neither. That labels 36,345 of 36,838 rooms with
no model involved.

v6 is v5 plus that one derived word per room (`setting`), with the direction list still
withheld. On the 553-item key:

| | v5 | v6 (+ `setting`) |
|---|---|---|
| accuracy | 88.2% | 88.2% |
| vertical / flat | 87.7% / 88.9% | 87.7% / 88.9% |
| p≥0.99 | 247/250 | 258/261 |

**No effect on accuracy** (fixed 3, broke 3). The trusted band grew by 11 items with the
same 3 errors, which is within noise. Indoors/outdoors does not say which end of a
staircase is higher. Keep the flag for job 3 and for the map; it is not a job 1 lever.

## 11d. Job 3, services only — the tags are a poor answer key

[`job3.py`](job3.py). Key: Lich's `go2` service tags, 27 kinds, 451 rooms, plus 300
untagged indoor rooms as "none of these". **68.2% (512/751) against a 39.9% baseline;
82.6% at p≥0.99.** Most of the misses are the key's:

- **Tags mark where an NPC stands, and room text does not mention NPCs.** `chronomage`
  0/11 (`[Stone Turret, Ground Floor]`, `[Wehnimer's, Shop Cellar]`), `cleric` 2/10,
  `dyers` 3/10. Jev answers "none of these", a fair reading of the text.
- **`public locker` is on whole halls of identical rooms**: 9/89.
- **The tags are incomplete.** Jev called 62 of the 300 untagged rooms a service, among
  them `[Zellyle's General Store]`, `[Ashorien Brothers Movers]` and `[The Krawling
  Kraken, Front Desk]` at p=1.00. Lich tags one shop per town for `go2`, not every shop.
- `reagent shop` 0/10 is a legend fault: it overlaps `alchemist` and `pawnshop`.

Where the service shows in the text Jev is good: postoffice 15/15, fletcher 10/10,
gemshop 19/20, furrier 18/20, bank 32/36, inn 18/21.

**INFERRED:** Jev is more useful here as a finder of untagged shops than as something to
score against the tags. Checking its 62 finds needs a person; there is no key.

Cost: job 3 $0.034934, v6 $0.027107. **Trial total $0.277469.**

## 11e. Neighbouring rooms — a small real gain as evidence, weak as a rule

The author's idea: a chain of staircases says something about the rooms along it.

**As a hard rule** ("a room with exactly two vertical exits has one up and one down") it
does not hold well enough. It fails at hubs with two staircases both going up
(`[Silvergate, Grand Hall]`, `[Krol Ship, Main Deck Aft]`) and at peaks where both exits
descend (`[Damsel of the Deep, Forecastle]`). Restricted to pure landings — 414 rooms
whose only two exits are both vertical — the premise holds in 25 of 26 checkable cases,
it labels 70 of the 1,241 open pairs, and it still disagreed with Jev's confident,
consistent answers in 2 of 7, where Jev looks right. A first check that reported 97.1%
was **circular** (it compared literal `up`/`down` commands, and a room has at most one of
each) and is discarded. **Not used as a rule.**

**As evidence for Jev** it helps a little. [`ask_v7.py`](ask_v7.py) is v6 plus each
room's `other_exits` (command and destination title), with the exit being judged left
out of both lists so nothing leaks. Symmetry test, same 1,241 pairs, via `sym7.py`:

| | v5 | v7 (+ neighbours) |
|---|---|---|
| consistent pairs | 864 (69.6%) | **910 (73.3%)** |
| lower probability ≥ 0.9 | 91.4% of 467 | 92.8% of 470 |
| lower probability ≥ 0.99 | 131/136 (96.3%) | 134/138 (97.1%) |
| `up`/`up` pairs | 217 | 179 |

On the first 1,135 pairs answered, 65 became consistent and 25 became inconsistent; of
the 766 consistent in both runs, the direction flipped in 1. So the gain is real but
modest: **+3.7 points ungated, and the gated band barely moves.** The `up` bias shrinks
but remains the main error.

99 calls in this run failed with HTTP 400 `Unknown model: jev-1.13.0`, a provider-side
fault; re-running asked only those again. Cost: $0.141104. **Trial total $0.418573.**

## 11f. Accepting pairs instead of single answers — coverage 11% → 45–73%

The 11% in §11b required **both** directions at p≥0.99. That double-counts caution,
because asking both directions is itself a check. The open pairs cannot say how accurate
a gentler rule is, so [`sym_keyed.py`](sym_keyed.py) makes **labelled pairs** from the
553 keyed exits: forward as usual, reverse from the far room with the forward command
reused (the shape of 1,039 of the 1,241 open pairs), the literal return leg never shown.
v7 prompt, 1,106 calls, $0.063608.

| accept rule | keyed: coverage | keyed: accuracy | open pairs: coverage |
|---|---|---|---|
| forward answer alone, p≥0.99 | 45.9% | 99.2% | — |
| **consistent, at least one side p≥0.99** | 43.8% | **98.8%** | **45.5%** |
| consistent, both sides p≥0.7 | 63.3% | 95.1% | 55.7% |
| consistent pair, any confidence | 78.5% | 93.8% | 73.3% |
| consistent, both sides p≥0.99 (the old gate) | 14.6% | 100.0% | 10.8% |

Coverage on the keyed and open sets tracks closely (43.8% vs 45.5%, 78.5% vs 73.3%),
which is some evidence the keyed accuracy carries over. It is INFERRED, not measured.

**The 27 consistent-but-wrong pairs are mostly the key's doubtful items again**: Jev says
`same floor` both ways for `go ironwood bridge`, `go dock`, `go trail`, `go altar`, where
the key says vertical only because a return leg is `up`/`down` (§10, §11). Read 93.8% as
a floor.

### The author's ground-floor rule

"From a ground-floor room it is up, unless the destination's name says down." On the 300
keyed vertical exits: **20 of 21 (95.2%)** from a strict-fill ground room, against 81.0%
for always-`up`; 85.7% of 70 from the loose fill; **80.0%** applied to all 300, against
Jev's 92.7%. Sound where ground is known, limited by the fill reaching 7% of rooms. The
down-word list is mine and improvable.

Room names as a cross-check on a consistent pair add little accuracy (95.3% vs 95.1%),
but **a consistent pair the names disagree with is right only 78.6% of the time** (2.5%
of keyed pairs, 3.1% of open pairs). That is a cheap review flag.

**Trial total $0.482181.**

## 11g. Classifying room names with Jev — works alone, adds nothing on top

The word list in §11f was written from general knowledge and never checked against the
map's names; the author rightly called that arbitrary. The replacement, the author's
idea: [`names.py`](names.py) has Jev judge each real room name alone as `below ground`,
`ground level`, `above ground` or `cannot tell`. Run on the 2,744 distinct names among the
keyed exits and open pairs: 445 below, 827 ground, 667 above, 805 cannot tell. $0.066008.

**Name-only prediction, 300 keyed vertical exits** (direction = the end whose name is
higher):

| both names judged at | covers | correct |
|---|---|---|
| any probability | 73 (24.3%) | 90.4% |
| p ≥ 0.9 | 25 (8.3%) | 100% |
| p ≥ 0.99 | 14 (4.7%) | 100% |

Accurate where both names carry a level, which is a quarter of exits.

**As a cross-check on direction-consistent pairs it adds nothing** (keyed, n=434): names
agree 95.2% correct (63), names silent 93.1% (346), names *disagree* **100% correct**
(25). The disagreements are mostly flat exits between differently-named places (a dock
and a street), where `same floor` is right. Jev already reads both names inside the pair
question, so asking about them separately tells it nothing new. **The name-disagreement
review flag of §11f is withdrawn**, and the full 20,226-name run was not done.

**Trial total $0.548189.**

## 11h. Every open exit, tiered — `results/job1_proposals.jsonl`

The author's call: walk down the accuracy bands until every exit has an answer, keeping
the best band each one qualifies for.

**The count, reconciled.** 3,178 open one-way exits = 2,482 in the 1,241 checkable pairs
+ 696 single exits (234 whose way back is `out`, 218 a plain word such as `go door`, 162
with no way back, 82 a Ruby script). The singles were asked both ways too, with the
forward command standing in where there is no usable way back. $0.079677.

Decision per pair: if the two directions agree, take that answer; if they disagree, take
the more confident side. Bands, with accuracy **for that band alone** on the 553 labelled
pairs:

| band | rule | accuracy (labelled) | pairs (1,241) | singles (696) |
|---|---|---|---|---|
| A | agree, both p≥0.98 | 98.1% (106/108) | 181 | 44 |
| B | agree, both p≥0.90 | 98.2% (111/113) | 255 | 92 |
| C | agree, both p≥0.70 | 89.9% (116/129) | 255 | 155 |
| D | agree, weaker | 88.1% (74/84) | 219 | 157 |
| E | disagree, one side ahead by 0.5+ | 88.9% (8/9) | 14 | 10 |
| F | disagree, close call | 75.5% (83/110) | 317 | 238 |

Answering everything scores 90.1% on the labelled pairs. The singles skew to weaker
bands (34% in F against 26%), as expected of exits with no real way back to compare.

The file holds 1,937 rows (1,241 pairs + 696 singles), sorted best band first: `from`,
`to`, `command`, `back_command`, `direction` (956 up, 612 down, 369 same floor), `band`,
both probabilities, `source: llm` and the model id. **It is a results file, not the
authored-corrections file of `plan/21` §3f**, whose format is still the author's to
decide. Band accuracies are fitted on the same labelled pairs they are measured on, and
applying them to the open exits is INFERRED.

**Trial total $0.627866.**

## 11i. The game's own room metadata — found in the author's survey sessions

The game sends `<roommeta weather= bonfire= inside= water= sanctuary= realm= climate=
terrain=/>` for every room. The author's survey runs saved each reading with the Lich
room id: 94 sessions under `E:\Gemstone\data\forge data\forge_sessions\*-survey\
events.jsonl`, code names in `forge_survey\state-GST-Nisugi.yml`. Extracted to
`results/roommeta_from_survey.json`: **20,149 rooms** (surveyed 2026-08-17 to 08-20).

- **The map's missing terrain is a recording gap, not the game's truth.** Of 7,451
  surveyed rooms where the map has no terrain, the game sets one on **6,814**; only 637
  read code 0. This corrects the premise recorded in `plan/22` ("rooms without them are
  set up without them in the game"). Two terrains, `ruins` and `open water`, are not in
  the map at all.
- **The map's `"none"` is often stale**: the game reads `subterranean` for 853 of those
  rooms, `hard, flat` for 1,049, `plain dirt` for 596.
- **`subterranean`: 1,411 rooms in the game, 25 in the map.** Where Jev judged the room
  name, 305 of 309 are `below ground` — a clean underground marker.
- **`inside` matches the exits-line wording** on 20,044 of 20,149 rooms (§11c).

Effect on job 1 is small, because most vertical exits stay within one terrain. The
author's terrain-change rule (crossing into `subterranean` is down): **4 of 4** on
labelled vertical exits, wrongly speaks on 2 of 253 flat ones, speaks on 52 open rows
(agrees with the proposal on 45). Using the flag as the "below ground" region for the
distance-from-surface rule adds little over the name levels: 125 labelled exits at 89.6%
against 118 at 89.0%; open rows reached 711 against 643. Only 1,051 of the 1,937 open
rows have a game reading on both ends; the survey did not reach the rest.

## 11j. Every open exit with an estimated accuracy, in 1% bands

[`classify.py`](classify.py) → `results/job1_classified.jsonl` (1,937 rows, best first)
and `results/job1_classified.txt` (the band table). The author picks which bands to keep.

553 labelled pairs cannot measure a 1% band directly, so a small logistic model is fitted
on them: do the two directions agree, the lower and higher probability, and whether the
author's distance-from-surface rule agrees, disagrees or is silent. Rows touching a
subterranean room use the terrain-aware prompt (v9, §11i); the rest use the neighbour
prompt (v7). The two-option prompt is not used: every labelled exit it could be
calibrated on is truly vertical, so its estimates would be blind to open exits that are
really flat.

Summary (full table in the .txt): top band is **97%** (261 rows); **≥95%: 510 rows
(26%)**; **≥90%: 1,124 rows (58%)**; ≥80%: 1,476 (76%); all 1,937 rows average 87.8%.

**How far to trust the estimates** — held-out check, 5-fold, v7 model:

| estimate band | labelled pairs | estimated | actually right |
|---|---|---|---|
| 0.97–1.00 | 27 | 97.2% | 100.0% |
| 0.93–0.97 | 290 | 94.8% | 96.6% |
| 0.88–0.93 | 95 | 90.9% | 85.3% |
| 0.80–0.88 | 59 | 84.2% | 79.7% |
| below 0.80 | 82 | 73.7% | 76.8% |

The top two bands are honest or slightly cautious. **The 80–93% range is about five
points optimistic.** Nothing is estimated at 98–99% because the best labelled group has
2 misses in about 220, and both look like the key's doubtful items. The estimates carry
over to the open rows only by inference; those rows have no answer key.

## 11k. A floor for every room — first pass, with known faults

The author's decision: keep exits at an estimated 93% or better (879; the 1,058 below
go to `results/job1_remaining.jsonl` for later) and fill in floors. [`floors.py`](floors.py)
→ `results/floors.json`, `results/floor_conflicts.jsonl`. It follows `plan/21` §3f:
literal `up`/`down` ±1, Jev's kept exits, and **the ground-floor default** — compass
moves, `out` and plain portals keep you on your floor. Rooms joined by same-floor moves
are a *level*; levels joined by up/down are a *stack*; a stack with a known ground room
is anchored there at 0, otherwise its most-outdoor level is 0 (`anchor: default`).
Not crossed: 7,923 Ruby-script exits and 1,717 height-word exits of unknown direction.

**Result:** all 36,838 rooms have a floor; 29,734 are floor 0; range −13 to +14. 63.3%
are anchored to known ground, 36.7% by default; 9.9% depend on a Jev edge.

**Checks.** Town centres: 17 of Lich's 21 `town` rooms are floor 0, and all 12 rooms
titled "town square/center". Exceptions: `[Cysaegir, Linsandrych Common]` −3 (with its
bank, inn, gemshop and furrier), `[Ta'Illistim, Hanging Gardens]` +1, Pinefar +1, a
crow's nest +1. Room names: of 1,381 rooms named below ground only 24 sit above 0, and of
1,256 named above ground only 80 sit below 0 — but about half of each sit AT 0.

**Known faults — 503 conflicts, 568 rooms touched, none resolved:**
- **336 same-level**: an up/down exit whose two rooms the default puts on one level. In
  **102 of them the two rooms are joined by compass moves alone**, so "a compass move
  never changes floor" is false outdoors: on a hillside `up`/`down` are slopes and the
  trail winds back. 124 of the 336 are outdoors at both ends. For the rest a portal on
  the loop changes level; only 19 trace to a single portal.
- **167 stack**: two routes between the same two levels disagree (109 involve a Jev edge).
- Cysaegir at −3 is the slope problem in one picture: a town reached by a mountain trail
  inherits every `down` along the way.

### Second pass: outdoor slopes (`floors.py 0.93 --slopes-flat`) — the current `floors.json`

A prediction of mine ("124 conflicts go away if outdoor up/down does not count") was
tested instead of trusted, and was wrong in both directions.

| | first pass | any outdoor up/down is a slope | **only the map's literal `up`/`down` outdoors** |
|---|---|---|---|
| conflicts | 503 | 312 | **348** |
| `[Ta'Illistim, Hanging Gardens]` | +1 | 0 | +1 |
| `[The Contempt, Crow's Nest]` | +1 | 0 | +1 |
| `[Pinefar Trading Post, Greatroom]` | +1 | 0 | +1 |
| `[Cysaegir, Linsandrych Common]` | −3 | +1 | +1 |
| rooms named above ground that sit above 0 | 576 | 395 | 548 |

The blunt rule flattened real heights (treetops, battlements, rigging are outdoors too)
and is dropped. The narrow rule treats 1,054 literal outdoor `up`/`down` exits as slopes
and keeps every climb Jev read. `results/floors_before_slopes.json` is the first pass.

**Author's decisions (2026-09-21):** the Hanging Gardens at +1 is right ("it is
hanging"); Pinefar at +1 is accepted, to revisit if it proves wrong, so no porch-steps
rule was added; pinning every `town` room at 0 was dropped because it would flatten the
gardens. Cysaegir's +1 was traced: its 186-room main level sits one step above
`[Cysaegir, Gorge Switchback]`, by a literal `climb steps` and a Jev `climb path` (0.94).

Still open: the 348 conflicts, the 1,717 height-word exits of unknown direction, and the
1,058 exits below the 93% cut.

## 11l. What is left to settle — 873 walkable exits

Author's decision (2026-09-21): of the 1,058 exits below the 93% cut, drop the ones
nobody can walk. [`set_aside.py`](set_aside.py) splits `job1_remaining.jsonl` into:

- `results/job1_to_settle.jsonl` — **873** exits on walkable rooms (the work list);
- `results/job1_set_aside.jsonl` — **185**, each with a `set_aside` reason: **177** touch a
  room that cannot be reached from any of Lich's 21 `town` rooms, **8** touch a room tagged
  `closed` or `gone` (13 rooms: ShadowGuard's three towers, Brass Tower, Naval A Salt,
  Little Man's Alehouse, the Solhaven Watchtower, Weffinait's Wagon).

Reachability follows every exit in the map, scripted ones included. Sixteen town centres
share one 27,959-room network; Caligos Isle (508 rooms), Evermore Hollow (934) and the
Flotilla (116) are separate. It cannot see places entered by ticket, boat or one-way
portal, so "unreachable" means "not connected in this map file". Jev's answer is kept on
every set-aside row; nothing is deleted.

The 873, by estimated accuracy: 90–92% about 200, 80s about 300, 70s about 280, below 70
about 80. Estimates in the 80–92% range ran about five points optimistic (§11j).

## 11m. A stronger reader on the hard pairs — Claude Opus subagents, blind

The author's point: no API key is needed to try a stronger model; subagents in the
session can read the rooms. [`agent_test.py`](agent_test.py) builds blind batches from the
**237 labelled pairs the calibration puts below 93%** — no answers, room ids, way-back
commands or exits lines, and each room's other exits with the judged exit removed. Six
Opus subagents, one batch each, told to read only their batch file and to give a
direction, a 0–100 confidence and a one-line reason. Same floor definitions as Jev's
prompt. No API cost.

| | Jev (both directions, v7) | Opus subagent (one reading) |
|---|---|---|
| right, all 237 | 190 (80.2%) | 193 (81.4%) |
| truly up (119) | 102 | 97 |
| truly down (37) | 26 | 26 |
| truly same floor (81) | 62 | **70** |

**As a plain reader the stronger model is a tie with Jev on these pairs** — they are hard
because the text is thin, not because Jev reads badly. It is better at flat exits and a
little worse at "up". What it adds is a confidence that sorts well, and a second opinion:

| rule | exits | share | right |
|---|---|---|---|
| agent confidence ≥ 90 | 42 | 17.7% | 100% |
| agent confidence ≥ 85 | 117 | 49.4% | 94.9% |
| agent confidence ≥ 80 | 147 | 62.0% | 93.2% |
| agrees with Jev and confidence ≥ 80 | 138 | 58.2% | 93.5% |
| agrees with Jev and confidence ≥ 85 | 112 | 47.3% | 95.5% |
| the rest (agree but < 75, or disagree and < 85) | 71 | 30.0% | 56.3% |

So about **half to 60% of the pairs Jev could not settle reach 93–95%** with the agent's
confidence as the gate, and the leftover 30% is close to a coin flip — those are the
ones for the author. Thresholds here are chosen on the same 237 pairs they are scored
on; n is small; one run, and a subagent's answers are not cached or exactly repeatable.

### The same subagents on the 873 open exits — `results/job1_agent_open.jsonl`

22 blind batches of up to 40 (`results/agent_open/`, shared `INSTRUCTIONS.txt`), one Opus
subagent each, all 873 answered with a valid direction. The agent agrees with Jev on 607
(69.5%). Gates, with the accuracy each showed on the 237 labelled pairs above:

| gate | labelled accuracy | open exits passing | share of 873 |
|---|---|---|---|
| agent confidence ≥ 90 | 100% (42) | 175 | 20.0% |
| agent confidence ≥ 85 | 94.9% (117) | 361 | 41.4% |
| agrees with Jev and ≥ 80, or ≥ 90 | 93.5% (139) | 415 | 47.5% |
| agent confidence ≥ 80 | 93.2% (147) | 480 | 55.0% |

Each row carries the agent's direction, confidence and one-line reason, Jev's direction
and estimate, and which gate it passes, sorted most confident first. The labelled
accuracies are fitted on 237 pairs and carried over to the open exits by inference. The
gate is the author's choice; nothing has been merged into `floors.json`.

### Author's gate (2026-09-21): 93%, and the floors rebuilt

Gate chosen: the subagent **agrees with Jev at confidence ≥ 80, or is at confidence ≥ 90**
(93.5% on the labelled pairs). The looser "confidence ≥ 80" adds 65 exits that are, on
their own, 7 of 8 right on labelled pairs — too thin to call 93% — so they are not
settled, but they head the review list with the subagent's answer as the best guess (Jev
was right on 1 of those 8).

- `results/job1_kept_agent.jsonl` — **415** exits settled. `floors.py` now reads it.
- `results/job1_review.jsonl` — **458** for the author, each with `best_guess`,
  `verified: false` and a `review_priority`: 1 = subagent fairly sure and disagrees with
  Jev (65), 2 = other disagreements (186), 3 = both agree but neither is sure (207).

Settled in total: 879 + 415 = **1,294 of the 1,937** open exits; 185 set aside as
unreachable; 458 to review.

Floors after the rebuild (`floors_before_agent.json` is the previous pass): 1,966 rooms
changed floor; height-word exits still uncrossed fell from 1,717 to 1,026; rooms named
below ground that sit below 0 rose from 588 to 704, named above ground that sit above 0
from 548 to 623; the four off-zero town rooms are unchanged. Conflicts rose from 348 to
463 — more edges means more loops to disagree; 281 of them now involve an LLM edge.

### Exits that cross more than one floor are not conflicts

The author's example (2026-09-21): in Emberthorn Refuge a tree-house room at +2 has a
swing straight down to a lake at 0. "One exit = one floor" is only the default for
placing a level nobody has reached yet; floors are set level by level, so a long exit
simply lands where its far end already sits. Emberthorn came out right (Bowery 0, Nook
+1, Platform +2, and `climb rope` Garden 0 → Platform +2) and was wrongly listed as a
conflict. `floors.py` now writes such exits to `results/floor_spans.jsonl` with the floors
crossed, and counts a stack conflict only when the direction contradicts the two floors
or they came out equal. 11 span exits; conflicts 463 → 452.

The span list is worth a look in its own right. Some entries are plainly real (Emberthorn's
rope; `[Bamboo Cottage, Basement]` −1 by staircase to the Attic +1). Others point at a
wrong floor somewhere nearby: `[Tower]` +1 by trapdoor to `[Wehnimer's, Burrow Way]` at
−1, where a street should be 0; `[Commander Dragorth's War Room]` +6 by hatch to a vault
at 0.

## 11n. Floors, second method — outdoor ground is floor 0 (`floors2.py`)

**Why a second method.** `floors.py` joins rooms over every same-floor move, so one
exit wrongly called flat merges two levels across the whole map. MEASURED on its output:
its largest level is **9,637 rooms** and holds **100 of its 320 same-level conflicts**;
**11,987 rooms** sit in stacks whose zero was a guess.

**The rules** (`python floors2.py`; writes `results/floors2.json`,
`results/floor2_conflicts.jsonl`; `floors.py` and its outputs are untouched):

1. Outdoor rooms are joined over same-floor moves. An outdoor level holding a known ground
   room, or of 25+ rooms, is floor 0. Indoor rooms are joined only with other indoor rooms,
   and an indoor level takes its floor from its plain door to the outside. An error cannot
   leave one structure.
2. The map's literal `up`/`down` between two outdoor rooms is a hillside slope (level) —
   UNLESS it is the only way into a pocket of 8 rooms or fewer, or a room name at either
   end has a built-structure word (deck, mast, nest, roof, tower, wall, terrace ...; 39 of
   the 448 outdoor literal `up` exits). The 8 and the word list are guesses.
3. A dead-end pocket of 40 rooms or fewer reached by an outdoor climb takes its floor from
   the climb (crow's nest, tree house, beach under a cliff). Bigger one-entrance regions
   stay at 0: left free, Icemule's town level floated.
4. **A ship's main deck is floor 0** (author). Keyed on the words "Main Deck". A crow's
   nest and a quarterdeck both come out +1: the pass counts climbs, not height, and the
   author accepted that ("a crows nest probably gets taller the bigger the ship").
5. A small "outdoor" level whose every neighbour is indoors (a rooftop, or a shop in a
   ship's hold that the map marks outdoors) waits for its building. Five such shops had
   pulled the Spitfire's 129-room cargo hold up to 0 against a literal `down`.
6. **A bug inherited from `floors.py`, fixed here only:** `go gate` whose way back is a
   literal `down` was classed flat one way and down the other. The way back is now checked
   before the plain-portal default. `floors.py` still has it, so its 452 is inflated.
7. **Indoors, the literal `up`/`down` is real and the compass way round is the slope**
   (author's reading; I had proposed the reverse). 62 indoor `up`/`down` exits join two
   rooms that compass moves also join. Glaes Vein 2240 settles which is which: *"A stark
   plunge down marks the southern finger ... To the southeast, the route turns down, but
   does so in a gentle decline."* — the `down` is the plunge, `southeast`→`southwest` the
   decline, both reach room 2242. MEASURED over the 62: slope wording on the compass
   route 23, only in the two end rooms 13, none 14, and 12 are MAZES (every room on the
   loop has one description; Graveyard Under Crypt reaches one room by `east`,`east` and
   by `west`,`southwest`). `cut_slopes()` cuts one compass move per route — the one whose
   rooms have slope wording, nearest the far end; the last move when none — and leaves
   mazes alone. **51 moves cut (37 on wording, 14 by position), 12 mazes left**;
   written to `results/floor2_slope_cuts.jsonl`. A cut move is not crossed at all. The
   room in the middle of a slope has no true floor; it lands with the near end.
8. **The indoor anchoring step repeats until nothing moves.** Run once, a balcony or
   courtyard took its floor from one building and a SECOND building opening onto it was
   never anchored from it. MEASURED: in 61 of the then-104 indoor/outdoor mismatches the
   indoor side sat at a default 0. Conflicts 272 → 209, those mismatches 104 → 41. My
   guess that they were mostly slopes was wrong: slope wording matched 16 of the 41 and
   most matches were noise ("walls rise up"); about three are real slopes.
9. **Steps up to a porch are level** (author: "agree steps are level"). A room named
   Porch, Veranda or Stoop is on its building's floor, whether the exit was read by
   Jev/the subagent or is the map's own `down`. All 47 Porch rooms are now at 0 (16 were
   at +1), and **Pinefar's town centre went to 0** — its +1 was porch steps, as the
   author suspected. Conflicts 209 → 203. TESTED AND REJECTED as wider: every `go steps`
   a reader judged, made level (`--steps-flat`) — conflicts rise to 235 and 2,653 rooms
   move. Steps in general do change floors; porch steps do not.
10. **The room-name list breaks the tie between a plain door and an `up`/`down`**
    (`results/name_levels.json`, section 11g). Of the 94 both-indoors conflicts, 43 had
    BOTH ends anchored at 0 by a plain exit to the outside: the Wayside Inn Garret by `go
    grate under ashes` to the street, the 120-room Catacombs level by a one-way drop in
    from the City Stables, the Abbey Cellar by its door to the kitchen garden. An indoor
    level whose names Jev called below or above ground waits for the `up`/`down` walk
    and takes a door's floor only if nothing walks to it.
    - **CORRECTED the same evening.** I first rejected "one off-ground name decides the
      level" because a sample showed street-level shops at −1 (Solhaven Grocer, Haldrick's
      Armory, the Temple of Lorminstra entry), and kept a cautious version (half the level
      named, no ground names). The author asked the right question: *"if there are 3
      connected rooms on the same level and one of them is called cellar, why would that
      not make the level -1?"* It should. The shops broke because their "levels" were
      fake — see rule 11 — not because the rule was wrong.
    - KEPT (author's rule): more off-ground names than ground-level names, and not both
      below and above. MEASURED after rule 11, `up`/`down` conflicts: no name rule 153,
      cautious version 144 (`--names-half`), **author's rule 121**. Shops stay at 0, the
      Garret is +1. Flat-exit mismatches rise 30 → 62: plain exits now believed to change
      floor (the Garret's grate), not new errors. `--no-names` turns it off.
    - THE LIMIT IS COVERAGE. The list classifies 2,744 of the map's 19,561 distinct room
      names. Of 282 names with an obvious level word (Cellar, Basement, Attic, Garret,
      Loft, Crypt, Catacomb, Upstairs) only 133 are classified — `[Abbey Cellar]` is not,
      so it is still at 0. 41 Cellar/Basement rooms and 14 Attic/Garret/Loft rooms sit at
      floor 0. Classifying the remaining names is the obvious next step.
    - **DONE, and it changed little.** The author had asked for every name the night
      before; I had run 2,744 and re-offered the rest as optional. All 20,226 titles are
      now in `results/name_levels_all.json` (17,479 new calls, **$0.42**, 0 errors; 9,886
      cannot tell, 5,984 ground, 2,311 below, 2,045 above; 5,536 confident and usable).
      `floors2.py` uses it by default (`--names-small` for the old list). MEASURED among
      walkable rooms: conflicts 159 → 162, Cellar/Basement rooms at 0 37 → 35,
      Attic/Garret/Loft at 0 unchanged at 8. WHY SO LITTLE: rule 10 only makes a cellar
      level WAIT for an `up`/`down` to reach it. Where the stairs into the cellar are
      themselves an uncrossed height-word exit, nothing reaches it and it falls back to
      its door's floor. `[Abbey Cellar]` now reads below ground at 1.0 and still lands at
      +1, because the map joins it by compass moves to the Abbey Kitchen, whose `down`
      leads to a Buttery that doors anchor at 0.
    - Names as a direct decider of uncrossed exits: of the **302 height-word exit pairs
      still uncrossed among walkable rooms**, the name verdicts of the two LEVELS decide
      13 (`results/name_decided_exits.json`, not applied; 12 look right on reading, one —
      Solhaven Inn Upstairs Hall `go steps` to A Concealed Room as "down" — is doubtful),
      90 have the same verdict at both ends, 199 have a level with no verdict.
11. **`urchin guide ...` and `ask <npc> about ...` are teleports, not walks, and are not
    crossed.** Eight Urchin Hideouts have 35–46 `urchin guide` exits each (429 in all), one
    to every shop in town; crossed as flat moves they glued Solhaven's shops into ONE
    155-room indoor level and Icemule's into one of 118. Removing them alone took
    conflicts **205 → 183** and the second-largest indoor level from 205 rooms to 165.
    `--teleports` puts them back.
    **Then widened, on the author's word** (*"urchin rooms and other rooms like that we
    can't actually access should not be touching any of the level stuff"*): WALKABLE =
    reachable from a town centre without `;e true` placeholders (523 in the map), urchin
    guides or `ask` exits. **28,810 of 36,838 rooms are walkable**; the other 8,028 are the
    7,968 no town reaches (Caligos Isle, Evermore Hollow, the Flotilla, event grounds) plus
    60 reached only through those exits (Urchin Hideouts, guild back rooms, "A Hidden
    Room"). The **969 exits joining a walkable room to a non-walkable one are dropped**.
    Non-walkable rooms still get floors among themselves and are marked
    `walkable: false` in `results/floors2.json`. Conflicts 183 → 171, of which **159 are
    among walkable rooms** (112 `up`/`down`, 47 flat exits) and 12 among the rest.
    Other odd verbs still crossed as flat and not examined:
    `pedal` 578, `swim` 349, `pull` 118, `jump` 66, and 8 `clim ladd` abbreviations the
    height-word pattern does not recognise.
    - NOT A FIX FOR MISSING VERTICALS: of the 568 height-word exit pairs still uncrossed,
      the names on the two ends decide the direction of **7**.
    - OPEN: 1,202 rooms have a confident off-ground name and a floor that disagrees (531
      of them outdoor rooms, which rule 1 puts at 0 by definition). Not examined. The
      comparison is partly circular, since rule 10 uses the same list.

12. **Round two for the subagents (`agent_round2.py`) — little came of it, and that is the
    finding.** An UNCROSSED exit is a height-word exit (stairs, steps, ladder, hole, ramp,
    tunnel ...) whose direction nobody has settled: the map has no literal `up`/`down` on
    either leg, Jev was under 93%, and the first subagent did not pass the gate.
    `floors2.py` does not cross it at all. Among walkable rooms there are 273, of which 271
    are on the review list. Those, plus the 70 conflict exits whose direction came from Jev
    or a subagent, went to nine Opus subagents (343 exits), this time with each room's
    computed floor and how it was reached, Jev's verdict on the room's name (both labelled
    fallible), and the author's rulings on porches, hillsides, sloping tunnels and spans.
    - **Uncrossed: 33 of 273 pass the 93% gate** (confidence 90+, or 80+ and agrees with
      Jev). Only 5 answers reached 90. Confidence: 50s 45, 60s 86, 70s 92, 80s 45, 90s 5.
    - **The second reader changed the first reader's direction on 92 of 271 (34%).** Two
      careful readings of the same text disagree a third of the time: these exits are
      underdetermined by the text, not under-read. 128 of the 273 came back "same floor".
    - **Conflict exits re-read: 60 of 70 confirmed as they stand** (48 at 80+), and the 10
      that differ are all under 80. The exit a reader judged is not the faulty edge in
      those conflicts; something else on the loop is.
    - Applied (`results/job1_kept_round2.jsonl`, `--no-round2` to leave out): conflicts
      among walkable rooms 162 → 169. Only 2 of the 33 are themselves in a conflict — a
      cliff-face `climb protrusion` and a treetop `go rope`, both between two outdoor rooms
      that rule 1 holds at 0, and both right on the text. The other 5 are knock-on.
    - NO ANSWER KEY exists for these, so none of this is scored. All 343 answers with
      reasons are in `results/job1_round2.jsonl`.

13. **The author's own answers (`apply_author.py`), which outrank everything.** He plays
    the game and has the in-game map. Three exits, each with his reasoning recorded:
    Czeroth Caverns `climb rocks` = same floor ("it's a downward slope, not a drop, and
    there's not many other rooms that would change a level so it would make the area 2
    levels when it should be one") — that one exit pinned **175 rooms**; Zaerthu `go
    crevasse` = same floor (the wall is sealed with granite "to prevent anyone from
    proceeding further down the tunnel", so this is a tunnel entrance through a wall, not
    a hole in the ground) — 41 rooms; Altar of the Elder `climb steps` = down.
14. **Outdoors, a step is a slope (`SLOPE_CMD`).** Rule 2 flattened only the map's literal
    `up`/`down`, so Solhaven's **Tumbledown Lane** — ONE cobbled street down a cliffside,
    "the uneven, downward sloping cobblestones", 8 rooms — had its `up`/`down` flattened
    and was then split across floors −1, +1 and +2 by the `go steps` between its segments.
    The author caught it: *"I'm not sure tumbledown lane is up/down."* Outdoors, a command
    naming steps, a ramp, a slope, an incline, a path or a trail is the hill. `BUILT` and
    the dead-end pocket still override. Conflicts among walkable rooms **169 → 152**.
    - TESTED AND REJECTED first: steps between two rooms sharing a name prefix are level.
      No separation at all (35.3% level vs 32.3%), because the prefix is a naming
      convention, not a space: `[Solhaven, Tumbledown Lane]` shares a TOWN name across
      three floors while `[Felinium's Fur Emporium]` and `[Felinium's Fur, Workroom]` are
      one shop written two ways.
    - The 553-exit key says steps change level 43/43 — but the key is BIASED here: it was
      built from exits whose return leg is a literal `up`/`down` or a compass move, so a
      step that keeps you level (`go steps` both ways) could never enter it. 119 of the
      map's 373 step exits are currently level. The key cannot settle this one.
15. **The outdoor cluster rule, from Simutronics' own layouts** — see section 11o. Two
    OUTDOOR rooms drawn on one plate are on one floor: 25 exits flattened, conflicts
    **152 → 148**. It moved one Hanging Gardens room to 0; the author: "don't care about
    hanging gardens room". `--no-clusters` turns it off.

**Result, MEASURED** (`python floors2.py`, cut 0.93):

| | `floors.py` | `floors2.py` |
|---|---|---|
| Conflicts among walkable rooms | 452 (all rooms, and its own bug inflates it) | **148** |
| Largest level | 9,637 rooms | 2,416 outdoor ground; largest indoor 422 |
| Rooms whose floor is a guess | 11,987 | 1,886 of 28,810 walkable |
| Town centres not at 0 | 4 | 1 (The Contempt's crow's nest, +1) |
| Main decks at 0 | not checked | 56 of 56 |
| Crow's nests at +1 | 4 | 19 of 22 |

The path: 314 → 272 (7) → 209 (8) → 203 (9) → 171 (10, 11) → 174 (full name list) → 181
(12) → 169 (13) → 152 (14) → **148** (15). Checkpoints that hold: Emberthorn Platform +2,
Burrow Way −1, Spitfire cargo hold −2, all 56 main decks at 0.

**Known open:** 42 rooms at −9 to −14 are one cave system whose chain of `down`s re-emerges
on Thurfel's Island at 0. 240 height-word exits are still uncrossed
(`results/uncrossed_ranked.tsv`, ranked by how many guessed-floor rooms hang on each; 126
have any, in 115 groups, and the top 5 groups cover 300 rooms). 148 conflicts are listed
room by room in `results/floor2_conflicts_walkable.tsv`.

## 11o. Simutronics' own map layouts — what they do and do not tell us

`reference/mapdb/map-data/prime/layouts.json` is **123 prebaked render layouts**, generated
by their `bake-layouts.ts`, "Do not edit by hand". Each holds `pos` (a room's x,y on a
grid) and `clusters` (which rooms are drawn on one plate, each `main` or `adjacent`).
14,292 of their rooms match a Lich room by uid; **13,689 are walkable rooms of ours, 48%**
— and the split is lopsided: **9,065 of 12,854 outdoor rooms (71%) but only 4,624 of
15,956 indoor (29%)**. It is a wilderness map.

**IT CONTAINS NO FLOOR DATA.** I first reported "official clusters agree with our floors on
91.5%" — that was wrong and I should have read their README first. A cluster is a DRAWING
artefact. What makes it useful is indirect, and the author found it: *"why is it
overlapping? Is it because there's all the cardinal directions and this one is up or
down?"* MEASURED, exits whose two rooms are in the same layout:

| exit kind | crosses a cluster boundary |
|---|---|
| compass move | 94 of 25,596 — **0.4%** |
| literal `up`/`down` | 800 of 1,179 — **67.9%** |
| `go door`, `climb stairs`, ... | 4,006 of 4,932 — 81.2% |
| script exit | 500 of 1,418 — 35.3% |

A plate cannot hold two rooms in one cell and cannot point a compass edge the wrong way, so
a vertical move — which carries no grid direction at all — usually has nowhere to go and
forces a new plate. **The correlation with floors is a side effect of that, not floor data.**

**Scored against the 553-exit key** (248 exits have both rooms in a layout):

| both rooms | same cluster -> same floor | different cluster -> changes level |
|---|---|---|
| outdoors | **99 of 99 (100%)** | 64 of 67 (95.5%) |
| indoors | 18 of 49 (36.7%) | 4 of 4, too few to score |

The author predicted the split before it was measured: *"inside and outside are different
beasts. When we get to interiors we're talking about a huge density in these towns."*
Outdoors the plate IS the terrain, so leaving it means leaving the ground. Indoors the
baker packs a dense town onto one plate, storeys and all.

**Their constraints, measured.** Occupancy is exactly 1.00 at both the median and the
maximum across all 2,762 clusters — no cluster ever puts two rooms in one cell. Of compass
edges inside a cluster, 22,841 land exactly one grid step in their direction and 1,682
stretch further in the same direction (891 do neither). **Zero violations in all 123
layouts.** They would rather strand a room than break a constraint: **1,501 of the 2,762
clusters hold a single room**, drawn beside the main plate as `adjacent`.

## 11p. A DESIGN PROPOSAL, not a finding: area level and world elevation are two numbers

This is the most important thing the night produced and it is the author's, reached while
looking at `[Kraken's Fall, Mantle's Landing]` `go stairs` -> `[Kraken's Fall, Beach]`, which
the cluster rule flattens and which is plainly a level change:

> *"yeah it is a level change but at the same time remember what we're making here. We're
> making visual representations of the area they're in. and stranding a beach room by itself
> on a level isn't condusive to that. Some things have to be fudged. Maybe there are no
> levels inside areas unless there needs to be and it really makes sense. Maybe levels is
> more of a world thing?"*

And, asked when an area genuinely needs a second plate: *"when it's too crowded on one
plate."*

**The proposal.** Two numbers, because one number has been asked to do two jobs all night:

- **Area level** — which plate to draw a room on. A RENDER decision. Split only when the
  plate cannot hold the rooms. A beach below a landing, a cliff top above a trail, a crow's
  nest above a deck: all one plate. Fewer levels is a better map.
- **World elevation** — how high the room actually is. Never splits a plate. For travel,
  for descriptions, for anything that needs real height.

**The evidence that area level should not be precomputed at all.** Simutronics' answer to
"too crowded" is not a threshold — it is a collision. Their baker splits a cluster the
moment a room cannot be given an empty cell whose compass edges point the right way. A
renderer for Hydra will face the same three constraints (cell collision, edge direction,
and — the author's third — **edges crossing**, which this data cannot confirm because a
stretched edge passing over an occupied cell is exactly what a crossing looks like) and will
make the same splits **from the room graph, without any floor number as input**.

**What this means for the numbers in 11n.** They are world elevation, and that is what they
are good for. The 148 conflicts and 240 uncrossed exits are elevation questions; most are
not rendering problems, because an exit that does not split a plate needs no direction to
draw the map. The metric that would matter for rendering is different again: **202 rooms
currently sit alone on their (location, floor) plate** — 105 isolated by the map's own
literal `up`/`down`, 59 by a stair word, 38 by something else; 122 indoors, 80 outdoors.
Some are right (`[The Firebird, Aftercastle]` is genuinely alone atop an airship); the rest
are a slope counted as storeys, and each is a rendering failure.

**The area unit, if one is wanted.** Official layout where one exists (123 areas, 48% of
walkable rooms), the mapdb `location` field otherwise (306 locations). They disagree: 66
official areas span several locations and 53 locations span several official areas, so
neither alone is right. Cross-referenced against `reference/wiki_clean/List of hunting
areas.txt` (112 areas): 64 match a location exactly, 29 are sub-areas inside one (Cavernhold
inside the foothills of Zeltoph, Lava Flows inside the volcano), 19 appear nowhere. Only two
locations truly hold two hunting areas — **Stone Valley** (Thanatoph + Stronghold, and it
spans floors −1..+8, so this one matters) and the Yegharren Plains (Orcswold + Black Moor,
all at 0). 10 areas have no anchor at all and would need an origin set by hand, 493 rooms:
the Rift (231), Maaghara Tower (61), Reim Fortress Defense (61), the bowels of Thanatoph
(57), Koar's Shrine (41) and five smaller.

**The Rift is not floors.** Its five official "planes" (202 rooms) are linked only by Ruby
scripts holding a 55-room list and a compass direction per room: you walk a compass
direction and which plane you land on depends on which room you were in. The links run
1->2->3->4->5->**1**. A cycle cannot be a stack of floors. The planes are a dimension of
their own; all 202 rooms belong at one elevation with the plane recorded as a separate
field.

## 12. Suggested next steps

- **Hand-check the 30 v2 mistakes** before any further prompt tuning. §7 argues several
  are the key's fault, not the model's, and tuning against them would be fitting noise.
  This is the highest-value next step and costs nothing.
- **Job 1 at scale** is a decision for the author, not a further trial. The scores are in.
- **The `17249 → 17250` ladder** deserves a look — see above.
- **Two untested ideas**, if more accuracy is wanted after the key is cleaned: a
  symmetry check (ask A→B and B→A, trust only consistent pairs — should kill the
  polarity inversions that are 26 of 34 mistakes, at double the cost), and a fan-out
  decomposition (a separate Noul asking whether the named feature is above the
  character). Neither was run.
- **Jobs 2 and 3 are untouched.** Job 2 needs the uid join through
  `map-data/prime/layouts.json`; job 3 needs a legend, which `plan/22` leaves to the
  author. Both can reuse `ask.py` and `score.py` unchanged apart from the criteria and
  the state builder.
