# Player Requirements — what using Cena must be like

Written 2026-09-18. This is the product document the
[gap audit](09-gap-audit.md) said was missing. Every other document in `plan/` describes how
Cena is **built**. This one describes what using it must be **like**, and it is grounded in
evidence rather than in the author's intuition.

The gap audit named three gaps with no evidence behind them at all:

- **§1.2 accessibility** — Tier 1, zero coverage in the entire plan
- **§2.1 the user-customization surface** — highlights, themes, layouts, keybinds, sounds
- **§2.2 triggers/aliases/macros** — what users get *instead* of scripting

Those are exactly the questions no source code can answer. Lich's source tells you what was
built; VellumFE's source tells you what its author wanted. Neither tells you what players
want. This document is the first input to the plan that comes from players.

---

## The source, and how it was read

`reference/discord/saga-thread.txt` — a thread from the official GemStone IV Discord titled
*"Saga: A New Front End"*.

| | |
|---|---|
| **Messages** | 4,979 |
| **Distinct posters** | 143 |
| **Span** | 2026-07-09 → 2026-09-18 (10 weeks) |
| **Volume** | July 1,034 · August 3,327 · September 618 |

**Saga** is a new **Electron/Chrome** front end for GemStone IV and DragonRealms, announced
7/9/2026, public beta 8/12/2026. The August spike is the beta. This thread is players
reacting to it in real time: gripes, requests, bug reports, comparisons to older clients,
and praise.

That makes it two things at once: **primary evidence about what this player population
wants**, and **competitive intelligence** on a modern client that shipped while Cena was
still a plan. What Saga got wrong is Cena's opportunity. What Saga got right is the bar.

### Method

Six analysts mined the thread independently, each under a different lens, and each was then
**adversarially verified** against the raw file. The verification mattered: it caught
inflated counts in every single lens, two cases where a complaint was reported as live when
it had been fixed weeks later, and several cases where a developer *answering* a question was
counted as a player *asking* one. **The counts in this document are the post-verification
numbers, recomputed directly from the transcript**, not the analysts' originals.

Four rules were applied throughout.

1. **Weight by distinct people, not message volume.** One person posting forty times about
   their pet issue is one data point. Five people independently raising the same thing is a
   pattern. Every count below is distinct usernames.
2. **Reactions are votes.** A message with seven reactions had seven people agree without
   posting. Reaction counts are given where they carry weight.
3. **Separate the voices.** Developer and staff statements are different evidence from
   player statements. See the roster below.
4. **Check recency.** A complaint that appears in July and stops by August was probably
   fixed. A complaint recurring across all ten weeks is structural. Several findings changed
   when this check was applied.

### Who is who

Getting this wrong inverts the meaning of a quote, so it was established from the text
rather than assumed.

- **`mirke` = GM Auchand**, Saga's lead developer. Established by self-reference
  ([7/10/2026 7:52 AM] mirke: *"Sometimes, Auchand Gets Approval"*) and by his answering the
  announcement Q&A directly. Auchand never posts under that handle.
- **`gs4nyxus` = GM Nyxus**, co-developer. **`theponzzz`, `gs4casil`, `whirlin`, `gmxynwen`**
  are Simutronics staff.
- **`lich_doug`** is Lich's maintainer. **`nisugi33`** is **Cena's own author**. **`horibu`**
  (243 messages) is a player acting as an unpaid support desk — his posts are mostly
  *answers*, so he is counted as a data point only where he states his own want.
- Everyone else is a player.

### Corpus corrections worth recording

- The brief said 151 posters. The true figure is **143**. A naive count gives 149 because six
  handles carry a ` (pinned)` suffix and double-count; `grep -cE '^\['` gives 5,000 because
  it catches bracketed lines inside message bodies.
- **Instruction-injection check.** Nothing in the transcript attempts to direct this work. The
  closest items are players pasting LLM output that ends *"Let me know if you want me to
  explore that"*, and moderators telling *Saga's users* to file feature requests in-app. Both
  are addressed to people in the thread, not to a reader of it. Treated as data, ignored as
  instruction.
- **No real names, emails or account names** appear here. Discord handles only, as they
  already appear in the transcript.

### What this sample is and is not

It is one thread, about one competing client, in one game, over ten weeks. It is **strong
evidence about what this population wants and weak evidence about anything else.** It
over-represents people for whom Saga works well enough to complain about it in Discord, and
it over-represents whatever broke most recently. The limits are set out in full at the end,
and they are load-bearing — one of the headline findings is a *silence*, and silences are
exactly where this kind of evidence is weakest.

---

## Headline findings

Seven things in this thread should change Cena's plan.

### 1. The accessibility requirement is not the one the plan assumed

The gap audit framed §1.2 around VellumFE's 821-line TTS module. **TTS is the least-demanded
accessibility feature in this entire corpus.** Two people asked about screen readers on the
first night; nobody mentioned it again in the following ten weeks. `NVDA`, `JAWS`,
`VoiceOver`, `Narrator` and `braille` return **zero hits in 25,255 lines**.

The real demand is **low vision** — magnification, contrast, and control over motion — and it
is driven by **age, not disability**. This is a 1988 game with a correspondingly old player
base. Accessibility here is a mainstream feature for the median user, not a minority
accommodation. Full evidence in [§A](#a-accessibility).

### 2. Nobody asked for a scripting language — which supports Cena's most contested decision,
but not for the reason the plan gives

[`08-curated-behaviors.md`](08-curated-behaviors.md) treats "no embedded scripting language"
as the risky call. In ten weeks, **not one player asks Saga to embed a scripting language**,
and **not one person asks for conditionals, loops or state in a macro**. The demand is for
*linear command sequences* and *macros*, both of which are pure configuration.

The threat to the decision is different and sharper than the plan anticipates: **Saga shipped
a scripting language anyway**, and a senior player argued in this thread that
*configuration breadth* — Cena's stated fallback — is itself the failure mode. See
[§P](#p-pressure-on-existing-cena-decisions).

### 3. Mobile is not validated as a driving constraint

**9 distinct people out of 143 (6%) mention mobile at all**, and most mentions are
incidental. Compare: 54 on highlights, 40 on macros, 76 on panels. The only substantive use
case named is *logging in quickly from a phone*, not playing. Nobody describes a session on a
tablet.

The single most uncomfortable data point for the plan: **the one person in the thread who
actually plays on a phone does it on VellumFE — Cena's own predecessor — and reports it
failing.** This does not kill mobile, but it means mobile is a **bet**, not demand, and the
plan should say so. See [§G](#g-mobile) and [§P](#p-pressure-on-existing-cena-decisions).

### 4. Electron's cost lands on CPU under multi-boxing — precisely Cena's target range

Saga is stable and memory-light. It is **CPU-expensive per session**, and players hit the
wall at 2–3 concurrent characters. Cena's stated range of 3–25 sessions sits *entirely inside
the region where Saga breaks down*, and the head-to-head comparison has already been made
publicly, by a player, in Cena's favour. See [§D](#d-performance-and-multi-session).

### 5. Highlights are not decoration — they are the primary perception channel for combat

Players do not read combat text. They pattern-match colour. This is stated flatly by a
speed-focused power user, by a second power user, and independently by a legally blind
player who arrived at the same technique out of necessity. That converges accessibility and
performance onto **one feature**, and it reframes the highlight engine from a cosmetic
setting into the client's main readout. See [§B](#b-customization-highlights-themes-layouts).

### 6. Maximal customization is itself a failure mode

Ten distinct people reported being *overwhelmed* by Saga's settings, including two staff
members. Saga's own lead developer conceded it publicly, twice. A plan that treats §2.1 as
"expose more knobs" reproduces this failure exactly. The thread's own answer is **curated,
shareable presets** — which maps cleanly onto Cena's data-profile model.

### 7. Migration from the old client is an adoption gate, not a convenience

Veterans carry 25–30 years of highlights and muscle memory. The thread contains people
stating plainly that failure to import is why they do not use a given client — and one case
where a partial import (53 of ~100 macros silently skipped) was described as *"crippling"*.
Note the migration sources are **Wrayth, WizardFE and Warlock** — *not* StormFront/Genie.
See [§H](#h-migration-from-older-clients).

---

# Requirements by area

---

## A. Accessibility

**This is the Tier-1 gap. The evidence overturns the plan's framing of it.**

A keyword search for `accessib*` returns 8 relevant hits from 8 people. That is the wrong
measurement and it would produce the wrong requirements list. The *substantive* accessibility
demand in this thread is several times larger and is phrased as *"failing eyesight"*,
*"squint"*, *"distracting"*, *"too bright"*, *"seizure animations"*. **A keyword-driven
accessibility audit of this corpus would have missed almost all of it.** That is itself a
methodological finding: Cena cannot gather accessibility requirements by asking who needs
accessibility features.

### A.1 Screen readers and TTS — asked twice, then ten weeks of silence

**2 distinct people, both on the first night.**

> **[7/9/2026 7:56 PM] onar:** "I don't want the question to get lost from twitch. but I'm
> hoping accessibility features are covered during the talk." *(👆)*

> **[7/9/2026 8:14 PM] cidraco:** "1. What accessibility options/features will it have?"

The developer answered within ten minutes:

> **[7/9/2026 8:24 PM] mirke:** "1) Screen reader mode + ARIA interfacing" *(❤️)*

**That is the entire screen-reader content of 4,979 messages.** Nobody asked whether it
shipped. Neither developer mentioned it again. `NVDA`, `JAWS`, `VoiceOver`, `Narrator`,
`braille` — zero hits across 25,255 lines.

**A correction worth recording, because an earlier analysis got it backwards.** mirke's
[7/12/2026 12:29 PM] *"I have not added the ability to do a Speech mode in Saga"* is **not
about text-to-speech.** It replies directly to soleblaze two minutes earlier —
*"Most annoying thing about the WebFE was the speak vs command button"* — and concerns an
**input mode** that routes typed text to the speak verb. It is not evidence that a developer
declined TTS, and must not be cited as such.

**INFERRED, and the inference is weak in a specific way:** a blind player cannot easily
participate in a Discord thread about a client they cannot use. The 143 posters are close to
a definition of "people Saga works for." This silence is partly a selection effect and is
**not** evidence that blind GemStone players do not exist.

**Recommendation.** Keep VellumFE's TTS module — it is already written and cheap to carry.
**Do not treat it as the accessibility story, and do not let it absorb the accessibility
budget.** The requirements this thread actually generates are A.2 through A.6, and they are
about low vision and motion.

### A.2 Low vision is the real demand, and it is about scaling — ~9 distinct people

**17 distinct people discuss font size or zoom.** Narrowed to people reporting actual
difficulty reading, the honest count is **~9**. At least five tie it explicitly to aging or
impaired vision:

> **[8/14/2026 2:17 PM] pukk:** "I have a question about font size changes. **I change the
> font sizes to adjust for my failing eyesight.** But when I change the font that handles the
> experience or info in the story window, it also changes the panel for experience too. **Can
> you add a font override to that panel? I notice some panels don't have font overrides.**"

> **[8/12/2026 6:47 PM] 0mrii:** "Hey, Zoom 150% for old man. Good, good." *(🤣 ×3)*

> **[8/18/2026 5:25 PM] paragonlost:** "Its very distracting. **As I've gotten old, my vision
> is more impacted by things like that** then they used to be."

> **[8/17/2026 4:54 PM] gooberosity:** "I went back to 100% zoom a few days back then opted
> instead just to **override font sizes on the particular windows I no longer wanted to squint
> to read.**"

Saga's failure mode is that font override exists on *some* panels and not others, and presets
are all-or-nothing:

> **[8/16/2026 11:01 AM] thehouseofklaus:** "Cannot adjust the Font Size of an individual
> Preset. It's all or nothing"

> **[8/31/2026 7:33 PM] misaroe:** "Has anyone found a way to manipulate the font size of the
> vitals bars? I can't find anything in Panel fonts." → **[7:36 PM] atinygnome:** "Mine is so
> tiny"

It also **regressed**: [9/8/2026 9:41 PM] drigler asks for per-panel link font control back
because *"It was like that a few patches ago, and now it's not."*

Two adjacent levers were raised and are easy to forget:

> **[8/13/2026] neovik:** "I reduced all my line spacing down to the lowest possible setting
> but in one of my characters story windows he's getting a lot of line spacing still at times."

> **[8/16/2026] whirlin:** "I'm more tilted by not adjusting the font size to wrap to a single
> row"

At 27-point type, **line-height and wrap behaviour are as load-bearing as font size** — they
determine how many lines fit and whether one logical line becomes three.

> **R-A1 — Universal, independent text scaling.** Every text-bearing surface — story, room,
> speech, thoughts, vitals, status, effects bar, links, highlights, hotbar labels, tooltips
> **and the settings UI itself** — exposes an independent font-size override. No panel is
> exempt. Line-height and wrap are user-controlled alongside size. Icon and glyph sizes scale
> with their associated text. Scaling is a **first-class layout input**, so a 27pt user gets a
> working layout rather than a broken one. This holds across TUI, GUI and web.

### A.3 The two visually impaired players — the most concrete requirements in the corpus

Two people self-identified and volunteered as testers within nine days of the announcement:

> **[7/14/2026 7:19 PM] zenaxa:** "Hello, heard on the Twitch replay you needed visually
> impaired players to test, potentially, the new FE Saga. **Legally Blind**, been away for many
> years so kinda newish, reporting in. How canI help?" *(🎉)*

> **[7/18/2026 2:59 PM] popthesmokes:** "im legally blind in my left eye.. i can dual test!"
> *(🎉)*

mirke replied warmly ([7/17/2026 11:40 PM] *"Awesome. Would love that!"*). **INFERRED: nothing
visible came of it.** zenaxa posts once more, on 8/9, and then disappears from the thread. No
accessibility outcome is ever reported.

But that 8/9 conversation is the densest accessibility passage in the thread, and it yields a
requirements list better than any guess:

> **[8/9/2026 2:22 PM] zenaxa:** "This is what Vision Accessibility can look like, or worse."

> **[8/9/2026 2:34 PM] zenaxa:** "My screen is about 44" and with "normal" font or average if
> you prefer, **it's like that tiny phone text for me.**"

> **[8/9/2026 2:33 PM] zenaxa:** "I do miss some things, esp in busy noisy rooms. **Sound
> alerts help**, and this speech window will help a lot."

> **[8/9/2026 2:37 PM] zenaxa:** "Yes, **very much using any other indicator you can that
> isn't reading.**"

She also states the routing-vs-filtering distinction precisely:

> **[8/9/2026] zenaxa:** "combat text could be moved to a side panel... which with 27 sized
> font is fast and furious, yo! So if I could **move** the combat text, **not squelch or
> minimize it** - I do still like to see the full roll"

**Three requirements fall out of that, none of which involve a screen reader:**

> **R-A2 — Routing is not filtering.** Sending a class of text to its own panel is a
> categorically different operation from suppressing it, and low-vision users need the first,
> not the second. Cena's typed-event model makes this natural: route by event class, retain
> everything.

> **R-A3 — Non-visual channels are primary, not garnish.** Sound must be bindable to game
> events, not only to text matches. See A.5.

> **R-A4 — Scroll velocity is the real enemy.** At large font sizes the screen scrolls faster
> than it can be read. Density controls — aggregation, routing, summarization of typed events
> — are an accessibility feature, not only a convenience.

**And note the disconfirming evidence, which the "what Saga got wrong" framing would filter
out:** zenaxa also said [8/9/2026] *"Im super grateful an FE that can handle this stuff is
being put up! Let's me play again. I'm excited!"* **Ordinary customization — font size, sound
alerts, panel separation — delivered a real accessibility outcome.** That is the bar, and it
was cleared with configuration rather than with assistive-technology integration.

### A.4 Motion and animation — ~7 distinct player requests, never called "accessibility"

Saga's headline feature is dynamic per-room theming with animated ambient backgrounds. It is
the **single largest source of accessibility complaints in the thread**, and **not one person
framed it as accessibility**:

> **[8/13/2026] bobandy2996:** "how do you turn off the seizure animations when getting hit
> casting etc"

> **[8/18/2026 5:25 PM] paragonlost:** "Its very distracting. As I've gotten old, my vision is
> more impacted by things like that then they used to be."

> **[8/25/2026] puptilian:** "I found the transition between environments that change the
> background ambient really slow things down for me. **Turning off ambients was a huge help**"

Motion complaints outnumber screen-reader mentions roughly **3–4 to 1**, and a keyword search
for `accessibility` finds none of them.

**Important precision, since an earlier analysis overstated this:** ambients *can* be
disabled — [9/17/2026] mirke: *"You can disable immersives or have them enabled"* — and
pachuu resolved his own opacity problem that way. The real defect is narrower: **specific
themes override user settings regardless of the ambient toggle**, and several individual
animations shipped with no toggle at all.

> **R-A5 — Motion is user-controlled, and the control is honoured everywhere.** Every flash,
> glow, shake, pulse and ambient animation has an off switch. A theme may never re-enable
> motion the user disabled. On web, honour `prefers-reduced-motion` automatically — **zero
> people in this thread knew that standard existed**, so expecting them to find a checkbox
> will fail.

### A.5 Sound as a substitute channel — and the exact ceiling Saga hit

The single cleanest statement of the requirement in the corpus:

> **[9/8/2026 12:20 PM] ravenwraithstudios:** "is there a way to make it **trigger when you
> take enough damage to be at 50% or less total health** (like in some games where it causes a
> heartbeat when your low health)? I know there's screen flash when you take damage just dont
> know how to translate that into an **audio cue**."

The answer states Saga's ceiling exactly:

> **[9/8/2026 12:30 PM] horibu:** "**Saga native sound alerts can only trigger on txt
> highlights.... currently.**"

> **[9/8/2026 12:33 PM] izzy101yzzi:** "Would also be fairly trivial to create a Lich script to
> output some text when you're <= 50% total HP, **which you could subsequently highlight and
> add a sound to**"

That exchange is the whole product problem in three messages: **text-pattern configuration
cannot express a state predicate**, so the workaround is to have a script *synthesize a text
line* for the configuration to match. Corroborated independently:

> **[8/31/2026 9:23 PM] thebardess:** "Is there a way to set Saga to play a sound when an
> incoming announcement is made? Since Announcements have no standard text formatting, this
> cannot be achieved with a highlight."

> **R-A6 — Alerts bind to typed events, not to rendered text.** `hp <= 50% → play sound` must
> be ordinary configuration. Cena's parser already extracts vitals, roundtime, spell effects
> and creature status; the trigger condition space is that typed state. This is the single
> largest capability gap between Saga and what Cena can offer, and it is a **gift, not a
> warning** — see [§C](#c-automation-triggers-macros).

### A.6 The settings UI is itself an accessibility surface

> **[8/12/2026 9:04 PM] misaroe:** "The accessibility (or lackthereof) there is killing me,
> sir." — about a click-only spinner with no type-in field.

Multiple people could not find settings that already existed. **INFERRED: the configuration
surface needs the same accessibility bar as the game view**, which §2.1 of the plan does not
currently state.

### A.7 Silences — and they are load-bearing

Reported because absence is informative, and because two of these change how requirements
should be justified.

| Searched | Hits | Reading |
|---|---|---|
| `colorblind`, `colour blind`, `deuteran`, `protan`, `tritan` | **0** | In a thread saturated with colour-highlight discussion, and where colour *is* the information channel (A.3, B.3), nobody once raised colour-vision deficiency. **INFERRED:** affected players are likely self-serving via fully user-configurable highlight colours. That suggests **full user colour control is the accessible design**, not a curated "safe palette". |
| `epilepsy`, `photosensitiv`, `seizure` (clinical), `migraine`, `vestibular` | **~0** | The motion kill-switch demand in A.4 was framed entirely as *distraction*, never as a medical need. Justify it on usability grounds; do not claim a photosensitivity mandate this corpus does not support. |
| `tremor`, `arthritis`, `carpal`, `RSI`, `one-handed` | **0** | Despite heavy macro/hotkey discussion, no motor-accessibility framing at all. The nearest is ergonomic: [8/27/2026] thehouseofklaus *"I changed my eloot macro to something simpler and my hands already feel better"*. |
| `dyslexia`, `OpenDyslexic` | **0** | No reading-disorder requirement surfaced. |
| `WCAG`, `a11y`, `aria-label`, `tabindex`, `focus order` | **~0** | `ARIA` appears exactly once — in the developer's own 7/9 answer. No standards vocabulary in this population. |
| `prefers-reduced-motion` | **0** | Nobody knew it existed, while ~7 people asked for its behaviour. Hence R-A5's "automatically". |
| Captions/subtitles for audio features | **0** | Saga ships music, ambient sound and sound alerts with no text equivalent, and no Deaf/HoH player raised it. **Unevidenced either way.** |
| Braille displays, magnifiers, switch access, eye tracking, voice control | **0** | No assistive hardware mentioned anywhere. |

**The honest summary of §A:** this corpus gives Cena an excellent, specific low-vision and
motion requirements list, and **tells us almost nothing about blindness, colour vision,
cognitive accessibility or assistive hardware.** Those need their own primary research. Do not
let the richness of the low-vision evidence disguise the emptiness of the rest.

---

## B. Customization: highlights, themes, layouts

This is §2.1, and it is the largest topic in the thread by a wide margin: **76 distinct people
discuss panels** (359 messages), **54 discuss highlights**, **30 layouts**, **25 themes**.

### B.1 Global vs per-character settings — the #1 structural request, unresolved for ten weeks

**11 distinct people**, and it is the **second message in the entire thread**, two minutes
after the announcement:

> **[7/9/2026 7:53 PM] ruseoffools:** "Hopefully it's a lot easier to share highlights and
> color schemes between characters and accounts"

> **[7/11/2026 1:31 AM] calista4817:** "I also kinda like the ability to do global highlights
> like in Warlock, so you can have certain highlights across all characters and others that are
> character-specific" *(💯 ×3)*

> **[8/27/2026 10:49 AM] korpacz:** "Is there a way to apply global highlights? In warlock I
> can set highlights up globally or cliff any character so when I log into another character
> those global highlights already apply. Eg. 'You' is a global highlight - I think in saga I
> need to create this highlight for each character rather then doing it once globally."

> **[9/7/2026] archtsean:** "But there's no global you'd have to do it for each highlight,
> correct?" *(💯)*

A complaint that appears on 7/9 and again on 9/7 is **structural, not a bug**. Saga's answer
throughout was export/import — [7/9/2026 7:54 PM] theponzzz: *"Exporting and importing is very
thorough."* (💯). Players consistently treated that as the problem, not the fix.

Crucially, **the design question was asked at SimuCon and left unanswered.** From the community
FAQ, quoting GM Auchand and relayed into the thread by horibu [7/19/2026 8:16 PM]:

> "It's certainly doable. [...] I can certainly see the value of globals. **The question is,
> then, does specific trump global.** [...] I guess it should override. I think with that
> hierarchy understood there's probably a good chance we can do that."

> **R-B1 — A settings hierarchy from day one: global → profile → character, with the more
> specific level overriding.** Not a later migration. Import/export of a flat per-character
> blob is the thing people are complaining about. This applies to highlights, macros, layouts,
> themes, sounds and squelches alike. For a 3–25 session client it is not a nicety; it is the
> difference between configuring once and configuring twenty-five times.

### B.2 Highlights at veteran scale — the failures are list management, not matching

The counts in this population are far larger than a plan would guess:

> **[8/29/2026 11:35 PM] .demandred:** "I have um... hang on. **1579 highlights.**"

> **[9/7/2026 7:19 PM] dwgemster:** "I ditto this sentiment, I have **over 1500** highlights
> (blush), scrolling is my life."

ktig reports 184, plus *"25 years of ignores"*. ktig also hit an **Add Highlight button greyed
out** because the dialog overflowed the window at non-100% zoom. The reported failures are
overwhelmingly about **navigating and maintaining a list of a thousand entries**, not about
matching:

> **[9/6/2026 7:45 PM] misaroe:** "Scroll down to the bottom of your highlights, you may have
> an unresolved 'new highlight' already down there. **I wish new highlights were added to the
> top instead**, to prevent this confusion." *(💯 ×4)*

> **[8/16/2026] misaroe:** "whoever added the **duplicate handling**, I love you ❤️"

Power features are actively wanted and demonstrably reduce burden. **Regex** was requested
pre-launch by zedarius, shipped, and then measured:

> **[8/15/2026 10:25 AM] ruseoffools:** "The new highlights are a little bit to work through
> but **having the ability to use regex is condensing my list by huge amounts**"

**Groups with inherited formatting** followed the same arc — requested 7/11, shipped ~8/18,
immediately praised:

> **[7/11/2026 2:10 AM] zedarius:** "**Highlight groups** would be awesome too. Set a group for
> Scrolls, and then add each scroll noun to the group and they inherit the formatting. Or I
> have like 70 different mob death highlights, and unifying them would be amazing. **Regex**
> would be awesome. And the ability to **toggle notifications per highlight**"

> **[8/18/2026 6:06 PM] ruseoffools:** "Grouped highlights is something I love but didn't know
> I wanted."

Saga also supports **capture-group-only highlighting** — matching on full context but colouring
only the captured span. That is a genuinely advanced feature worth matching.

> **R-B2 — A highlight engine built for 1,500+ entries.** Regex including capture-group-only
> colouring; named groups with inherited formatting; duplicate detection; sort and search;
> new-entry-at-top; a compact list view; per-highlight sound and notification toggles; live
> preview of matches. Performance and **navigability** at scale are the requirement.

### B.3 Highlights are the combat HUD — and this is the deepest finding in the thread

Three people, arriving independently from different directions, describe the same technique:

> **[8/3/2026 7:01 PM] izzy101yzzi:** "ScreenScrollStone is the name of the game. **If you know
> what is happening during combat by reading, you're going too slow. Gotta interpret by color**"

> **[8/9/2026 2:36 PM] rysk:** "It really comes down to **using highlights to interpret what's
> happening without reading any of it**" *(⬆️)*

> **[8/9/2026 2:37 PM] zenaxa** (legally blind): "Yes, very much using any other indicator you
> can that isn't reading."

Two sighted power users optimizing for **speed**, and one low-vision player compensating for
**necessity**, converge on the identical strategy. And the cost of building it by hand is
stated explicitly:

> **[7/29/2026 1:42 PM] rysk:** "I'd also love if there was some way for Death Messages, as well
> as Prone/Stun messages to be flagged. **90% of my custom highlights are me just highlighting
> stuff so i can decode whats going on in the screen scroll as it zips by, but each one is
> custom**"

Players are **hand-building thousands of string patterns to reconstruct semantics the client
already receives in the protocol.** Related asks:

> **[7/29/2026 1:40 PM] izzy101yzzi:** "it would be nice if a given string could be tagged as
> **'generated by me.'** One possible use case being a flare message without a subject."

> **[7/17/2026 5:19 PM] jenuinejenanigans:** "Now I just want to be able to have **all herbs a
> color, all gems a color**, etc, and overridden by highlights."

> **[8/12/2026 8:15 PM] .pbandjeep:** "is it hidden in the FE (like wrayth) or can you insert
> the color codes directly into the XML. **Unfortunately I'd not use a FE that doesn't give
> some flexibility for highlighting via item types.**" — a stated adoption condition.

> **R-B3 — Semantic highlighting on typed events, with string highlights as the fallback.**
> Cena's parser emits typed events; users should be able to style **by event class and actor**
> — damage, death, flare, stun, prone, item type, mine-vs-theirs — not only by string. Most of
> those 1,500 hand-written highlights become unnecessary. **This is simultaneously a
> performance feature, an accessibility feature (A.3) and the single strongest product argument
> for the typed-event architecture the plan already has.**
>
> **Corollary for accessibility:** because colour is a primary information channel here,
> redundant encoding — colour **plus** text or sound — is the right accessibility answer, not
> removing colour. Naive "don't rely on colour" advice would break the thing that makes the
> client usable.

### B.4 Highlights everywhere, not just the story window — 2 people + reaction votes

> **[8/18/2026 7:22 PM] karmicinjustice:** "Anyone know if they are going to make highlights
> work in the inventory panel?"
> **[8/18/2026 7:22 PM] zedarius:** "yeah, would love highlights in all the different panels
> and windows"
> **[8/25/2026 12:25 PM] karmicinjustice:** "Yeah, I was just hoping highlights would work
> throughout the whole interface." *(💯)*
> **[8/25/2026 3:11 PM] zedarius:** "+1 for allowing highlights everywhere." *(👍)*

Only **2 distinct askers** — stated honestly — but the architectural implication is
disproportionate to the vote count:

> **R-B4 — Highlighting is a property of the shared text-rendering layer, not a feature of the
> story view.** Every panel that renders game text applies the same highlight pipeline. Cheap
> if designed in, expensive if retrofitted.

### B.5 Themes — the user's contrast settings must win

**25 distinct people on themes.** Saga's per-room dynamic theming is its most-loved *and*
most-complained-about feature:

> **[7/25/2026] 0mrii:** "Still getting used to the dynamic themes, Cold River was a nice dark
> and all of a sudden I pop my tent to **a blinding sand color** 😆"

Three distinct people report themes destroying readability, and ~6 more report the panel
opacity slider having no effect under certain themes. **Recency check: this was being actively
worked on at thread's end** — [9/8/2026 7:10 PM] mirke: *"BTW, been working on some theme
updates to make them more readable and cleaner."* So it is a **live defect being fixed**, not a
permanent failure.

A separate, narrower defect that Cena should learn from directly: Saga's CSS dimmed all
Lich-origin output with a hardcoded `.line-lich { opacity: .6 }` and no user toggle —
third-party tool output visually second-classed by the client. Diagnosed by ruseoffools on
8/15 and **fixed by 8/27** ([8/27/2026] ruseoffools: *"It looks like the lich reduced opacity
was fixed unless I'm crazy"*; misaroe the same day: *"ALSO! Fixing the transparency of lich
commands!"*). Cena merges proxy and frontend, so this whole class of "whose text is this and
why does it look different" simply does not arise — provided no styling is hardcoded.

Also wanted: a **user-owned custom theme slot**, because editing a built-in theme destroys it
(3 distinct people, with 💯×2 + ❤️ on the request).

> **R-B5 — User contrast and colour settings are architecturally superior to any theme.** A
> theme, ambient or immersive effect may never override a value the user set explicitly. Custom
> themes are saved separately from built-ins. Automatic/contextual theme switching is **opt-in**.

### B.6 Panels and layout — Saga's best work, and the bar

> **R-B6 — Every stream is a panel; every panel can be a tab, a pane, or its own OS window on
> another monitor; each carries its own font, background and trim; the whole arrangement is one
> exportable artifact.** Detail and evidence in [§I](#i-what-saga-got-right).

Note this is the **hardest thing to port to a TUI**, and the plan should solve that
deliberately rather than discover it late.

### B.7 Customization silences — several of which should stop work from happening

| Searched | Hits | Reading |
|---|---|---|
| `dock`, `docking` | **0** | Nobody in 4,979 messages uses docking as a concept. Players think in *panels*, *pull-outs*, *workspaces*, *move to new window*. **Building an IDE-style docking framework solves a problem in vocabulary players do not have.** |
| `contrast` | **0** | Contrast problems are described entirely as *"blinding"*, *"can barely read"*, *"too bright"*. Another case where the clinical keyword finds nothing. |
| `dark mode` | **1** | Dark is simply assumed — every shipped theme is dark. The actual minority ask is the opposite: [8/6/2026] soleblaze *"Hopefully Ya'll have some light colored themes for us crazies."* **Build theming, not "dark mode".** |
| CSS / stylesheet theming API | **~1** | `css` appears once, in a bug diagnosis, never as a request. **Players want GUI controls, not a style language.** This contradicts the instinct to expose theming as text config. |
| Theme marketplace, cloud sync of settings | **0** | Sharing is expected to happen by handing a file to a friend. No infrastructure wanted. |
| Multi-monitor as a want | **3** | Raised by 3 people and satisfied immediately by "Move to New Window". Low-cost, high-satisfaction, **not a differentiator**. |
| Text-editable config files | **~1** | Despite a very technical population (people decompiling Saga's JS, writing regex, running Ruby), essentially nobody asked to hand-edit config. Worth weighing against Cena's YAML/TOML profile model — see [§P](#p-pressure-on-existing-cena-decisions). |

---

## C. Automation: triggers, macros, scripting expectations

This is §2.2 — what users get **instead** of scripting. The evidence here is unusually clean,
and it is the most decision-relevant section in the document.

### C.1 The demand curve is bimodal with an empty middle

**Mode A — linear command sequences. The mass market.** Saga's `CHAIN` is the most
enthusiastically received automation feature in ten weeks. **19 distinct people** discuss
`CHAIN`/`FOREACH`.

> **[8/20/2026 11:39 PM] theponzzz:** "Highly recommend using CHAIN. Super useful."
> *(❤️, facts ×2)*

> **[8/20/2026 8:23 PM] kederijan:** "or, you could set up a macro instead and use Saga's chain
> command. So for example make F12 = `chain[stance off][attack][stance def]`"

The grammar is deliberately minimal — and, critically, **game-state awareness lives in the
engine, not in user config**:

```
CHAIN [command1][command2][etc..]     - Fire the commands in order
CHAIN {#} [command1][command2][etc..] - Fire the sequence, repeating it {#} times
CHAIN STOP                            - Stop the running chain (or foreach)

Commands are sent 250ms apart, or as the game responds to each one, whichever
is slower.  If a command invokes roundtime, the chain waits for it to expire
before continuing.
```
*(posted [8/24/2026 4:06 PM] izzy101yzzi; note the command delimiter is user-configurable —
zedarius posts the same help text with `;` where izzy101yzzi has `[ ]`)*

`FOREACH` extends it to **set operations over a typed object model** — which is precisely
Cena's curated-behaviour thesis, already validated in production:

> **[8/13/2026 9:22 AM] izzy101yzzi:** `foreach aortic amber in cloak [get item][trash item]`
> → `[foreach: done, 12 items]` — "Pretty, pretty cool!" *(😍)*

**Mode B — arbitrary Ruby. The expert tail.** These users are not asking for a DSL. They have
Lich and use it inline:

> **[8/27/2026 5:16 PM] rysk:** "an alias like `groupall =>;e GameObj.pcs.each { |pc| fput
> \"group #{pc.noun}\" }`"

> **[8/17/2026 12:37 PM] izzy101yzzi:** "Alternatively you can use an inline Lich script if you
> need fancier things ... `;eq multifput('stance offensive', 'attack')`"

**Nothing sits between the two modes.** A deliberate search for `conditional`, `if/then`,
`boolean`, `variable`, `loop` and `state` in the context of user-authored automation returns
**zero people asking for logic in a macro**. Every `variable` hit is a Wrayth-style `%placeholder%`
substitution or an unrelated sense of the word.

> **INFERRED, and this is the central finding of §C:** a config-expressible **sequence
> primitive** plus an **escape hatch** spans the entire observed distribution. A mid-tier
> scripting DSL would serve almost nobody — and that is exactly the space Saga's own built-in
> script engine occupies, and exactly where it generates complaints (C.4).

### C.2 Triggers: the dog that did not bark

**This is the strongest single piece of evidence for Cena's no-scripting decision.**

`trigger` appears on **19 lines out of 25,255**, across 13 people — and nearly all are other
senses of the word: being "triggered", GPU repaints, weather ambients, DOM layout. Genuine
"when X appears, do Y" requests come from **~2 distinct people in ten weeks**:

> **[9/8/2026 12:20 PM] ravenwraithstudios:** the low-HP heartbeat request quoted in full at
> [A.5](#a5-sound-as-a-substitute-channel--and-the-exact-ceiling-saga-hit).

> **[9/14/2026 1:03 PM] izzy101yzzi:** "Like, maybe I would want to create a trigger that
> identifies when I'm on fire and turn on that fire background 😆" — explicitly cosmetic.

One person was **surprised the capability existed** — surprise, not demand:

> **[7/29/2026 2:48 PM] calista4817:** "Whoa! this supports triggers? nice!"

The same pattern appears in a completely different feature area, which corroborates it. On
notifications (**10 distinct people**), the sharpest request is for a **predicate over parsed
spell-effect state**, not a text trigger:

> **[7/11/2026] on toasts:** "Is there a way to specify toasts by more specific criteria... I
> have 2 countdowns in my buff window. **I want a toast for the necro ring expiry, but I don't
> need to know every time I lose a COL sign.**"

> **R-C1 — Trigger conditions bind to parsed state, not to regex over rendered output.** Vitals,
> roundtime, spell effects, creature status, injury, encumbrance, item class. `hp <= 50% → play
> sound` and `effect 'necro ring' expires → toast` are then ordinary configuration, and the
> entire "script synthesizes text so config can match it" workaround disappears. **Text-pattern
> triggers remain as a fallback for what the parser has not yet classified.**

### C.3 Macros and hotkeys — the real demand, and it is pure configuration

**40 distinct people.** This dwarfs triggers, aliases and scripting-language requests combined.
The requirement is not "support macros" — it is **exhaustive modifier coverage and exact
muscle-memory preservation.**

> **[8/16/2026 1:25 PM] paragonlost:** "I want complete control of what I have in my hot keys,
> whether it be alt, shift, ctrl, number keys, F keys, numpad, alt-shift or ctrl-shift. Do we
> have this currently?"

> **[8/16/2026 2:01 PM] _nidal:** "it does support macros to the same degree Wrayth does, yes?
> You can have multiple sets, can have ALT or CTRL variations of them on any alpha key, etc?
> **Huge step back if not. I have zero percent chance at changing my macro muscle memory at
> this point**"

> **[8/27/2026 9:04 AM] thehouseofklaus:** "**I play GS like a musical instrument okay. I don't
> hit macros I key chords.**" … "I have alt-shift-ctrl-numpad macros. So I can just keep my
> hands in one spot."

Four structural lessons, each with a clear requirement:

**(a) Macro sets need a global/default layer.** Saga made sets mutually exclusive; Wrayth
layered them:

> **[8/16/2026 2:13 PM] thehouseofklaus:** "Wrayth let the Default macro set (Macro Set 0)
> basically function like a Global set. You could then have sets 1-9 work on top of it."
> **[8/16/2026 2:14 PM]:** "A macro set in the Default (Macro Set 0) wouldn't stop functioning
> if you switched to a different set. It was nice for using the different macro sets for
> different characters"

**(b) Never reserve a chord the user cannot reclaim.** The most sustained single-user pain in
the thread, and it is a *self-inflicted* wound rather than an Electron one:

> **[8/19/2026 2:39 PM] paragonlost:** "Its a real PITA that Saga will re-populate those macros
> it wants to have in it's system once you log out and back in. … One which opened one of the
> panels was control H. **Which I've been using as my universal Hide macro since 1992.** … So,
> its very stubborn about insisting that those commands be in the macro set somewhere. …
> **really drains my desire to deal with the other nine characters.**"

**(c) Sort keybinds by key.** A trivially avoidable defect that became a heavy user's number-one
request — and was **fixed in ~9 days**, which is a data point about how cheap this class of fix
is:

> **[8/18/2026 3:58 PM] thehouseofklaus:** "**Keybinds should be sorted by the key. It is the
> way.**" *(💯 ⭐)*
> Resolved — **[8/27/2026 5:20 PM] paragonlost:** "Thank you for adding the Hot Key/Macro Keys
> sort option A to Z."

**(d) Mouse buttons and hotbar buttons are macro targets too.** **16 distinct people** discuss
the hotbar, and the single most-celebrated automation in the whole thread needed no scripting
at all:

> **[9/8/2026 11:25 PM] ubp:** "I've configured **the entirety of my combat to be keyboard
> free**. I use my third mouse button to swap targets ... and then **my hot bar mirrors 10
> macros mapped to the 10 side buttons on my mouse** (the logitech 600 MMO mouse) ... It's
> really made combat reminiscent of my World of Warcraft experience." *(❤️ ×4)*

**The aspirational endpoint players name is a WoW action bar, not a script engine.** And note
the seam: **a keybind, a hotbar button and a chain step are three bindings of the same
underlying command action.** Saga treats them as three subsystems, which is why players had to
ask *"is there any way to have Macros activate the Hotbar keys, or is the Hotbar meant to be
click only?"*

> **R-C2 — One command-action model, bound from many surfaces.** A named action is invocable
> from a keybind (any chord, including Alt/Ctrl/Shift × numpad/F-keys/letters), a hotbar button,
> a mouse button, a chain step, or a behaviour. Keybind scopes layer: **global → profile →
> character**. **No chord is reserved by the application.** Cena never silently reinstates a
> binding the user removed. Keybind lists sort by key.

### C.4 Saga's built-in script engine — and why Cena should not copy it

Saga ships a Wizard-compatible script language. The most complete public documentation of it in
existence is in this thread, reverse-engineered by Cena's own author:

> **[8/21/2026 11:16 AM] nisugi33:** "`SOUND / PLAYSOUND` — play a sound file. `LOGLINE <file>
> <text>`. `SETVARIABLE name value / DELETEVARIABLE name` — script variables; they're
> substituted into every typed command before dispatch. … Wizard-style verbs: `put, echo,
> pause, wait, waitfor, waitforre, move, nextroom, goto, match, matchre, matchwait, counter
> (set/add/subtract/multiply/divide)`, plus `sound` and `logline` reused inline, and `if_N`."

That is labels, `goto`, pattern-matched waits, arithmetic accumulators and argument-count
branching — **a real, if archaic, programming language.** Simple scripts are stored as flat
command lists inside JSON:

> **[8/13/2026 6:46 AM] soleblaze:** `"body": "stance off\nsay I'm gonna attack!..."` —
> "editing it is doable, but painful. especially if you're not used to dealing with large json
> files"

**And it is a persistent source of unhappiness.** Saga's developer effectively admitted the
compatibility surface is unspecifiable, and asked the community to write the spec:

> **[9/2/2026 12:34 AM] mirke:** "It feels like no matter how I change scripting ... there's a
> group that feels like the scripting is buggy or otherwise not a perfect fit for their scripts."
> **[12:35 AM]:** "**Can someone write up a spec on the more fractious parts of writing and
> running scripts so we can get some agreement on what right looks like?**"

**No spec appears in the following 16 days.**

> **INFERRED:** Wizard-script compatibility is a long tail of undocumented edge-case semantics,
> not a feature. This is a strong argument for Cena **not** to implement a legacy-compatible
> scripting dialect — the compatibility target cannot be written down, so it can never be met.

### C.5 Lich is the assumed substrate, and the counts understate it

**53 distinct people mention Lich**; broadening to `lich|script` gives **72 of 143 (50%)**.
Lich compatibility was the *second question asked in the thread* and the first thing the
developers answered:

> **[7/9/2026 8:33 PM] lucu11an:** "I gotta ask if there will be built-in support for things
> like Lich, or will it at least allow Lich to hijack the xml feed as in FEs past?"
> **[7/9/2026 8:33 PM] gs4nyxus:** "**It works with lich**" *(Hype ×4)*

And the count is a **floor**, because moderators repeatedly redirected Lich discussion out of
this channel ([8/12/2026] lich_doug: *"I would really appreciate it if we could move Lich
questions to the scripting channel."*).

But the direction of travel is the interesting part. Saga is **displacing** Lich for casual
users:

> **[7/29/2026 1:05 PM] horibu:** "if you only relied on Lich for go2/map functionality, you
> could 100% leave behind Lich."
> **[7/29/2026 12:59 PM] gs4casil:** "as a casual lich user, Saga has definitely replaced or
> mostly replaced some of these things."

> **INFERRED:** a well-curated behaviour set genuinely retires most users' scripting needs. The
> irreducible remainder is the power users — and Cena, which absorbs Lich rather than sitting
> beside it, must serve that remainder itself. See [§P](#p-pressure-on-existing-cena-decisions).

### C.6 Aliases are real but markedly weaker than macros — 11 distinct people

Several people explicitly reject them:

> **[8/16/2026 1:26 PM] paragonlost:** "Since I don't use alias, Bigshot etc and depend on my
> macros"
> **[8/14/2026 12:39 PM] paragonlost:** "I've never used an alias btw. Don't even grok them."

Alias demand is largely satisfied by Lich's `;alias`. **Build aliases, but do not prioritize
them over macros or sequences.**

### C.7 Automation silences

| Searched | Hits | Reading |
|---|---|---|
| Embedded scripting language requests | **0** | Nobody asks Saga to embed Ruby/Lua/Python. Across 133 messages about scripting from 46 people, the asks are for the client to absorb specific script *functions* or to host *panels*. **Direct support for Cena's no-embedded-scripting thesis.** |
| Conditionals/loops/state in user automation | **0** | Nobody says "I need my macro to branch." |
| Botting, AFK automation, automation policy | **0** | **INFERRED:** Cena need not litigate an automation-ethics position for this audience; they treat scripting as QoL, not as an exploit vector. |
| Cross-character or multi-session automation | **0** | Despite the 3–25 character constraint, nobody asks for a behaviour spanning characters. No prior art, no validated demand. |
| Trigger/macro debugger, dry-run, sandbox | **0** | Nobody asks to preview what an automation will do before it fires. Saga *does* ship live preview for **highlights** and it was praised — **INFERRED:** preview is valued for pattern-matching config generally, but nobody framed it as a safety need. |
| Scripts as a shareable first-class artifact | **0** | Sharing happens as raw settings files. No registry or package manager requested — Lich's repo already fills that role. |
| Accessibility ↔ automation connection | **0** | Nobody links accessibility to macros or triggers. §1.2 gets **no automation-side evidence** from this corpus. |

**Caveat on all §C counts.** From 8/5 onward, moderators redirected feature requests to an
in-app form (*"Use the File → Report Bug/Feature request in Saga for suggestions/bugs"*).
**Post-August feature demand is systematically undercounted here.** One player objected to the
opacity of that process — [8/19/2026] rysk, a product manager by trade: *"If you're unwilling
... to show open feature requests and allow +1ing to determine customer interest..."*

---

## D. Performance and multi-session

**32 distinct people discuss CPU/memory/resource cost.** This is where Cena's architecture wins,
and the win is narrower and better-evidenced than a generic anti-Electron argument.

### D.1 The finding is the scaling curve, not the absolute number

Single-instance Saga is fine for nearly everyone. Pain begins at **2–3** concurrent characters
and becomes disqualifying at **5–11**:

> **[8/13/2026] izzy101yzzi:** "Oof, Saga is pretty resource intensive. Absolutely destroys my
> CPU when I've got the full squad loaded. Hopefully some optimizations come that way,
> **otherwise it can't be a daily driver for my alts** 😭"

> **[8/15/2026 1:42 AM] dwgemster:** "When I'm duplicating how I run using Wrayth, the CPU usage
> is substantially higher, like **20-30% higher**. It's high enough that **it kicks on the fans
> in my system**, whereas running Wrayth never does that. My computer system's only a year old
> and it's pretty high performance."

> **[9/8/2026] izzy101yzzi:** "Yeah, **two saga instances is still too much**. Fine while idling,
> but as soon as I start up a hunt and the screen scroll starts pumping, it gobbles up my CPU to
> the point that **my mouse even stutters**."

> **[8/14/2026 12:00 PM] stav4025:** "So my testing ... is concluded. **11 instances in shattered
> is just too much for my laptop...8 was too**. But really like it"

The architecture is explicitly the cause, explained in-thread:

> **[8/12/2026] horibu:** "Yes. **7 instances is normal. Electron spins up multiple processes
> like that to make it work**" — replying to calista4817's Task Manager screenshot asking *"is
> it supposed to have 7 instances? (i have 3 windows open)"* *(😱 ×2)*

**3 windows = 7 OS processes.** Two characters with Lich ≈ 1.9 GB (horibu measured it).

### D.2 The head-to-head comparison has already been made publicly, in Cena's favour

> **[9/8/2026] izzy101yzzi:** "FWIW, **I can run 1 saga + 9 vellum GUI instances without issue**"

A player running **nine** instances of Cena's own predecessor alongside one Electron client,
and the one Electron client is the problem. This is the closest thing in the corpus to a direct
native-vs-Electron benchmark, and Cena's author did not have to make the claim — a user did.

**Cena's 3–25 session target sits entirely inside the region where Saga breaks down.**

### D.3 Honest counter-evidence, and two recency corrections

Weighting by distinct people cuts both ways, and this section would be dishonest without these.

- **Four distinct people report no performance problems at all** (ruseoffools, vorlash,
  thehouseofklaus, drekoi); drekoi runs 5 clients comfortably and stress-tested 10.
- **Memory was actively praised**, not criticized — [8/14/2026] neovik: *"I went from using 5
  instances of WarlockFE to 5 instances of SAGA. It looks like **SAGA used a lot less memory
  compared to WarlockFE**."*
- **Saga essentially never crashes.** Every substantive crash/memory-leak discussion in the
  thread is about the **incumbent** — [7/12/2026] stav4025 on Wrayth: *"Wrayth can blow up to
  over 2000..then freeze yer entire PC"*.
- **RECENCY: the problem was substantially mitigated in September.** [9/2/2026] dwgemster: *"I
  can now run 6 instances with the CPU levels at roughly what they are doing the same in
  Wrayth"*; [9/8/2026] ktig: *"**Switching Saga.exe to use GPU resolved my saga system lag.** I
  have HOA Reim-levels of screen scroll across 3 Saga instances with no issues."*

**So the honest claim is narrow and it is still decisive:** *Saga's CPU cost scales badly with
concurrent sessions; its memory and stability do not.* Do not claim "Electron is bloated" — this
thread does not support it. **Cena cannot win on stability or RAM. It can win on per-session CPU
scaling, and that is the axis the target market already cares about.**

### D.4 The GPU/software-rendering mess is an Electron-inherited class of problem

Users hand-tuned Chromium rendering paths with **contradictory** results: dwgemster found
*software* rendering faster on an i9-14900K + RTX 5080; ktig and althaz2.0 found the opposite.
The developers' response was to ask for Chromium performance traces. The best diagnosis came
from a player:

> **[9/2/2026] nisugi33:** "for a text-only UI, Chromium's GPU pipeline costs more CPU (IPC to
> the GPU process + driver overhead + bigger repaints) than it saves... **GPU acceleration is
> just the wrong tool for scrolling text.**"

> **R-D1 — Native text rendering.** The entire class — GPU compositing overhead for a text UI,
> per-machine driver pathology, users editing Windows graphics settings to play a MUD — does not
> exist in a native renderer. This is not an optimization; it is an absent problem.

### D.5 Multi-session as a UI concept is unmet demand that has not learned to speak

Saga's developer rejected multi-character tabs on day one:

> **[7/9/2026 10:08 PM] mirke:** "Separate instances of the front-end. Saga does not support
> multiple character tabs in one instance ... (**it would get very, very heavy, but it also just
> didn't make sense.**)"

**Three minutes later, a player specified Cena's architecture unprompted:**

> **[7/9/2026 10:11 PM] elmagen:** "it would be lighter weight than 3-to-5 instances of the
> actual FE. But, seems like it was considered. Idea still makes sense to me. **Some windows
> could be focused for just one character (i.e, the map). Don't need maps for everyone. Some
> characters would be more game/room window only.** ... **may not want all the ESP channels on
> all the characters**, too, especially on same channels." *(👍)*

That is shared/singleton panels across sessions, asymmetric per-character panel sets, **and
cross-session deduplication of chat channels** — Cena's multi-session thesis, asked for by a
player and declined by the official client.

The community also believes this work belongs to the third-party tool:

> **[7/12/2026 12:55 PM] amwranes:** "I'm not sure why Simu would choose to build that into
> their front end" *(👆 ×2)* — **stav4025:** "That's a 111% lich thing for sure."

**Cena's lane is uncontested.** Note the flip side, stated by Lich's own maintainer
([8/18/2026] lich_doug: *"the effort should be to continue adding functionality to make Lich
unnecessary"*): the official client is advancing into this territory. Cena's defensible ground
is **architecture** — multi-session, typed events, resource cost, unified pacing — not a feature
checklist.

> **R-D2 — Multi-session is a first-class UI concept, not N copies of the app.** One process, N
> sessions. Singleton panels shared across characters where it makes sense (map), per-character
> panels where it does not (story). Asymmetric layouts: the main character gets everything, alts
> get a room window. **Chat/ESP channels deduplicated across sessions.** Per-character window
> geometry that persists reliably — calista4817 asked for this on 8/15, 8/16, 9/2 and 9/14 and
> never got an answer. *(Flagged honestly: geometry persistence is one persistent voice plus two
> corroborators, not a crowd — but for a multi-session client it is table stakes, because a fixed
> rectangle is how a multiboxer knows which window is which character.)*

### D.6 Command pacing — a trap Cena sits in the middle of

**13 distinct people** discuss typeahead. Saga enforced the server's real typeahead limit;
Wrayth+Lich had been quietly circumventing it for a decade. **Users experienced correct protocol
behaviour as a regression.** The developer ultimately chose bug-compatibility with the incumbent:

> **[8/27/2026 5:25 PM] mirke:** "The next patch will probably align typeaheads closer with how
> they work in Wrayth."

**Recency and honesty note:** the most rigorous investigator retracted. [8/27/2026] dwgemster
published 30 timed runs showing Saga averaging 5.458s against Stormfront's 5.510s — marginally
*faster* — and concluded *"the issue was maybe how ;go2 was interacting with Saga."*
calista4817 hedged the same day: *"It's just a feeling, and I'm not sure it's real."*

> **R-D3 — One flow-controlled send queue.** Cena owns both the proxy and the frontend, so
> pacing is decided in one place instead of being guessed at by two components fighting each
> other. The behaviour layer, the user's typing and any macro/chain share one queue that
> understands roundtime and the server's real limit. **This is a structural advantage of the
> merged architecture that the plan has not yet claimed.**

---

## E. UI and layout

### E.1 A significant population plays GemStone primarily with a mouse

This challenges a keyboard/TUI-first mental model, and it surprised Saga's own developer:

> **[8/26/2026 2:53 PM] mirke:** "**Imagine my surprise at the folks who apparently only play GS
> using this panel!**" *(~10 reactions)*

Corroborated by regwen1 (*"I use the buttons here exclusively"*), calista4817 (*"is it for people
who play with a mouse only?"*), and two people for whom drag-and-drop inventory is
non-negotiable. **20 distinct people** discuss right-click/context menus.

> **R-E1 — Clickable affordances are a real input path, not a convenience.** The GUI and web
> frontends need click targets generated from typed events — clickable links, context menus,
> drag-and-drop inventory, hotbar buttons. **A TUI-first client cannot borrow Saga's interaction
> model wholesale, and the plan should not assume the keyboard is the primary interface for
> everyone.**

Fidelity to legacy clickable-link semantics from server-provided metadata (nested container GET,
put-into-container-in-other-hand) is the **one thing actively pulling users back to Wrayth** —
narrow (1 persistent voice, misaroe, plus 1 corroborator), but category-significant.

### E.2 Combat routing — 28 distinct people, the largest UI demand

> **[8/11/2026 12:41 PM] korpacz:** "20+ lines of scrolling bloat to tell you the creature is
> dead and you looted 20 silver."

> **[7/29/2026 1:59 PM] misaroe:** "I genuinely can't handle it anymore, especially during boss
> fights or onslaughts. **My brain just turns into mush.** But if I use things like
> `;briefcombat`, I also don't get to see how I die so..."

The asks are more sophisticated than "a combat window":

- **Different verbosity of the same event in two panels simultaneously** — jenuinejenanigans
  [7/15/2026]: *"echo combat panel to story panel but squelch extraneous things only in story
  window and not in combat panel."*
- **Aggregation, not routing** — calista4817 [8/3/2026] wants *"a single small window that just
  lists total damage, # of crits, # of flares, end result."*
- **Incoming vs outgoing, and others' combat separated** — rysk [7/15/2026].

And the server-side flags are **not** sufficient — [7/15/2026] jaladattanagra.: *"combatbrief
itself could use a lift. it only covers the initial attack, which now-a-days is the least spammy
part of our attacks heh. **it's the 1000 flares that come after.**"*

> **R-E2 — Per-panel verbosity over typed events.** Not routing and not squelching: the same
> event renders at different density in different panels, with aggregation available as a
> render mode. **If the plan assumed `COMBATBRIEF`/`COMBATNONUMBERS` could handle verbosity
> reduction, that assumption fails** — Cena must aggregate over its own typed events. This is the
> same requirement as A.2/A.4 arriving from non-disabled users, which means **the accessibility
> feature and the mainstream feature are one feature.**

### E.3 Status/roundtime indicators must be movable and have text alternatives

> **[8/12/2026 7:35 PM] misaroe:** "They're so important for hunting that I'd love to see it in
> its own window. **And also a text version**" … the icons are "too complex for the size... I
> wish there were tooltips or a text version."

> **[8/12/2026 7:14 PM] 0mrii:** on the default roundtime wheel — "The wheel was cute, then I
> went hunting and **wanted it to die**."

> **R-E3 — Every glanceable indicator is movable, resizable, and has a text rendering.** A text
> alternative is required for the TUI anyway; it is also the accessibility answer and the
> small-screen answer.

### E.4 Quick loadout switching by activity, not by character

> **[7/13/2026 10:07 AM] maylan_the_maybug:** "quick loadout switching. **I'll use a different
> set of panels for combat vs RP situations.**" — 14 reactions, the **highest-voted feature
> request in the thread**.

> **[8/14/2026 3:47 AM] chueeb_23674:** "a layout you use when solo and another you use when
> grouped?"

Saga requires a menu dive. **R-E4 — Named layout presets, switchable with one keystroke, scoped
per activity rather than per character.**

### E.5 Two things Saga shipped that are worth copying outright

- **ALT+click split-scroll** — [8/12/2026 9:08 PM] theponzzz: *"you can split screen and save
  what you want to stay on screen! Super helpful when you want to scroll back but still see
  what's going on."* Solves the real problem that scrolling back means going blind to live
  combat.
- **Right-click "Track All" on active effects** — [8/12/2026] izzy101yzzi. Turns current game
  state into persistent configuration in one action. **A generalizable pattern: any observed
  state should be convertible to config with one gesture.**

---

## F. Multi-session

Covered in [§D.5](#d5-multi-session-as-a-ui-concept-is-unmet-demand-that-has-not-learned-to-speak).
The observed instance-count distribution, stated by players in their own words:

| Person | Instances | Source |
|---|---|---|
| stav4025 | **11** (8 too many) | [8/14/2026 12:00 PM] |
| drekoi | 5, stress-tested 10 | [8/13/2026 1:25 AM] |
| izzy101yzzi | 10 total | [9/8/2026] |
| paragonlost | 10 | [8/18/2026 4:08 PM] |
| dwgemster | 6 | [9/2/2026 10:47 AM] |
| neovik | 5 | [8/14/2026 12:39 PM] |
| nisugi33 | 5 | [8/25/2026 10:31 AM] |
| mirke (dev) | 7–8 | [7/12/2026 2:22 PM] |

**Cena's 3–25 range is validated at the low-to-mid end and is conservative at the top.** The
observed cluster is **3–6** with a real tail at **10–11**. Nobody claims 25 — but nobody claims
it is absurd either. elmagen frames the population [7/9/2026 10:03 PM]: *"I think there are a lot
players who MA to various degrees: some 2 or 3, others with platoons."*

---

## G. Mobile

**This is the section that most challenges the plan, so it is stated plainly.**

**9 distinct people out of 143 (6%)** mention mobile, phone or tablet across ten weeks — and
several of those are incidental (*"I was mobile and didn't see it"*). Compare 76 on panels, 54 on
highlights, 40 on macros.

The substantive mentions, in full, are these:

> **[7/12/2026 12:13 AM] bait4376:** "I'm sure this was asked but will there be IOS or Android
> support for Saga?" — **no reply.**

> **[8/11/2026 7:39 PM] calista4817:** "does saga work on ios/ipados?" → **horibu:** "not
> currently, I think the goal there is for the eventual Saga web client to be what you'd use for
> that."

> **[8/4/2026 10:53 PM] calista4817:** "a website thing would be amazing. or a mobile app."

> **[7/12/2026 12:27 PM] soleblaze:** "It was a good (well, only) option to **quickly login with
> a phone**."

That is the whole of it. **Nobody describes playing a session on a phone or tablet.** `tablet`
and `ipad` together yield one question about OS support. When told mobile was deferred
indefinitely, **there was no pushback, no follow-up, and no reaction votes** — compare typeahead,
which generated a user-run benchmark table and cost Saga at least one named non-adopter.

**And the single most uncomfortable data point in this entire document:**

> **[8/9/2026 2:34 PM] rysk:** "I can only imagine. **I use VellumFE on my phone and even with
> tiny font and briefcombat at its most extreme its still easy to miss things**"

The one person in the corpus actually playing on a phone is doing it on **Cena's own
predecessor**, and reporting that it fails. Notably, **the reported failure is not layout — it is
content density.** Even at minimum font with maximum server-side brevity, the combat stream is
too much to follow on a small screen.

> **R-G1 — Re-derive the mobile constraint as a density problem, not a layout problem.** The
> requirement this evidence supports is: *the client must degrade to a narrow column with
> **aggregated, summarized** combat*, which is [R-E2](#e2-combat-routing--28-distinct-people-the-largest-ui-demand)
> again. It does **not** support letting phone width shape desktop panel architecture.

**Better-evidenced than mobile play, and cheaper:** a **cross-character offline state view** —

> **[8/4/2026 3:53 PM] calista4817:** "something that snapshots the important info of all your
> characters when you log out. So you can check their info without logging them in." — whirlin
> cited Blizzard's Armory and FFXIAH as precedent.

Cena already persists session state, so this is nearly free, and it is the *actual* thing a phone
is good for in this game.

---

## H. Migration from older clients

### H.1 The migration sources are not the ones the plan assumes

| Client | Distinct people mentioning |
|---|---|
| **Wrayth** | **50** |
| **Warlock** (3rd-party) | **10** |
| StormFront | **3** |
| Genie | **1** |

**Genie is a DragonRealms client and is essentially absent from this GemStone thread.
StormFront is nearly absent.** The real migration sources are **Wrayth**, **WizardFE** and
**Warlock** — and Warlock, a *hobbyist third-party client*, is the named reference
implementation for the features players most want (global highlights, twice).

**For competitive-intelligence purposes, Cena's customization surface is being measured against
Warlock, not against Simutronics' own Wrayth.**

### H.2 Import is an adoption gate, stated as such

> **[7/13/2026 10:23 AM] lucu11an:** "Can we make highlight/settings exportable from Wrayth
> importable in Saga? ... **having to rebuild all my highlights are why I dont use warlock, or
> any other front end**, so I dont have to redo **25 years of rainbow coloring and sound cues**."

**23 distinct people** touch import/migration. Highlight import largely worked and materially
drove adoption. **Macro import did not**, and partial success was worse than failure:

> **[8/16/2026 3:24 PM] paragonlost:** "Ok, so I exported 'just the macros' and saved them. Went
> to import in Saga and got the same messaging. Which means my macros aren't usable. ... he's
> probably at around 100 or less macros. **53 not migrating is crippling.**"

Related and equally disqualifying — **silently reinstating defaults over a user's bindings**
(C.3b): a macro used *"since 1992"* kept being overwritten, and the user said it *"really drains
my desire to deal with the other nine characters."*

> **R-H1 — Import Wrayth/WizardFE/Warlock highlights, macros and colour schemes on day one, and
> report exactly what did not import and why.** A silent partial import is worse than a refusal.
> **R-H2 — Never silently reinstate a setting the user removed.**

### H.3 Monospace lock-in is an abandonment condition

> **[8/9/2026] nisugi33:** "It's neat, lots of configurability. But **it's locked to mono-spaced
> fonts if you want to be able to read the text and that killed it for me.**"

Corroborated by soleblaze (*"I don't think I've ever played gemstone a non monospace font... I'd
like to break that habit for more readable space"*), and by Simutronics' own producer
([8/9/2026] theponzzz): when Saga was shown to DragonRealms, the absence of fixed system font
support *"was described as a complete dealbreaker on the FE."*

**R-H3 — Proportional font support is an adoption gate, not a nicety.** Note this interacts with
alignment-sensitive output (maps, tables, ASCII art), which is a real design problem rather than
a flag to flip.

---

# I. What Saga got right — the bar Cena must clear

**Saga is a genuine success, not a weak incumbent to exploit.** The plan should not assume
otherwise:

> **[8/19/2026] ruseoffools:** "I'm stumped to think of any functionality that wrayth had that
> saga doesn't and **saga has so many more features already I'm perfectly content.**"

Even the harshest performance critics say the client is worth using.

### I.1 The panel surface — layout as a user-owned artifact

**76 distinct people / 359 messages.** This is the single most-loved thing about Saga, and it is
the answer to "what would make a new client feel like a downgrade."

- Drag a panel out of the tab header to detach it; drag one text panel onto another to *make* a
  tab. **Tabs and windows are the same object at different positions.**
- "Move to New Window" onto a second monitor.
- Per-panel styling by right-click: background, trim, font.
- A snap grid on resize.
- The whole arrangement exports as one artifact.

> **[8/14/2026 12:26 PM] paragonlost:** "The options are just... wow. […] it truly is an
> outstanding bit of work and I can tell right now its **magnitudes better than any of the
> previous FEs** going back to the GEnie FE or the various MUD FE's I've known over the decades."

> **[8/18/2026 10:13 PM] drigler:** "This new front end gave me that **old feeling of buying that
> new video game** and getting it set up to play... quite the gamechanger! feels like a brand new
> game!" *(13 reactions — the highest-reacted pure praise in the thread)*

### I.2 Cross-platform native — the most emotional praise in the corpus

**19 distinct people on Mac, 14 on Linux.**

> **[7/10/2026 4:06 AM] gs.itzel:** "I put this elsewhere, but y'all. Y'ALL. **The only reason I
> still had Windows on a partition on my Mac was for GS. It is now gone, gone, gone.**"
> *(❤️ ×8 🎉 ×2)*

soleblaze pulled the GitHub release download counts on 8/13: **Windows 269 / macOS 45 / Linux
AppImage 18.** **INFERRED: ~80% Windows, ~13% Mac, ~5% Linux** among early adopters. Linux is
small but vocal and technically capable — and it is the population best served by a Rust binary
with a TUI, including over SSH, which Electron cannot do at all.

Note *what* they celebrate: not "it runs on Mac", but **"I deleted my Windows partition."** The
win is removing a workaround from the player's life.

### I.3 Session lifecycle QoL — the best goodwill-per-engineering-hour in the thread

**Reconnect: 10 distinct people praise it, zero criticize it, and praise recurs July through
September** — structural, not launch-week glow.

> **[9/1/2026 11:52 AM] al_aladain:** "I don't know whose brainchild it was to have the
> reconnect/go back to login option when your session ends but I'm oddly very thankful for it!
> **I really didn't think I was going to switch away from WRAYTH.**"

**Verses** (log in a defined group of characters as a set) — 9 distinct people, and note *who*
praises it:

> **[8/13/2026 4:18 PM] lich_doug** (Lich's maintainer): "Possibly my favorite new feature is one
> oft requested of Lich - how can I log in a team, or set up a grouping of characters to manage.
> I always wanted to deliver that. But the Saga team came along and said **'move along old man,
> let us show you how this is done.'** And BAM! `Verses` is born. Love this thing! So jelly."

**A named, saveable character-group launcher is table stakes**, and Lich's own maintainer says
it is the feature he always wanted and never shipped. Similarly cheap and disproportionately
loved: *alt-tab doesn't drop you out of rest mode*, fixed in 30 minutes and still cited weeks
later as a favourite.

> **Session-lifecycle QoL is radically under-weighted in the Cena plan relative to its observed
> emotional return.**

### I.4 Structured game-data panels replacing script output

Almanac, Codex, Ascension trainer, Objectives/bounties, combat tracker, inventory by item
category, active-spell timers with right-click "Track All". Saga pulled information out of the
text stream into always-on structured UI — work that previously required Lich scripts or nothing.

> **[8/18/2026 5:30 PM] maylan_the_maybug:** "Very nice updates with the latest release. And,
> ascension **finally** has a real GUI trainer 🎉"

**This is the Cena curated-behaviour thesis, already validated as a product direction.**

### I.5 Third-party panels as first-class citizens

Community Lich scripts render as native Saga panels (`*_sagapanel.lic`), and the developers moved
to formalize it:

> **[8/25/2026 10:32 AM] gs4nyxus:** "To the folks who are writing lich panels for Saga... at
> some point we should discuss requirements for **a hypothetical Saga extensions API**"
> *(💚 ×3 🎉 ×4)*

**The extension point players actually use is "contribute a panel", not "write a script."** That
validates Cena's data-profile approach — **with a catch the plan must account for: Saga got
community panels *because Lich still existed alongside it* as the scripting host. Cena absorbs
Lich, so it must supply that extension surface itself or lose the ecosystem effect entirely.**

### I.6 In-client structured bug/feature reporting

Credited by multiple people as the reason Saga's iteration felt fast. Also a caution: one person
criticized it as a one-way black hole (soleblaze — flagged as a single voice), and rysk argued
for a public, +1-able list.

---

# J. What Saga got wrong — Cena's opportunity

Ordered by how directly Cena's architecture avoids the problem.

| # | Defect | Distinct people | Electron-specific? | What Cena does instead |
|---|---|---|---|---|
| 1 | **Per-session CPU cost** — unusable at 2–3 concurrent characters for many | ~6 solid, 32 touching the topic | **Yes** — multi-process Chromium per instance | One process, N sessions. [R-D2](#d5-multi-session-as-a-ui-concept-is-unmet-demand-that-has-not-learned-to-speak) |
| 2 | **Chromium steals keybinds** — Ctrl +/- and Ctrl+Numpad resize fonts instead of firing macros; user cannot override | 6 genuine complainants | **Yes** — some chords are reserved *outside the application* | Native client owns its full keyboard event stream. [R-C2](#c3-macros-and-hotkeys--the-real-demand-and-it-is-pure-configuration) |
| 3 | **GPU/software rendering pathology** — contradictory fixes, no official guidance | several | **Yes** | Native text renderer. [R-D1](#d4-the-gpusoftware-rendering-mess-is-an-electron-inherited-class-of-problem) |
| 4 | **Windows Alt-codes leak into the command line** — `©+‡7ú♣♣♣♣` appearing mid-hunt | 3+ | Partly | Own the input path |
| 5 | **No global settings layer** — configure everything N times | 11 | No | [R-B1](#b1-global-vs-per-character-settings--the-1-structural-request-unresolved-for-ten-weeks) |
| 6 | **Font override missing on some panels; regressed mid-beta** | ~9 | No | [R-A1](#a2-low-vision-is-the-real-demand-and-it-is-about-scaling--9-distinct-people) |
| 7 | **Themes override user contrast settings** | 3 + ~6 on opacity | No *(and being fixed by 9/8)* | [R-B5](#b5-themes--the-users-contrast-settings-must-win) |
| 8 | **Sound alerts bind only to text matches** | 8 | No | [R-A6 / R-C1](#c2-triggers-the-dog-that-did-not-bark) |
| 9 | **Macro import dropped 53 of ~100 macros silently** | 1 acute + ~9 asking | No | [R-H1](#h2-import-is-an-adoption-gate-stated-as-such) |
| 10 | **Settings overwhelm / discoverability** | 10, incl. 2 staff | No | Curated presets — see [§P](#p-pressure-on-existing-cena-decisions) |

**ktig's line is the sharpest statement of the cost of defect #2:**

> **[8/14/2026] ktig:** "**I can't use Saga for hunting until that gets fixed**" — and it was
> still broken two days later, and the developer was still saying *"back to the drawing board"*
> on 8/27.

**Items 1–4 are inherited from the runtime and simply do not exist in a native Rust client.
Items 5–10 are design choices Cena can make correctly the first time because the evidence is
now in hand.**

---

# P. Pressure on existing Cena decisions

This is the most valuable section in the document, so it is written to be uncomfortable where
the evidence warrants it.

### P.1 No embedded scripting language ([`08-curated-behaviors.md`](08-curated-behaviors.md)) — **SUPPORTED, but the plan's stated reasoning is incomplete**

**What the evidence supports, strongly:**

- **Zero players ask for an embedded scripting language** in 4,979 messages. Across 133 messages
  about scripting from 46 people, the asks are for the client to absorb specific script
  *functions* (go2, briefcombat, mana timer, creature window) or to host *panels*.
- **Zero people ask for conditionals, loops or state** in user-authored automation.
- **~2 people in ten weeks ask for a reactive trigger.** The plan's biggest perceived risk —
  "users will demand triggers" — is **not supported by this evidence at all.**
- The most celebrated automation in the thread (ubp's fully mouse-driven combat, ❤️×4) required
  **no scripting**: macros, a hotbar and mouse buttons.
- Saga's own attempt at a legacy-compatible script dialect produced a compatibility surface its
  developer could not specify and asked the community to write down for him. **Nobody did.**

**What genuinely threatens the decision — three things, and the plan should answer all three:**

**(a) The competitor shipped one anyway.** Saga concluded configuration was insufficient and
built `SETVARIABLE`, `matchre`, `waitfor`, `counter`, `goto` and `if_N` — *independently of user
demand*. That is a live market signal that a serious team, looking at the same players, reached
the opposite conclusion. The plan should state why Cena's answer differs. **The available answer
is good:** Saga's scripting exists to be Wizard-compatible for legacy scripts; Cena, being
private and greenfield, has no legacy corpus to satisfy and can **declare** its semantics rather
than chase consensus.

**(b) The fallback — "configuration will cover it" — was explicitly argued against, in this
thread, by an experienced player.** This is the single most direct challenge in the corpus to
Cena's stated strategy, and it deserves to be quoted in full:

> **[8/26/2026 4:12 PM] soleblaze:** "One thing I see a lot with agentic coding is **config
> creep**. It loves to make as much as it can configurable and doesn't understand how confusing
> it can make interfaces. I'd agree that **it'd be better to have some kind of extension system
> vs trying to cater to all the various niches. It'd windup a big ball of mud otherwise.**"

Saga's lead developer agreed, about his own product:

> **[8/26/2026 4:18 PM] mirke:** "One piece of feedback I get from colleagues and coders outside
> of the GS spectrum is that **it's a fool's errand to try to code to everyone's needs. I am
> definitely spread a little thin trying to hit all of the customizations everyone wants.**"
> *(VibeCat ×3)*

> **[8/27/2026] mirke:** "We have so many customizations that **it is hard to intuitively find
> things.**"

**Ten distinct people independently reported feeling overwhelmed, including two staff members.**
A plan that answers "no scripting" with "more configuration" walks directly into the failure
mode this thread documents. **The plan needs a stated position on configuration ceiling and
information architecture, not just on option count.** Cena's data-profile model is well placed
for this *if* profiles are treated as **curated, shareable presets** rather than as exhaustive
knob inventories.

**(c) Cena absorbs Lich, so it inherits the expert tail with no escape hatch.** Today the
bimodal distribution is served by two products: Saga for the mass market, Lich for the ~50% who
touch scripting. `rysk`'s `GameObj.pcs.each { |pc| fput "group #{pc.noun}" }` has somewhere to
live. **In Cena's world it does not.** The thread shows Saga surviving its config ceiling
*precisely because Lich exists beside it* — including for the community panel ecosystem
([I.5](#i5-third-party-panels-as-first-class-citizens)).

> **RECOMMENDATION.** Keep the no-scripting decision — the player-demand evidence for it is
> stronger than the plan claims. But **the plan should name the escape hatch explicitly**, and
> the thread says what shape it takes: **an extension/panel API, not a scripting language.** That
> is what Saga's own staff proposed (gs4nyxus, 💚×3 🎉×4), what soleblaze argued for, and what the
> `*_sagapanel.lic` ecosystem demonstrates in practice. An extension surface is compatible with
> "no embedded scripting language" — it is a different, out-of-process boundary — and it is the
> thing that keeps the expert tail from being stranded.

One more piece of support the plan can use, which cuts against the usual usability objection to
config:

> **[8/14/2026 3:30 PM] bobandy2996:** "having **codex read your logs and write 30+ regex rules
> for flare highlighting directly to your settings.json file** is pretty cool" *(❤️)*

**Users are already generating their configuration with LLMs.** If the config file is the
interface and a model writes it, config verbosity stops being a usability cost. That materially
strengthens Cena's position — and it argues for keeping profiles in a **machine-writable text
format**, which is worth weighing against the B.7 silence (essentially nobody asked to hand-edit
config). The resolution: **text format for tools and agents, GUI for humans, and the GUI is not
optional.**

### P.2 Mobile as a driving constraint — **NOT VALIDATED by this evidence**

Stated plainly, because the task asked for plainness: **this corpus does not support mobile as a
driving constraint.** 9 of 143 people mention it, the only named use case is logging in, nobody
describes playing on a phone or tablet, and when the official client deferred mobile
indefinitely nobody objected. The one person actually playing on a phone is doing it on
VellumFE and reporting failure.

**Three honest readings, and the plan should pick one deliberately:**

1. **It is a genuine bet on an unserved market.** Defensible — Saga has explicitly deferred
   mobile to a hypothetical web client, so the lane is empty. But call it a **bet**, not a
   requirement, and do not cite player demand for it.
2. **It is an architecture forcing-function.** Also defensible, and arguably the stronger case:
   designing for a narrow column forces the density work in [R-E2](#e2-combat-routing--28-distinct-people-the-largest-ui-demand),
   which **28 distinct people demanded on the desktop**. The mobile constraint earns its keep by
   what it forces, not by who asked for it.
3. **The constraint is mis-specified.** The observed failure is **content density**, not layout.
   Re-derive it as *"must degrade to a narrow column with aggregated combat"* rather than letting
   phone width shape desktop panel architecture.

> **RECOMMENDATION: keep mobile, re-label it.** Demote it from *driving constraint backed by
> demand* to *deliberate bet plus architecture forcing-function*, and **re-specify it as a
> density requirement**. The same move preserves the engineering benefit while stopping the plan
> from citing evidence it does not have.

### P.3 The TUI — **no demand, and that is fine if the plan says why**

Nobody in 4,979 messages complains about lacking a TUI, and Vellum's TUI is mentioned exactly
once, descriptively. Only 1 person touches keyboard-only navigation.

**But** [§E.1](#e1-a-significant-population-plays-gemstone-primarily-with-a-mouse) shows a
significant population plays with a mouse, and Saga's most-loved feature (the panel surface) is
the hardest thing to express in a terminal.

> **INFERRED: the TUI's value to Cena is as an architecture forcing-function, a Linux/SSH asset,
> and a latent accessibility asset — not as a headline user feature.** The plan should say so,
> and should not assume the TUI's interaction model transfers to the GUI and web frontends. It
> does not: those need clickable affordances as a real input path.

### P.4 Typed events and the parser — **strongly validated, and under-claimed**

Every major requirement in this document converges on the same architectural feature:

- Semantic highlighting by event class ([R-B3](#b3-highlights-are-the-combat-hud--and-this-is-the-deepest-finding-in-the-thread))
- Alerts on state predicates rather than text ([R-C1](#c2-triggers-the-dog-that-did-not-bark))
- Per-panel verbosity and aggregation ([R-E2](#e2-combat-routing--28-distinct-people-the-largest-ui-demand))
- Text alternatives for glanceable indicators ([R-E3](#e3-statusroundtime-indicators-must-be-movable-and-have-text-alternatives))
- Clickable affordances generated from metadata ([R-E1](#e1-a-significant-population-plays-gemstone-primarily-with-a-mouse))
- Mobile density degradation ([R-G1](#g-mobile))

**The plan treats typed events as an implementation detail. This evidence makes them the
product.** Five of the ten highest-demand requirements in this document are *unbuildable*
without them and *straightforward* with them.

### P.5 Merged proxy + frontend — **validated, and it solves problems Saga could not**

Three concrete wins the plan has not claimed:

1. **One send queue.** [R-D3](#d6-command-pacing--a-trap-cena-sits-in-the-middle-of) — Saga and
   Lich fought over typeahead for two weeks because neither owned the whole path.
2. **One log.** [8/13/2026] neovik: *"so on all my characters I'm running `;log` script. I'm
   assuming now with logging in SAGA **I'm doing double work?**"* Cena is the only client that can
   answer this cleanly.
3. **No second-class text.** Saga hardcoded `.line-lich { opacity: .6 }`, visually demoting
   third-party output. In Cena there is no "third-party output" — it is all one client's text.

### P.6 A positioning question the plan should answer

Cena's author is already an established, respected voice in this community: `nisugi33` is among
the most active posters, gives Saga's team engineering advice drawn from Vellum's keybind work,
proposes protocol extensions to staff, and published the definitive reverse-engineering of
Saga's scripting verb set. VellumFE is described accurately and favourably by others in the
thread, and horibu links it publicly.

**Cena is not launching into a cold market.** But note the asymmetry: a **private** client versus
this much public presence is a positioning question the plan does not currently address. Related,
from day one of the thread:

> **[7/9/2026] tdriggs83:** "He's fast because there's no QA or process behind him — which is
> also why Warlock breaks something almost every release. **You give him a pass on that because
> he's a hobbyist. Simu won't get the same pass**, and you know it."

**Cena sits on the forgiving side of that line, and that tolerance is a real, spendable asset —
which disappears the moment Cena is perceived as official.**

---

# Q. Priority ranking for Cena

Ranked by **(evidence strength × Cena's structural advantage) ÷ cost**, not by how loudly a
thing was demanded. Reasoning given for each, because the ranking is an argument, not a list.

### Tier 1 — Do these first; they are load-bearing for everything else

**1. Typed events as the product surface, not an internal detail.**
*Why first:* five of the ten highest-demand requirements are unbuildable without it and
straightforward with it ([P.4](#p4-typed-events-and-the-parser--strongly-validated-and-under-claimed)).
It is also the only item here that gets *harder* to retrofit over time. Every week it is deferred
costs more.

**2. The settings hierarchy: global → profile → character.**
*Why first:* 11 distinct people, the second message in the thread, still unresolved ten weeks
later in the competing client — and it is a **data-model decision**, so it is nearly free now and
a migration later. For a 3–25 session client it is the difference between configuring once and
configuring twenty-five times. [R-B1](#b1-global-vs-per-character-settings--the-1-structural-request-unresolved-for-ten-weeks)

**3. One command-action model, bound from many surfaces, with no reserved chords.**
*Why first:* 40 distinct people on macros — the largest concrete configuration demand — and the
seam between keybind/hotbar/mouse/chain is an architectural decision, not a feature. Saga's
failure here made one player say he could not hunt. [R-C2](#c3-macros-and-hotkeys--the-real-demand-and-it-is-pure-configuration)

**4. Universal, independent text scaling — every surface, including settings.**
*Why first:* it is the top accessibility requirement, it serves the **median** user of an aging
player base rather than a minority, and scaling must be a layout input from the start. Retrofit
produces exactly Saga's "some panels have overrides, some don't" outcome. [R-A1](#a2-low-vision-is-the-real-demand-and-it-is-about-scaling--9-distinct-people)

### Tier 2 — High demand, high differentiation, buildable on Tier 1

**5. Highlight engine at veteran scale, with semantic highlighting on typed events.**
54 distinct people; 1,500-entry lists are real. The semantic layer is the deepest insight in the
corpus and is simultaneously a speed feature and an accessibility feature. [R-B2](#b2-highlights-at-veteran-scale--the-failures-are-list-management-not-matching) / [R-B3](#b3-highlights-are-the-combat-hud--and-this-is-the-deepest-finding-in-the-thread)

**6. Per-panel verbosity and aggregation over combat.**
28 distinct people — the largest UI demand — and it doubles as the accessibility requirement
(A.3/A.4) *and* the mobile density requirement (R-G1). **Three constituencies, one feature.**
[R-E2](#e2-combat-routing--28-distinct-people-the-largest-ui-demand)

**7. Alerts bound to parsed state, not text.**
Low raw demand (~2 people asked) but it is where Saga's ceiling is provably located, it closes a
gap no configuration-only client can close, and it is cheap once typed events exist. **Ranked on
structural advantage rather than vote count, deliberately.** [R-C1](#c2-triggers-the-dog-that-did-not-bark)

**8. Import from Wrayth / WizardFE / Warlock — with an honest failure report.**
An adoption gate, stated as such by multiple people. Cheap relative to its effect on whether
anyone uses the client at all. [R-H1](#h2-import-is-an-adoption-gate-stated-as-such)

**9. Multi-session as a UI concept: one process, N sessions, shared and asymmetric panels,
deduplicated chat channels.**
Cena's clearest structural win, specified by a player on day one and declined by the official
client. Ranked below the above because the *architecture* is already in the plan — what is
missing is the **UI concept**, which is the part players cannot ask for because they have never
seen it. [R-D2](#d5-multi-session-as-a-ui-concept-is-unmet-demand-that-has-not-learned-to-speak)

### Tier 3 — Real demand, cheap, disproportionate goodwill

**10. Session-lifecycle QoL: reconnect, return-to-login, named character-group launcher.**
The best goodwill-per-engineering-hour ratio in the entire thread — 10 distinct people, zero
detractors, praise sustained over three months, and one named switcher. Cheap. **Under-weighted
in the current plan.** [I.3](#i3-session-lifecycle-qol--the-best-goodwill-per-engineering-hour-in-the-thread)

**11. Panels as the universal primitive** — every stream a panel, any panel a tab or an OS
window, per-panel styling, the whole layout one exportable artifact. This is the bar. It is high
effort, which is the only reason it is not Tier 2; the TUI expression of it needs solving
deliberately. [R-B6](#b6-panels-and-layout--sagas-best-work-and-the-bar)

**12. Motion control, and user contrast settings that survive theming.**
~7 distinct people, trivially cheap, and Cena has no immersive-theming feature yet to conflict
with — so getting the **precedence rule** right now costs nothing. [R-A5](#a4-motion-and-animation--7-distinct-player-requests-never-called-accessibility) / [R-B5](#b5-themes--the-users-contrast-settings-must-win)

**13. Named layout presets switchable by one keystroke.**
Highest-voted single feature request in the thread (14 reactions), and it fits the data-profile
model exactly. [R-E4](#e4-quick-loadout-switching-by-activity-not-by-character)

**14. Curated shareable presets as the answer to configuration overwhelm.**
Not a feature so much as a policy: ship opinionated defaults, make them shareable, treat the
exhaustive knob inventory as the advanced path. This is the answer to the failure mode in
[P.1(b)](#p1-no-embedded-scripting-language-08-curated-behaviorsmd--supported-but-the-plans-stated-reasoning-is-incomplete).

### Tier 4 — Do, but do not lead with

**15. Clickable affordances and context menus** generated from typed events (20 distinct people;
a real input path for a real population, but it follows from typed events).
**16. Proportional font support** (adoption gate, but interacts with alignment-sensitive output).
**17. Cross-character offline state view** (better-evidenced than mobile play, and nearly free).
**18. Sound alerts with per-highlight and per-event binding** (8 distinct people; follows from #7).
**19. An extension/panel API** — the escape hatch named in [P.1](#p1-no-embedded-scripting-language-08-curated-behaviorsmd--supported-but-the-plans-stated-reasoning-is-incomplete).
Low urgency, high importance: **design the boundary early even if it ships late**, because Cena
absorbs Lich and therefore inherits the ecosystem obligation Saga could outsource.

### Explicitly de-prioritized, with reasons

| Item | Why down-ranked |
|---|---|
| **TTS / screen-reader mode** | Keep the existing Vellum module; do not build more. Least-demanded accessibility feature in the corpus (2 people, day one, never again). **But see the selection-effect caveat in [A.1](#a1-screen-readers-and-tts--asked-twice-then-ten-weeks-of-silence) — this is de-prioritized, not dismissed.** |
| **An IDE-style docking framework** | `dock` appears **zero times** in 4,979 messages. Solving a problem in vocabulary players do not have. |
| **Multi-monitor as a feature** | 3 people, satisfied immediately by "move to new window". Falls out of #11 for free. |
| **Colorblind-safe palettes as a curated feature** | Zero mentions. **INFERRED:** full user colour control *is* the accessible design here. Revisit only with primary research. |
| **A mid-tier scripting DSL** | Serves the empty middle of a bimodal distribution — almost nobody. [C.1](#c1-the-demand-curve-is-bimodal-with-an-empty-middle) |
| **Legacy Wizard-script compatibility** | Saga's developer could not specify the compatibility target and the community would not write it down. Unwinnable. [C.4](#c4-sagas-built-in-script-engine--and-why-cena-should-not-copy-it) |
| **Mobile-first layout work** | Re-specified as a density requirement under #6. See [P.2](#p2-mobile-as-a-driving-constraint--not-validated-by-this-evidence). |

---

# R. What this evidence cannot tell us

The limits, stated at the same level of detail as the findings, because a product document that
overstates its evidence is worse than none.

### R.1 Structural limits of the sample

**One thread, one competing client, one game, ten weeks.** It is strong evidence about what
*this* population wants and weak evidence about anything else. Specifically:

- **It measures reaction to Saga, not desire in the abstract.** People name what is broken or
  missing in front of them. Features nobody has seen — a unified multi-session interface, a TUI,
  semantic highlighting — generate near-zero demand *because they are unimaginable from where the
  players sit*, not because they are unwanted. elmagen's day-one multi-session specification is
  the exception that proves it: he could describe it only because he had just been told it did
  not exist.
- **Absence of a complaint is not absence of a need.** This cuts hardest on accessibility: a blind
  player cannot complain in a Discord thread about a client they cannot use. **The 143 posters
  approximate "people Saga works for."**
- **Selection toward the technical.** These are people who join a beta Discord, decompile
  JavaScript, run Lich, and post benchmark tables. Roleplay — GemStone's core social loop —
  appears almost entirely as *a mode to switch layouts for*, never as something people describe
  doing. **Do not conclude social play is unimportant; conclude this corpus cannot measure it.**
- **Recency bias toward breakage.** August is 67% of the messages because the beta shipped then.
  What broke in August is over-represented relative to what matters in general.
- **Feature requests were systematically diverted.** From 8/5 onward moderators redirected
  requests to an in-app form. **Post-August feature demand is undercounted across every section
  of this document.** All counts here are floors.
- **Late-thread praise is socially policed.** [8/18/2026 4:30 PM] mirke: *"I'd also ask that
  people keep things positive here. Saga is a passion project. Don't be a wet blanket on the
  fire."* (14 reactions). **Noted and ignored as instruction; recorded as a reason to discount
  September praise slightly.**

### R.2 Questions this corpus answers badly or not at all

| Question | Status |
|---|---|
| Do blind players need Cena, and what would they need? | **Unanswerable here.** Zero screen-reader mentions, but see the selection effect. Needs primary research. |
| Colour-vision deficiency, dyslexia, motor accessibility, Deaf/HoH needs | **Zero evidence in either direction.** |
| Would anyone play on a phone or tablet? | **Near-zero evidence.** One person tries, on Vellum, and reports failure. |
| Does anyone want a TUI? | **Zero demand expressed; also zero exposure.** Unmeasurable from here. |
| Is 25 concurrent sessions real? | **Unvalidated.** Observed ceiling is 11, cluster is 3–6. Nobody says 25 is absurd; nobody does it. |
| What do DragonRealms players want? | **8 distinct mentions** in a GemStone thread. Essentially unmeasured, despite official DR support. |
| Security, privacy, telemetry, credential handling | **Raised by nobody**, despite the client proxying account credentials and shipping auto-updates. **This is a silence Cena should NOT read as permission** — it reflects what players think about, not what is safe. |
| Localization / internationalization / IME | **Zero mentions.** |
| Does the "no scripting" decision survive contact with real users over years? | **Unknowable from a ten-week beta thread.** The evidence here is about *stated demand*, which is not the same as what people reach for at month eighteen. |

### R.3 Where this document is least reliable

Stated so future readers can discount appropriately:

- **Counts marked "~" are judgement calls** about whether a mention constitutes a request. Six
  analysts produced six different numbers for the same topics; the adversarial verification
  narrowed but did not eliminate the spread. Treat every count as **±30%** and the *ordering* as
  more reliable than the magnitudes.
- **Single-voice findings are labelled as such** (per-character window geometry, legacy
  clickable-link semantics, the bug-tracker critique). They are included because they are
  structurally significant, not because they are well-supported. **Do not let a good argument
  from one person become "players want".**
- **Reaction counts are a noisy signal.** The transcript format places `{Reactions}` blocks in
  positions that invite mis-attribution to an adjacent message. Reactions cited here were
  anchored to the message body, but any future counting pass must do the same deliberately.
- **Developer and staff statements are evidence about Saga's roadmap, not about player demand**,
  and were excluded from player counts wherever identified. That identification is itself an
  inference for a few borderline handles.

### R.4 What would improve this evidence most

In rough order of value per unit effort:

1. **Ask the two self-identified visually impaired players directly.** They volunteered publicly,
   they are named in the transcript, and their requirements are demonstrably better than
   guesswork. Nobody appears to have followed up with them.
2. **Watch the same thread for the next ten weeks.** Several findings here are provisional
   because the thread ends mid-fix (theme readability, keybind collisions, GPU rendering).
3. **Find where the feature requests went.** From 8/5 the real demand signal moved into Saga's
   in-app form and out of this corpus entirely.
4. **Primary research on accessibility beyond low vision** — the single largest evidence gap in
   this document.
5. **One DragonRealms source.** Cena supports both games; this corpus covers one.

---

*Sources: `reference/discord/saga-thread.txt` (4,979 messages, 143 distinct posters,
2026-07-09 → 2026-09-18). Counts recomputed directly from the transcript on 2026-09-18. Related
plan documents: [`09-gap-audit.md`](09-gap-audit.md) (the gaps this fills),
[`08-curated-behaviors.md`](08-curated-behaviors.md) (the decision under pressure in §P.1),
[`01-architecture.md`](01-architecture.md), [`02-frontend-seam.md`](02-frontend-seam.md).*
