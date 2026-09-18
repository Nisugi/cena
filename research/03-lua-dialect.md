# Cena — The Lua Dialect Decision

> **STATUS: VOID AS A DECISION.** Cena has no embedded scripting language ([`08-curated-behaviors.md`](08-curated-behaviors.md)). Kept for (a) the regex research in §3, which still governs Cena's Rust-side matching, and (b) the record of why an embedded language was considered and set aside. **Do not implement anything in this document.**


Written 2026-09-17. Synthesis of five independent research angles (runtime comparison,
binding layer, regex engine strategy, coroutine scheduler design, ecosystem/precedent),
plus my own verification pass over the corpus at `E:\Cena\reference\` and the primary
sources.

This document settles the scripting-runtime stack. It is a **hard-to-reverse choice**:
it determines the language every ported script is written in, so §6 ("What would change
this decision") is not decoration — it is the tripwire list.

The user's ranking, verbatim, governs every tradeoff below:

> "what's important to me is performance and supports the most (regex, ect), cost (time
> to implement) is less important if I have to pick 2"

**Performance and capability rank joint-first. Implementation cost is explicitly the
thing being sacrificed.** Where this document recommends the more expensive option, that
is deliberate, and §5 prices it.

---

## 1. The decision

**Cena embeds Luau via `mlua` 0.12.x, runs one Lua VM per session on its own thread,
does all regex matching in Rust with `fancy-regex` over a `regex-automata` multi-pattern
dispatcher, and preempts runaway scripts with an arm-on-demand `set_interrupt`
watchdog.**

Concretely:

```toml
mlua = { version = "0.12", features = ["luau", "async", "vendored", "serde", "macros"] }
fancy-regex = "0.19"        # script-facing engine; delegates to regex-automata
regex-automata = "0.4"      # meta::Regex::new_many() hot-path dispatcher
# deliberately NOT "send" — LocalSet per session is lock-free (maintainer guidance)
# "luau-jit" on x86-64 desktop ONLY, and only after the §7 spike passes
```

| Layer | Choice | The one-line reason |
|---|---|---|
| **Runtime** | **Luau** | On iOS the JIT is *definitionally* unavailable, so the real contest is interpreter-vs-interpreter — and Luau ties LuaJIT's interpreter, beats PUC 5.4, and is the only candidate whose GC is optimized for thousands of mostly-idle coroutines. |
| **Binding** | **mlua 0.12.x** | Uncontested. The only live binding crate; rlua is archived, hlua has no async, piccolo has no string library. |
| **Regex** | **`fancy-regex` + `regex-automata` multi-pattern** | Supplies 100% of the corpus's capability (lookaround, backrefs) while keeping ~98% of sites on the linear-time engine. PCRE2 degrades 19x silently on mobile; Oniguruma is slower and thinly maintained. |
| **Scheduler** | **One VM/session, `LocalSet`, arm-on-demand interrupt** | Luau's `set_interrupt` is the only preemption mechanism that works without a compile-time patch, and it costs ~0.8% when not armed. |
| **Concurrency** | Scripts are `Thread`s via `into_async()`, in `FuturesUnordered` | Measured: 60 blocking-style scripts complete in 209ms, not 6s serial. |

### Why Luau, in one paragraph

The decisive fact is not a benchmark, it is a platform rule. Mobile is a driving
constraint for Cena, and **LuaJIT's JIT is hard-disabled on iOS at compile time**
(`LJ_OS_NOJIT`). I verified this at the primary source — [luajit.org/install.html](https://luajit.org/install.html)
says, in Mike Pall's own words: *"the JIT compiler is disabled for iOS, because regular
iOS Apps are not allowed to generate code at runtime … You'll only get the performance
of the LuaJIT interpreter on iOS."* So on the platform that constrains the design
hardest, choosing LuaJIT means choosing **LuaJIT's interpreter** — and there Luau is
roughly equal, while additionally offering preemption that works, a GC built for latency,
per-VM memory limits, and sandboxing. LuaJIT's genuine JIT advantage would apply to one
of four target configurations, in exchange for a *permanent* Lua 5.1 language freeze
(the cost Neovim documents and accepts because it never leaves the desktop). That is
paying a permanent price for a conditional benefit.

Crucially, **the regex requirement — 68% of the corpus — does not discriminate between
the dialects at all.** Every Lua variant ships only Lua patterns (no alternation, no
lookaround, no backreferences). Matching must therefore live in Rust regardless of
dialect. This reframing is the most useful thing the research produced: it moves the
capability question off the dialect axis entirely and onto the regex-engine axis, where
it is comfortably solved.

---

## 2. Comparison against the ranked criteria

Ordered by the user's ranking. "Cost" is last and is explicitly permitted to be bad.

### 2.1 Runtimes

| | **Luau** ✅ | LuaJIT | PUC Lua 5.4 | piccolo |
|---|---|---|---|---|
| **PERF — iOS / Apple Silicon** | Interpreter, ties LuaJIT's | **Interpreter only (JIT hard-disabled)**; arm64 JIT regressions on M-series | Slower interpreter | Unknown, unoptimized |
| **PERF — x86-64 desktop** | +codegen ≈ 1.6x behind LuaJIT JIT | **Best** | Baseline | Unknown |
| **PERF — GC pause** | **Incremental + PID pacing + paged sweeper + incremental coroutine marking** | GC 2.0, never rewritten — weak under large heaps | Generational, no coroutine optimization | Incremental, good design |
| **PERF — coroutine cost** | ~1.3 KB measured | ~410 B (UNVERIFIED) | ~1.12 KB measured | n/a |
| **CAP — regex** | Lua patterns only | Lua patterns only | Lua patterns only | **No `string.find`/`match`/`gmatch`/`gsub` at all** |
| **CAP — preemption** | **`set_interrupt`; can Yield (deschedule) not just kill** | **Hooks don't fire in JIT'd code** — needs undocumented `LUAJIT_ENABLE_CHECKHOOK` | `set_hook`, flat ~37% overhead | n/a |
| **CAP — sandbox / mem limit** | **Both** (`Lua::sandbox` is Luau-only) | Memory limit only | Memory limit only | Strong isolation by design |
| **CAP — language level** | 5.1 + selective 5.2–5.4 + gradual types | **5.1, permanently frozen** | 5.4, most modern | 5.4-ish, incomplete |
| **CAP — known gaps** | No 64-bit ints, no `goto`, no `__gc`, no `load` | No `goto`/`<close>`/int div | — | Enormous |
| **COST** ⬇ | **Highest**: C++ ~118K LOC, mobile toolchain risk, Roblox-flavored docs | Low | **Lowest**: plain C, Blightmud-proven | Catastrophic |
| **Maintenance** | 0.738, **2026-09-11**, weekly (verified) | Rolling, bus-factor-1, commit 2026-09-08 | Stable | **Last release 2024-06-16** ❌ |

### 2.2 Regex engines

| | **fancy-regex** ✅ | `regex` alone | PCRE2 | Oniguruma |
|---|---|---|---|---|
| **PERF (rebar geo-mean, lower=better)** | Delegates to rust/regex: **3.08** | **3.08** | jit 6.00 / **no-JIT 114.75** | Not in rebar; "considerably" worse than rust/regex |
| **PERF on mobile** | Unaffected (no JIT) | Unaffected | **19x silent cliff** (W^X blocks JIT) ❌ | Unaffected but slow |
| **PERF worst case** | Backtracking on ~2% of sites, `backtrack_limit` bounded | **Linear, guaranteed** | Exponential | Exponential |
| **CAP — lookaround** | **Yes** | **No** ❌ | Yes | Yes |
| **CAP — backreferences** | **Yes** | **No** ❌ | Yes | Yes |
| **CAP — Ruby semantics** | `oniguruma_mode()` (UNVERIFIED) | No | No | **Exact** (but not required) |
| **COST** ⬇ | Moderate | Lowest | High (C dep × 4 targets) | High (C dep, no mobile guidance) |
| **Maintenance** | **0.19.2, 2026-09-13** | Excellent | Excellent | ~1 substantive release in 33 months |

**`regex` alone is disqualified on capability** — it cannot express the corpus's
lookarounds or backreferences at all, and capability ranks joint-first. Blightmud is the
cautionary precedent: it shipped with `regex`-only and its users filed enhancement
issues against exactly those gaps. **PCRE2 is disqualified on performance** — on the
stated primary constraint (mobile) its entire case evaporates, silently, with no error.

---

## 3. Where the researchers disagreed — and how I adjudicate

Four genuine conflicts. I resolved each by going back to the corpus or the primary
source myself rather than averaging the claims.

### 3.1 CONFLICT: How many lookarounds are there really? (54 vs 87 vs 486)

This was the single largest disagreement, and it matters because it decides whether a
backtracking engine is forced onto the per-line hot path.

| Source | Claim |
|---|---|
| Census (`07-...`) | 486 across 85 files |
| Angle 1 (runtime) | 54 occurrences, 25 files; only 17 "live" |
| Angle 3 (regex) | 87 occurrences, 33 files |
| Angle 5 (ecosystem) | Trusted the census's 486 |

**All four are wrong, and I measured it myself.** The disagreement is entirely an
artifact of *file-glob scope* — Angle 1 counted only `*.rb`, Angle 3 counted only the
`scripts/` and `dr-scripts/` trees. Counting `*.rb` **and** `*.lic` across all three
trees:

```
grep -rhoE '\(\?(=|!|<=|<!)' --include=*.rb --include=*.lic lich-5 scripts dr-scripts
  -> 145 occurrences, 60 files
     (?=  66   (?!  56   (?<!  19   (?<=  4
```

Excluding `spec/`, `test/`, and the vendored `repos/` copies — i.e. code Cena would
actually port:

```
  -> 119 occurrences, 51 files
```

**Adjudication: the true live figure is 119 sites across 51 files.** That is ~2.2x
Angle 3's number and ~7x Angle 1's, but it is still only **~1.8% of the 6,447 `=~` call
sites.**

Angle 3's *diagnosis* of the census error is nonetheless correct and valuable: the census
pattern `(?<` matched `(?<name>` named captures as well as `(?<=`/`(?<!` lookbehind, and
its arithmetic (87 + 399 = 486) confirms the double-count on its own file scope. The
census's "486 lookarounds" should be struck.

**Why the correction does not change the recommendation:** at 54, 87, or 119 sites, the
conclusion is identical — ~98% of the regex surface runs on the linear-time engine, and
the fancy tail is small enough to sit behind a literal prefilter. Angle 3 put this well:
a higher count *strengthens* the fancy-regex case rather than creating it. What the
correction does change is the **porting estimate**: 119 hand-portable sites across 51
files, not 17 across a handful. That is priced in §5.

**UNVERIFIED:** my 119 figure counts regex-literal occurrences by grep, not by parsing
Ruby. Some may sit in comments or strings. The measurement that settles it: parse each
`.rb`/`.lic` with Ripper, extract only `regexp_literal` nodes, and re-count.

### 3.2 CONFLICT: In-pattern backreferences — "~28 files" vs "exactly 2"

The census claimed ~28 files; Angle 3 claimed exactly 2 genuine sites, arguing all other
`\1` occurrences are `gsub` *replacement* strings (not a regex-engine feature).

**Angle 3's reasoning is right but its count is too low.** My scan found Angle 3 missed
at least two real in-pattern backreferences outside its search scope:

```
lich-5/lib/common/authentication/web_login.rb:359
  body.scan(/id="(W_[A-Za-z0-9_]+)"[^>]*>\s*<label for="\1">.../)
scripts/scripts/ledger.lic:556
  line.gsub!(/<(compDef|inv|...)[^>]*>.*?<\/\1>/m, '')
scripts/scripts/briefcombat.lic:452
  full_line =~ /--exclude=(["']?)(.+?)\1(?:\s+|$)/
```

Plus **genuine `\k<name>` named backreferences**, which Angle 1 counted as 3 and Angle 3
declared to be **zero**. Both are wrong — there are 4, and at least two are live
production code, including one inside a *lookahead*:

```
lich-5/lib/games.rb:299
  /^<(?<xmltag>dynaStream|component) id='.*'>[^<]*(?!<\/\k<xmltag>>)\r\n$/
lich-5/lib/gemstone/combat/defs/supplements.rb:225   (a rule string referencing \k<name>)
```

**Adjudication: ~5 genuine in-pattern backreference sites, not 2 and not 28.** The
`games.rb:299` case is notable because it combines a named capture, a negative lookahead,
*and* a named backreference in one pattern — it is the single most demanding regex in the
corpus and it alone rules out the `regex` crate. Still ~0.08% of call sites; the
conclusion is unchanged, and fancy-regex handles all of them.

Angle 3's core methodological point stands and should be carried forward: **the
overwhelming majority of `\1` occurrences are replacement-string templating, not engine
features.** The census's "~28 files" conflated the two.

### 3.3 CONFLICT: Lua 5.4 vs Luau (Angle 2 vs Angles 1, 4, 5)

Angle 2 (binding layer) recommended `lua54`; Angles 1, 4 and 5 recommended `luau`. This
is the substantive dialect disagreement.

Angle 2's case is real and I want it on the record: **lua54 is the lower-risk mobile
build** (plain C, not ~118K lines of C++), it is what Blightmud ships in production on
this exact stack, the open Android issue ([mlua#724](https://github.com/mlua-rs/mlua/issues/724))
is Luau-JIT-specific and does not touch lua54, and it keeps 64-bit integers, `goto` and
`<close>`.

**I adjudicate for Luau, on the user's own ranking.** Angle 2's argument is almost
entirely a *cost and risk* argument — toolchain friction, build weight, ecosystem
familiarity. Those are the axis the user explicitly demoted. On the two axes ranked
first:

- **Performance:** Angle 4 measured Lua 5.4's count-hook overhead as a **flat ~37% floor
  that does not fall with sampling interval** (+38.7% at every 1,000 instructions,
  +37.3% at every 1,000,000) — any hook mask forces the VM off its fast dispatch path.
  Luau's interrupt costs ~19.7%, is pure FFI overhead, and **`remove_interrupt()`
  restores full speed (+0.8%)**, enabling an arm-on-demand watchdog that is free in
  steady state. For a client that must preempt runaway scripts across 3 sessions, this is
  a decisive structural advantage, not a marginal one.
- **Capability:** `Lua::sandbox` and `set_interrupt`'s *yield-instead-of-kill* semantics
  are Luau-exclusive in mlua. Descheduling a runaway script and keeping it alive is
  materially better behavior for a MUD client than killing it.

Angle 2's genuine contributions survive and are adopted regardless of dialect: **expose
`world` as `UserData` with lazy getters, never a materialized snapshot table** (mlua
[#318](https://github.com/mlua-rs/mlua/issues/318) measured ~14.7x slowdown on
cross-boundary table iteration), and **never cross the Lua↔Rust boundary per-pattern-
per-line**.

One point where Angle 2 corrected everyone else and was right: it read mlua's source and
established that **mlua's async does not use `lua_yieldk`** — it is a Lua-level
trampoline, so "cannot yield across a C-call boundary" does not apply. This defuses
Angle 5's §6 concern in its general form and means async support did not constrain the
dialect choice.

### 3.4 CONFLICT: Is `RegexSet` the right hot-path structure?

The existing plan (`09-hot-path-measurements.md` §5.1) and Angles 1 and 5 say
`RegexSet`. Angle 3 says `RegexSet` is the wrong tool and `regex_automata::meta::Regex::new_many()`
is correct.

**Adjudication: Angle 3 is right, decisively.** `RegexSet` reports only *which* patterns
matched — no byte offsets, no capture groups. Since Cena's entire model is deriving
**typed frames with fields**, `RegexSet` would force a second full match on every hit.
`meta::Regex::new_many()` returns `PatternID` **plus captures and offsets in one pass**.
`RegexSet` is a prefilter; it is not a dispatcher. `09-hot-path-measurements.md` §5.1
should be amended.

### 3.5 CONFLICT: Does Lich's "one big union is slower" result carry over?

Lich's combat parser measured 58µs vs 35µs/line and concluded a single combined union was
*slower* than several small gated ones. Angle 3 argues this **inverts** on Rust.

**Adjudication: Angle 3 is right.** Read Lich's own stated cause, at
`lib/gemstone/combat/parser.rb:35`: *"Ruby's regex engine scans large alternations
linearly."* That is a property of Ruby's backtracking engine, not of regex. Rust compiles
alternations into a single automaton with SIMD literal prefilters, where cost does not
scale linearly in branch count. **One large union is the correct structure on Rust
precisely because it was the wrong one in Ruby.** This is the most important thing not to
cargo-cult from Lich.

Corroborating: Lich hand-rolls literal extraction (`pattern_gate.rb:58-72`), a first-word
bucket index over ~2,400 patterns (`critranks.rb:86-114`), and notes at
`infomon/parser.rb:154-161` that unanchored scanning *"was ~half of all server-thread
CPU."* All three are hand-approximations of what `regex-automata` does natively.

### 3.6 Note: Angle 5's "50x headroom" claim vs everyone's optimization push

Angle 5 cited Mudlet's benchmark (5M regex-fails/sec vs Cena's ~100k/sec need) to argue
Cena has ~50x headroom and should not over-engineer. This sits in tension with Angles 1–3
all pushing hard on multi-pattern dispatch.

**Both are right about different things.** Angle 5's number correctly says *throughput is
not the binding constraint* — this should stop anyone from heroics in v1. Angles 1–3 are
right that `meta::Regex::new_many()` is *simpler* than the N-sequential-matches
alternative, not more complex, once you need `PatternID` + captures anyway. **Adopt the
structure because it is the cleanest correct design, not because throughput demands it.**
Do not spend time on Hyperscan-class optimization.

---

## 4. Consequences that are now design rules

These are load-bearing and each came out of a measured finding. They belong in
`01-architecture.md`.

1. **Never expose raw `coroutine.create`/`resume`/`wrap` to scripts.** Angle 4
   *reproduced* the mlua `poll_pending` sentinel leak: a script's own `resume` around an
   async call swallows the scheduler's sentinel and the thread parks forever
   ([mlua#418](https://github.com/mlua-rs/mlua/issues/418), closed *not planned*). Ship a
   scheduler-aware `spawn()` instead. This also absorbs the corpus's 37 `Thread.new` files.
2. **Keep all `create_async_function` calls in one binary.** The sentinel is an *address*,
   not a constant — async functions from a dynamically loaded `.so` would not be
   recognized as pending and would busy-loop at 100% CPU. **No dynamic plugin modules
   that export async functions.**
3. **Never yield inside a metamethod.** Luau supports yieldable `pcall` but *not*
   yieldable metamethods. Since 64.5% of the corpus calls `pause`/`sleep`, a `world` seam
   built on `__index` would fail **at runtime in production**, not at compile time. This
   reinforces rule 4.
4. **`world` is `UserData` with lazy field getters — never a per-frame snapshot table.**
   Boundary conversion is the real hot-path cost (mlua#318: ~14.7x).
5. **Matching is centralised in Rust. Lua receives only typed events for patterns that
   already matched.** The boundary tax would otherwise multiply by
   scripts × patterns × lines × sessions.
6. **No implicit `$1`..`$9`.** Ruby's match globals are implicit, thread-local
   action-at-a-distance. Under coroutine scheduling a `pause` between a match and its
   `$1` read lets another script clobber them — **a genuine data-race class, not a style
   preference.** Return captures explicitly via Lua multi-return, with a capture-free
   `re.test()` for the 713 `!~` and many of the 1,282 `case when` boolean sites.
7. **Default regex to multi-line (`(?m)`).** Ruby's `^`/`$` are *line* anchors; Rust's
   are *text* anchors. This is the highest-volume silent porting hazard in the entire
   corpus — a pattern that worked in Ruby silently matches less in Rust, with no error.
8. **Compile patterns once at subscription time; rebuild the dispatcher on subscription
   change, never per line.** Compilation is the one axis where Rust regex is genuinely
   slower than PCRE2 (rebar: 11.85 vs 1.41).

---

## 5. What this decision costs us

The user accepted higher implementation cost. That is not licence to leave it unstated —
an accepted cost still has to be budgeted. This is the honest bill.

### 5.1 The costs we are choosing to pay

**Mobile toolchain risk — the single largest item, and it is front-loaded.**
Luau is ~118K lines of **C++** (Lua 5.1 is ~14K of C). Cross-compiling C++ to
`aarch64-apple-ios` and `aarch64-linux-android` is materially harder than C. mlua has
documented vendored cross-compilation friction on aarch64, and I confirmed
[mlua#724](https://github.com/mlua-rs/mlua/issues/724) is **open with no resolution** —
Android aarch64 + Luau JIT fails on a missing `__clear_cache` symbol. That issue is
JIT-specific and does not block interpreter-mode Luau, but it is evidence the path is
less trodden. **UNVERIFIED and highest-risk:** I could not find any primary documentation
or public report of `mlua` + `luau` + `vendored` successfully building for
`aarch64-apple-ios`. mlua's CI covers aarch64 cross-compilation generally and WASM
explicitly excludes JIT, but iOS specifically is undocumented. **§7 Spike 0 exists to
retire this risk before anything else is built.**

**Choosing the harder build over the proven one.** Blightmud — the closest analogue in
existence, Rust + mlua + MUD — ships `lua54`. We are deliberately not copying the
battle-tested configuration. That is the user's ranking applied honestly, but it means we
forfeit "someone already proved this works" on the mobile build.

**Regex porting: ~119 lookaround sites across ~51 files, plus ~5 backreference sites.**
Note these do *not* need rewriting — fancy-regex supports them directly. The cost is
**verification**: each must be confirmed to behave identically under Rust semantics.

**The `$1`→explicit-return rewrite: 1,666 uses across 99 files.** This is the largest
single mechanical cost in the port. It is unavoidable under *any* dialect or engine
choice, so it is not a differentiator — but it is real, and design rule 6 means it cannot
be shortcut by emulating Ruby's globals.

**The `^`/`$` anchor audit.** Every ported pattern must be checked against the
line-vs-text anchor divergence. Mitigated by defaulting to `(?m)`, but "mitigated" is not
"eliminated" — this is a silent-failure class.

**Luau's language gaps.** No 64-bit integers (all numbers are doubles, 53 bits of
lossless integer precision), no `goto`, no `__gc`, no `load`/`loadstring`, no
to-be-closed variables. Measured impact: corpus scan found **zero** integer literals
≥10^16 and only 3 numeric bitwise sites (covered by `bit32`); only 9 files use `eval`,
and host-side loading via `luau_load` is unaffected. So the practical cost is near zero —
but it is a permanent language-level constraint.

**Documentation noise.** Script authors searching for Luau help will find **Roblox**
answers with Roblox-specific globals and idioms. Lua 5.4 does not have this problem, and
the existing MUD-scripting corpus (Mudlet, MUSHclient) is all Lua 5.1-shaped. Mitigated
by luau-lsp being genuinely good and by Luau's 5.1 syntax compatibility.

**Ecosystem thinness outside Roblox.** Embedders have reported support *"basically
nonexistent"* outside Roblox and inadequate docs for custom integrations. The Luau team
concedes this and cites the Lute standalone runtime as investment. Direction of travel is
right; 2026 is early.

**Two engines to reason about.** `fancy-regex` routes internally, but we still maintain a
split: the `meta::Regex` dispatcher for the hot bulk, and a small prefiltered list for the
~124 fancy sites. Two code paths, two failure modes.

**Luau's VM baseline is 16x Lua 5.4's** (measured: 403 KB vs 24 KB). At 3 sessions that
is 1.2 MB vs 73 KB — irrelevant against the GC isolation win, but it is a real number on
mobile and worth knowing.

### 5.2 Costs we are NOT paying (worth stating, because they were expected)

- **Rewriting lookarounds into post-match guards** — not needed; fancy-regex supports
  them. The brief expected to pay this; it evaporates.
- **A C dependency for regex** — avoided entirely by rejecting PCRE2 and Oniguruma. The
  mobile build carries **one** C++ dependency (Luau), not two.
- **Reimplementing the Lua string library** — the piccolo tax, avoided.
- **Per-script OS threads** — avoided by the coroutine design; measured 60 scripts in
  209ms on one thread.

---

## 6. What would change this decision

This is a hard-to-reverse choice. These are the specific tripwires — each is a fact that,
if it turns out otherwise, should force a revisit *before* Phase 5 commits.

### 6.1 Would flip the runtime to Lua 5.4

1. **`mlua` + `luau` + `vendored` cannot be built for `aarch64-apple-ios` or
   `aarch64-linux-android`** after reasonable effort. This is the live, unretired risk
   (§7 Spike 0). Lua 5.4 is the fallback: plain C, Blightmud-proven, lowest toolchain
   risk. It loses `set_interrupt` and `sandbox` and takes the flat 37% hook cost, but
   keeps everything else.
2. **Any GemStone/DragonRealms server value used in *arithmetic* exceeds 2^53.** Luau's
   double-only numbers become a correctness risk; PUC 5.4's native 64-bit integers win.
   Corpus scan found zero such literals, but this is **UNVERIFIED against live server
   XML**. Settle it by scanning captured XML for integer fields > 9007199254740992. Note
   the escape hatch: if such a value exists but is only ever an *identifier*, it is a
   string in Lua, not an arithmetic value, and Luau survives.
3. **Luau's release cadence stalls.** I verified weekly releases through 0.738
   (2026-09-11). A multi-month gap is the early warning. The MIT licence (Nov 2021) makes
   a fork legally possible, so relicensing is not itself a killer.
4. **Arm-on-demand interrupt proves unworkable** — e.g. the watchdog cannot arm fast
   enough to catch a tight loop before it starves the session. This would negate Luau's
   main structural advantage over 5.4.

### 6.2 Would flip the runtime to LuaJIT

5. **Mobile is dropped as a requirement.** If Cena becomes desktop-x86-64-only, LuaJIT's
   JIT advantage becomes unconditional and the Lua 5.1 freeze becomes the only cost. This
   is the *sole* scenario where LuaJIT wins, and it is a product decision, not a technical
   one.
6. **`LUAJIT_ENABLE_CHECKHOOK` is shown to be cheap in practice** *and* iOS ceases to
   matter. Both must hold; the flag's own source comment warns it "may be quite expensive
   in tight loops."

### 6.3 Would flip the regex engine

7. **`fancy-regex`'s delegation proves not to hold** — i.e. profiling shows plain patterns
   are *not* actually running on the `regex-automata` path. This would remove the whole
   basis for choosing it. Measure by comparing identical plain patterns under
   `fancy_regex::Regex` and `regex::Regex`.
8. **A pathological backtracking case is found in live game text** that `backtrack_limit`
   cannot bound acceptably. Mitigation exists (set the limit to 10k–50k on the hot path,
   far below the 1,000,000 default) and failure is a *recoverable error*, not a hang — but
   a genuine DoS vector on attacker-influenceable text would force all-linear-engine plus
   hand-porting the ~124 fancy sites.
9. **The true lookaround count is off by another order of magnitude** (i.e. thousands, not
   ~119). Would force more traffic onto the backtracking path. My Ripper-based
   re-measurement (§3.1) settles this.

### 6.4 Would change the scheduler shape

10. **Sessions need to migrate across OS threads.** Would force the `send` feature and its
    reentrant-mutex locking, serializing script execution. Currently avoided by
    one-VM-per-session on a `LocalSet`.
11. **`luau-jit` native codegen changes interrupt cost substantially** on desktop.
    **UNVERIFIED** — all interrupt measurements are interpreter-mode x86-64 Windows.

### 6.5 Would NOT change the decision

- piccolo becoming maintained again — it would still need an entire string library.
- `oniguruma_mode()` failing to cover the `^`/`$` case — explicit `(?m)` solves it; engine
  unchanged.
- Higher-than-expected `meta::Regex` rebuild cost — solved by caching and a two-tier
  stable/volatile split; engine unchanged.

---

## 7. Prototype / benchmark plan

The smallest experiment set that validates the recommendation before Phase 5 commits.
**Ordered by risk-retired-per-hour.** Spike 0 is a genuine go/no-go gate; run it first and
alone.

### Spike 0 — Mobile build gate (GO/NO-GO) ⚠

**The only spike that can invalidate the runtime choice. Nothing else should be built
until it passes.**

- Build a trivial Rust binary with `mlua = { features = ["luau", "async", "vendored"] }`
  for **`aarch64-apple-ios`** and **`aarch64-linux-android`**.
- Run a script that does `pause(0.05)` and a `string.find` on a real device (or simulator
  + emulator if devices are unavailable — note the weaker evidence).
- **Measure:** does it link? does it run? binary size delta vs `lua54`? peak RSS?
- **Also build the `lua54` equivalent** for both targets, as the control and fallback.
- **Pass:** both Luau targets build and run. **Fail → adopt Lua 5.4** and re-run §3.3 with
  the interrupt advantage removed.
- Explicitly do **not** enable `luau-jit` here — mlua#724 is open and JIT is not needed on
  mobile anyway.

### Spike 1 — Regex capability and semantics conformance

- Extract every distinct regex literal from the corpus (Ripper `regexp_literal` nodes —
  this doubles as the §3.1 re-measurement).
- Attempt to compile each under `fancy-regex`. **Measure:** how many fail, and why.
- For the ~119 lookaround + ~5 backreference sites, and especially
  `lich-5/lib/games.rb:299` (named capture + negative lookahead + named backreference in
  one pattern), run each against recorded game lines under Ruby and under fancy-regex and
  **diff the match results**.
- Run the same corpus under `oniguruma_mode(true)` and `(false)` and diff. **This settles
  the UNVERIFIED `oniguruma_mode` semantics question** (Angle 3 could not retrieve the
  docs).
- **Measure:** count of patterns where Ruby and Rust disagree, bucketed by cause (anchor
  semantics, Unicode `\w`/`\b`, empty-match, leftmost-first vs leftmost-longest).
- **Pass:** every disagreement has a known, mechanical fix.

### Spike 2 — Hot-path dispatcher throughput

- Load the ~2,395 crit patterns + ~1,082 combat defs into one
  `regex_automata::meta::Regex::new_many()`.
- Replay a recorded session log (real game text, not synthetic).
- **Measure:** µs/line for (a) `meta::Regex` multi-pattern one-pass, (b) N sequential
  `regex::Regex` matches, (c) `RegexSet` prefilter + second match on hit. Report p50 and
  **p99** — tail latency is what a live client feels.
- **Measure separately:** dispatcher *rebuild* time as a function of pattern count, since
  subscription churn is the one real perf risk in the design.
- **Expected:** (a) wins and Lich's 58µs-vs-35µs finding inverts (§3.5). If (a) loses to
  (b), that is a genuine surprise — investigate before proceeding.
- Run on **both** desktop and the mobile device from Spike 0.

### Spike 3 — Scheduler under realistic multi-session load

Angle 4's harness at
`C:\Users\<USER>\AppData\Local\Temp\claude\e--Cena\c5699bde-ad04-4784-b2f3-569a7e2aba84\scratchpad\cotest`
already proves the mechanism at 60 coroutines. Extend it to Cena's real shape:

- **3 session VMs × ~20 scripts each**, all blocking on `pause`/event-await, fed by
  replayed game text from Spike 2 at realistic line rates.
- **Measure:** wall-clock vs ideal; per-VM `used_memory()`; **GC pause p99 per session**
  (the number that justified one-VM-per-session — Angle 4 measured 11.95ms shared vs
  22.4µs isolated, a 534x gap).
- **Measure:** interrupt cost armed vs unarmed **with the real regex hot path running**,
  since that interaction is untested (Angle 4's +19.7%/+0.8% figures are on a synthetic
  string-match loop).
- **Verify the escalation ladder end to end:** `VmState::Yield` to deschedule a zero-yield
  runaway → `Err` to kill after N slices → `Thread::reset(before_dying)` for cleanup.
  Confirm neighbours and the VM survive.
- **Verify the cancel-while-parked-in-a-Rust-future path** — dropping the `AsyncThread`
  then running cleanup via `reset`. This is a *distinct* path from interrupt-kill and is
  the one most likely to be missed.
- **Verify** `pcall`/`xpcall` + `debug.traceback` survive a yield (Angle 4 confirmed on
  x86-64; re-confirm on ARM).
- Repeat on the mobile device. **All existing scheduler measurements are x86-64 Windows;
  none are ARM.**

### Spike 4 — Boundary cost calibration

- Run mlua's own criterion suite (`cargo bench --features luau,vendored,async`) on desktop
  **and on the mobile device**.
- **Measure:** `function [call Rust sum]` vs `function [call Lua sum]` (the boundary tax);
  `userdata [call index]` vs `table [get and set]` (validates design rule 4 — `world` as
  lazy `UserData` vs snapshot table).
- **Pass:** `userdata [call index]` is competitive, confirming the lazy-getter `world`
  design. If materialized tables win decisively, revisit rule 4.
- Also confirm the `Thread::reset` pooling win (Angle 4 measured 2.2x: 289ns create vs
  131ns reuse) holds on ARM.

### Cross-cutting

Every spike reports **desktop and mobile** numbers. The largest gap in the entire research
body is that **all measurements to date are x86-64 Windows**, while mobile is the stated
driving constraint. A result that only holds on desktop has not validated this decision.

---

## 8. Open items explicitly marked UNVERIFIED

| Claim | How to settle |
|---|---|
| `mlua`+`luau`+`vendored` builds for `aarch64-apple-ios` | **Spike 0** — highest risk in the plan |
| No server value used in arithmetic exceeds 2^53 | Scan captured XML for integer fields > 9007199254740992 |
| `oniguruma_mode()` covers the `^`/`$` line-anchor case | **Spike 1** differential test |
| My 119 lookaround / ~5 backref counts are grep-based, not parsed | **Spike 1** Ripper-based re-count |
| Interrupt cost under `luau-jit` native codegen | **Spike 3**, desktop only |
| LuaJIT ~410 B/coroutine (Mike Pall, lua-l 2009) | Original message not retrievable; moot given LuaJIT rejection |
| LuaJIT "will never support arm64e" (PAC incompatibility) | Widely repeated, primary source not found; moot given rejection |
| Whether the GSL/XML feed can emit non-UTF-8 bytes | Capture a raw session log and validate. If yes, use fancy-regex `bytes_mode()` on the inbound path — a correctness bug regardless of engine choice |
| LuaJIT vendored build fails on Windows/MSVC (`luajit-src` 210.7.3) | Observed once by Angle 4; incidental given rejection |

---

## 9. Amendments to existing plan documents

- **`inventory/07-script-corpus-api-census.md`** — strike "486 lookahead/lookbehind uses
  across 85 files"; replace with **119 live sites across 51 files** (§3.1), noting the
  `(?<` named-capture double-count. Strike "~28 files use backreferences"; replace with
  **~5 genuine in-pattern backreference sites** (§3.2), noting that the overwhelming
  majority of `\1` occurrences are `gsub` replacement templating, not engine features.
- **`inventory/09-hot-path-measurements.md` §5.1** — replace the `RegexSet`
  recommendation with `regex_automata::meta::Regex::new_many()` (§3.4), and record that
  Lich's "one big union is slower" result **inverts on Rust** (§3.5).
- **`plan/01-architecture.md` §0.2** — add design rules 1–8 from §4, particularly **"never
  yield inside a metamethod"** (a production-only failure mode) and **"no implicit
  `$1`..`$9`"** (a data-race class under coroutine scheduling).

---

## Sources

**Verified by me during this synthesis:** corpus counts via grep over
`E:\Cena\reference\{lich-5,scripts,dr-scripts}` (§3.1, §3.2);
[luajit.org/install.html](https://luajit.org/install.html) iOS JIT statement;
[Luau releases](https://github.com/luau-lang/luau/releases) 0.738 / 2026-09-11 / weekly
cadence; [mlua#724](https://github.com/mlua-rs/mlua/issues/724) open, Luau-JIT-specific.

**Runtime:** [luau.org/performance](https://luau.org/performance/) ·
[compatibility](https://luau.org/compatibility/) · [sandbox](https://luau.org/sandbox/) ·
[why](https://luau.org/why/) · [64-bit int RFC](https://rfcs.luau.org/type-long-integer.html) ·
[LuaJIT lj_record.c](https://github.com/LuaJIT/LuaJIT/blob/v2.1/src/lj_record.c) ·
[LuaJIT #1072](https://github.com/LuaJIT/LuaJIT/issues/1072) ·
[LuaJIT extensions (resumable VM)](https://luajit.org/extensions.html) ·
[Jipok/Lua-Benchmarks](https://github.com/Jipok/Lua-Benchmarks) ·
[luanti#14611 (arm64 JIT regressions)](https://github.com/minetest/minetest/issues/14611)

**Binding:** [mlua repo](https://github.com/mlua-rs/mlua) ·
[docs.rs/mlua](https://docs.rs/mlua/latest/mlua/) ·
[discussion #494 (multi-VM)](https://github.com/mlua-rs/mlua/discussions/494) ·
[issue #318 (14.7x boundary cost)](https://github.com/mlua-rs/mlua/issues/318) ·
[issue #418 (poll_pending sentinel)](https://github.com/mlua-rs/mlua/issues/418) ·
[VmState](https://docs.rs/mlua/latest/mlua/enum.VmState.html) ·
[rlua (archived)](https://github.com/mlua-rs/rlua) ·
[piccolo COMPATIBILITY.md](https://github.com/kyren/piccolo/blob/master/COMPATIBILITY.md)

**Regex:** [fancy-regex](https://docs.rs/fancy-regex/latest/fancy_regex/) ·
[PERFORMANCE.md](https://github.com/fancy-regex/fancy-regex/blob/main/PERFORMANCE.md) ·
[regex-automata](https://docs.rs/regex-automata/latest/regex_automata/) ·
[rebar](https://github.com/BurntSushi/rebar) ·
[regex internals](https://burntsushi.net/regex-internals/) ·
[pcre2jit](https://www.pcre.org/current/doc/html/pcre2jit.html) ·
[sljit#99 (Apple Silicon MAP_JIT)](https://github.com/zherczeg/sljit/issues/99)

**Precedent:** [Blightmud](https://github.com/Blightmud/Blightmud) ·
[Blightmud v3 plan #194](https://github.com/Blightmud/Blightmud/issues/194) ·
[Mudlet Technical Manual](https://wiki.mudlet.org/w/Manual:Technical_Manual) ·
[Mudlet trigger benchmark](https://forums.mudlet.org/viewtopic.php?t=22836) ·
[MUSHclient regexp](https://www.gammon.com.au/scripts/doc.php?general=regexp) ·
[SLua FAQ](https://wiki.secondlife.com/wiki/SLua_FAQ) ·
[Neovim Lua discussion #33011](https://github.com/neovim/neovim/discussions/33011)

**Local:** `lich-5/lib/gemstone/combat/parser.rb:35` ·
`lib/gemstone/combat/defs/pattern_gate.rb:58-72` · `lib/gemstone/critranks.rb:86-114` ·
`lib/gemstone/infomon/parser.rb:154-161` · `lib/games.rb:299`
