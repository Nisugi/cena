# Gap Audit — what the plan has not looked at, or not looked at closely enough

Written 2026-09-18. An audit of the eight planning documents against what a **shipped**
game client actually requires. Ordered by risk, not by size.

The plan so far is strong on *architecture* and weak on *product*. That is a normal place to
be at this stage, but several of the gaps below are load-bearing and one is potentially
project-defining.

---

## Tier 1 — Could change the project

### 1.1 Licensing — RESOLVED (2026-09-18) ✅

**Cena is a private project for private use by the creator and copyright holder of
VellumFE.** That settles this entirely.

| Source | License | Status for Cena |
|---|---|---|
| **VellumFE** | GPL-3.0-or-later | **Own work.** The copyright holder is not bound by their own licence; reuse and relicense at will. |
| **lich-5** | BSD 3-Clause | Permissive. Private use and derivatives fine; retain the notice if source is ever redistributed. |
| **dr-scripts** | has a LICENSE | Check terms only if code is reused; not currently planned. |
| **scripts** (GS) | none | No grant — but private, undistributed use raises no practical issue. |
| **eohunter** | none | Own work. |

GPL obligations attach to **distribution**, not to authorship or private use. Nothing here
blocks the parser port (Phase 1) or any other planned reuse.

**The author wrote both VellumFE and eohunter**, which are the two largest sources Cena
draws on. Between them plus BSD-licensed Lich, essentially every planned reuse is either own
work or permissively licensed.

**One thing to keep true for the future.** If Cena is ever distributed publicly, two items
return:
1. Vellum's GPL would bind *other people's* contributions to Vellum, if any exist.
2. The unlicensed **community** scripts (GS `scripts/`, i.e. bigshot and the rest — *not*
   eohunter) carry no grant. Cena's behaviors should therefore be implemented from
   **documented behavior and observed game mechanics**, not translated line-by-line from
   other people's unlicensed Ruby. This is what
   [`08-curated-behaviors.md`](08-curated-behaviors.md) already describes — rebuilding what
   those scripts *accomplish* as first-class actions — so the plan is already on the right
   side of it.

*No action required. Recorded so the question is not reopened.*

### 1.1a What eohunter's authorship changes methodologically

Not a licensing point — a research one. eohunter is **own work**, so it can be ported
directly rather than reimplemented. But two earlier conclusions should be re-read in that
light:

- **The three-seam interrogation is now partly self-assessment.** `04` Decision 3 treats
  `world.rb`'s header rules as "a rationalization written by someone who had already built
  it." That framing stands — it is how anyone's post-hoc design docs read — but the author is
  available to answer directly. **Where the interrogation had to infer intent from artifacts
  (marked `INFERRED:`), ask instead.** That is strictly better evidence than archaeology.
- **eohunter is a specification, not just a reference.** `08` sets the conformance target as
  *"does Cena's Hunt behavior do what eohunter does?"* Since the author wrote it, its profile
  format, its policy model, and its hard-won combat knowledge are directly transplantable —
  and the parts that were **Lich workarounds** (the `World` delegation wall, the 50ms polling
  floor, the global `Events` bus, the refusal to start a second session) are known to be
  workarounds rather than intent. The author can say which is which faster than any dig.

### 1.2 Accessibility — zero coverage, and it is a regression

**Grep of all eight plan documents for "accessib", "screen reader", "TTS": no hits.**

VellumFE ships `src/tts/` (821 lines). Users depend on it. A client that drops
text-to-speech is not a superset of what it replaces — it is a regression for the people who
need it most, and MUD clients have a meaningful blind/low-vision user base.

**Action:** TTS is a requirement, not a nice-to-have. It also has architectural
implications: speech is driven from *frames*, not from rendered UI text, which means it
belongs at `cena-ui` and works across all frontends including headless. Design it in now;
retrofitting speech onto a rendering layer is painful.

---

## Tier 2 — Substantial unplanned scope

### 2.1 The entire user-customization surface (~47,000 lines in Vellum)

The plan discusses frames, sessions, behaviors and layers. It never mentions the features
users actually touch every day:

| Subsystem | Vellum LOC | In the plan? |
|---|---:|---|
| `src/config/` | **32,694** | mentioned only as "the config system" |
| `src/theme/` | 5,302 | no |
| `src/launcher/` | 3,522 | no |
| `core/highlight_engine.rs` | **2,260** | **no** |
| `window_position/` | 1,307 | no |
| `src/tts/` | 821 | **no** (see 1.2) |
| `core/hotbar.rs` | 759 | no |
| `src/sound.rs` | 530 | no |
| `core/custom_emoji.rs` | 367 | no |
| `core/inline_image.rs` | 149 | no |

Vellum's own user documentation (`book/src/customization/`) lists what people expect:
**highlights, keybinds, layouts, themes, skins, sounds, emoji, inline images, UI packs**.

**Highlights are the important one.** A 2,260-line engine is not incidental — pattern-based
text colouring is *the* most-used customization in every MUD client, and it is a **per-line,
per-pattern hot-path consumer**, which means it belongs in the same multi-pattern matching
design as everything else
([`../inventory/09-hot-path-measurements.md`](../inventory/09-hot-path-measurements.md)).
It should be designed alongside the matcher, not bolted on.

**Under curated behaviors this gap grows.** With no user scripting, *configuration is the
only customization surface a user has.* It has to be excellent. The plan currently treats it
as an implementation detail.

### 2.2 Triggers, aliases and macros — the other thing scripting used to provide

Related to 2.1 but distinct. In Lich, users write a one-line script for "when X appears, do
Y." With no user scripting, Cena must provide this **as configuration**, or lose a capability
every competing client has.

This is the strongest pressure on the curated-behaviors decision, and
[`08-curated-behaviors.md`](08-curated-behaviors.md) names the exact tripwire: *"profiles
growing conditional logic."* Triggers are conditional logic. Designing them well — expressive
enough to be useful, constrained enough not to become a programming language — is a real
design problem the plan has not touched.

### 2.3 Settings, profiles and migration

`05-engineering-rules.md` §5.4 says "persistence is a deliberate act" and Phase 3 now
specifies the state model, but nothing covers:

- **The profile format itself** — now the primary user interface (see 2.1).
- **Schema migration.** Cena will change its config format. Vellum has `src/migrate.rs`;
  Lich has migration history. Users must not lose settings on upgrade.
- **Per-character vs per-account vs global scope.** Lich has `Settings`/`CharSettings`/
  `GameSettings`; the distinction is real and users rely on it.
- **Where files live** on Windows/macOS/Linux/Android/iOS, and what `--data-dir` means.

### 2.4 Distribution, install and update

The plan ends at "Phase 9: desktop frontends" — it never ships to a user.

- **Self-update.** Lich has `lib/update.rb` and update workflows. Vellum ships releases.
  Mobile stores have their own rules — App Store review, Play Store signing.
- **Installers** per platform; code signing and notarization (macOS) — a real, slow,
  bureaucratic prerequisite that blocks release if discovered late.
- **Release/versioning process.** Lich uses release-please with a structured CHANGELOG.
- **Crash reporting and diagnostics** — how does a user report a bug usefully?

### 2.5 Error handling, logging and observability as a design

`05` mentions logging in passing. Missing: the error taxonomy (what is recoverable, what
kills a session, what kills the process), log levels and destinations, redaction of
credentials (Lich has `credential_scrub.rb` precisely because ARGV and ivars leak), and what
a user sees when something breaks.

---

## Tier 3 — Named but not examined closely enough

### 3.1 EAccess authentication — flagged as highest-risk, never studied

`01` Phase 2 calls it *"the highest-risk reimplementation — an unmovable external
protocol,"* and `04-inherited-decisions.md` covers the *session* layer, not the auth
handshake. Nobody has read `lib/common/authentication/` closely enough to know what
reimplementing it involves: the SGE protocol on :7910, the HTTPS fallback (PR #1570),
`entry_store.rb`'s encrypted saved logins (1,131 lines), and credential scrubbing.

**This gates Phase 2, which gates everything.** It deserves the same depth of dig the six
inherited decisions got.

### 3.2 Reconnect, disconnect and session lifecycle

Mentioned in passing. Not designed: what happens to in-flight command round-trips on
disconnect, whether typed state is preserved or reset (this interacts directly with the new
stream-desync rule in `01` §0.1a), whether behaviors pause or abort, and how reconnection
interacts with the session registry.

### 3.3 The GameAdapter trait is a sketch

`01` §4 proposes a three-method trait. The DragonRealms inventory shows DR scrapes far more
from text than GemStone does. Whether that trait shape survives contact with real DR
requirements is untested — and Phase 8 is a long way from Phase 1, which is exactly when a
wrong abstraction is most expensive.

### 3.4 Performance budget

We have measured the *workload* (~2,395 crit patterns, 1,082 combat regexes per line) but
never stated a **target**. What is acceptable per-line latency on a mid-range phone with 3
sessions? Without a number, "fast enough" is unfalsifiable and the benchmark plan has no
pass criterion.

### 3.5 Data provenance and updates

Listed as open question 5 in `01` and never resolved. Crit tables and creature templates
come from Lich (BSD — fine with attribution). But how do they *update*? A new creature
shouldn't require a Cena release — `08` §"What it costs" leans on data-driven updates as the
mitigation for slow iteration, so this mechanism is load-bearing.

---

## Tier 4 — Deliberately deferred, worth naming

- **Game rules and ToS.** `07` §4 addresses automation policy for agents. Not addressed for
  *curated behaviors*, which is arguably a bigger question: Cena shipping a hunting engine is
  a different posture than a user writing one. Worth an explicit position.
- **Multi-account/multi-game.** Vellum has `core/multiaccount/` (1,533 lines).
- **The web frontend's own security.** It binds a local port with a token; threat model
  unexamined.
- **Windows/macOS/Linux platform quirks.** Lich has a whole `wine.rb` and a
  Windows-specific CI job.

---

## What I would do next, in order

1. ~~Settle the license~~ — **done 2026-09-18** (§1.1). Private project, own copyright.
   Phase 1's parser port is unblocked.
2. **Dig EAccess** (3.1) at the depth the inherited decisions got. It gates Phase 2.
3. **Design the customization surface** (2.1, 2.2) — under curated behaviors this *is* the
   product, and highlights must be co-designed with the matcher.
4. **Add accessibility to the architecture** (1.2) while frames are still being designed.
5. **Set a performance budget** (3.4) so Phase 1's benchmarks can pass or fail.

**A note on private scope.** "Private project for private use" also *shrinks* several Tier 2
items, and the plan should take the discount honestly:

| Item | Effect of private scope |
|---|---|
| 2.4 Distribution/install/update | **Largely deleted.** No installers, no code signing, no App Store review, no self-update infrastructure. Build and run it. |
| 2.5 Crash reporting | Simplified to good local logs — the only user who files bugs is the author. |
| 1.2 Accessibility | **Depends on you.** If you do not need TTS, it drops to optional. Keeping frames→speech feasible costs nothing now. |
| 2.1 Customization surface | **Unchanged and still Tier 2.** You are the user; the config surface still has to be good, and highlights are still hot-path. |
| 2.2 Triggers/aliases | **Unchanged.** Still the main pressure on curated-only. |
| 4 Game rules/ToS | Still worth a position, but a personal one rather than a policy. |

The architectural gaps (EAccess, reconnect, GameAdapter, performance budget) are unaffected —
they are about whether the thing works, not about who ships it.

Items 3 and 4 both argue for the same thing: **the plan needs a product document to sit
beside the architecture ones.** Everything written so far describes how Cena is built.
Almost nothing describes what using it is like.
