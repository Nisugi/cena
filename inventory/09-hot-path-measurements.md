# Hot-path measurements — the workload the script runtime must feed

Measured 2026-09-17 against `E:\Cena\reference\`. Gathered to ground the Lua dialect and
regex-engine decision ([`../plan/03-lua-dialect.md`](../plan/03-lua-dialect.md)) in the
real matching load rather than in general benchmarks.

---

## 1. Pattern scale — how many regexes run against game text

| Source | Count | Note |
|---|---:|---|
| Crit rank patterns (`lib/gemstone/critranks/`) | **~2,395** | 46,163 LOC of generated tables |
| Combat def regexes (`lib/gemstone/combat/defs/`) | **1,082** | across 13 files |
| Combat parser patterns (`combat/parser.rb`) | 49 | the dispatch layer |
| Creature templates (`lib/gemstone/creatures/`) | 628 files | 7,744 LOC |

**This is the central performance fact.** Cena derives typed events centrally, so it will
run *more* matching than Lich does, not less — every inbound line may be tested against
thousands of patterns, per session, with 3+ sessions live.

## 2. Lich's own measured gating result

`lib/gemstone/combat/parser.rb:34-40`, verbatim:

> **NOTE ON SCALE:** each def family gates itself with a small literal union
> (PatternGate). A single combined prefilter union was tried and measured **SLOWER (58µs
> vs 35µs/line on real logs)** — Ruby's regex engine scans large alternations linearly, so
> one big union costs more than several small ones. When def counts grow into the
> hundreds-per-family, the proven fix is a **first-word bucket index** (see CritRanks —
> 2,389 patterns, indexed by first literal word), not a bigger union.

**Archetype correction (from the decision interrogation):** the measurement is obsolete but
**the forward-looking advice survives** — *"the proven fix is a first-word bucket index …
not a bigger union."* Literal-prefix dispatch beating linear alternation is a real
algorithmic insight, and it is **exactly what `RegexSet`/`aho-corasick` implement
natively**. So Cena should *not* hand-roll bucket indices; it gets the same win for free.
Correct conclusion, obsolete implementation.

**Two conclusions, and they point in opposite directions:**

1. **Port the discipline, not the strategy.** The 58µs-vs-35µs result is a *Ruby regex
   artifact* — Ruby scans large alternations linearly. Rust's `RegexSet` compiles
   alternations into a single automaton and `aho-corasick` does multi-literal prefiltering
   in one pass, which **inverts the conclusion**. The right Rust structure is likely the
   big combined matcher Ruby could not afford.
2. **The first-word bucket index is still sound**, and is essentially hand-rolled
   Aho-Corasick. It confirms the workload shape: thousands of patterns, most sharing
   literal prefixes, nearly all failing on any given line.

**Implication for the dialect decision:** the regex engine's *multi-pattern* story matters
more than its single-match speed. An engine that must test patterns sequentially from Lua
will lose regardless of how fast each individual match is.

## 3. Corpus regex feature dependencies

From `07-script-corpus-api-census.md` §4.2, plus direct measurement:

| Feature | Uses | Files | Rust `regex` supports? |
|---|---:|---:|---|
| `=~` matching | 6,447 | **310 (68%)** | yes |
| Non-capturing `(?:…)` | 2,176 | 149 | yes |
| `$1`..`$9` implicit globals | 1,666 | 99 | n/a — needs explicit captures |
| `gsub`/`sub` | 1,001 | 145 | yes |
| `case when /regex/` | 1,282 | 129 | yes |
| **Lookahead/lookbehind** | **486** | **85** | **NO** |
| Named captures | 399 | 64 | yes |
| `Regexp.union`/`.escape`/`.new` | 369 | 96 | partial |
| `.scan` | 133 | 64 | yes |
| **Backreferences in patterns** | — | **~28** | **NO** |

Measured directly for this document:
- **148 files** use `^` line anchors in regexes. Ruby's `^`/`$` are **line** anchors by
  default; Rust's are **text** anchors unless multi-line mode is set. This is a silent
  semantic difference that will produce wrong matches rather than errors.
- Backreferences appear **inside match patterns**, not only in replacements — e.g.
  `/--exclude=(["']?)(.+?)\1(?:\s+|$)/`. This rules out `regex`-crate-only.
- **Zero** uses of atomic groups `(?>` or possessive quantifiers — the corpus does not
  need the exotic end of PCRE.

**Conclusion: `regex` alone is insufficient.** 486 lookarounds across 85 files plus
backreferences in ~28 files mean the engine must support backtracking constructs
(`fancy-regex`, `pcre2`, or `onig`), or those scripts cannot be expressed.

## 4. Per-line matching consumers in the corpus

| Mechanism | Files | Note |
|---|---:|---|
| `DownstreamHook` | 63 | filters/observes every inbound line |
| `Watchfor` | 1 | the legacy trigger mechanism, nearly dead |

`Watchfor` being effectively unused (1 file) while `DownstreamHook` has 63 is a useful
signal: the corpus already prefers a **filter on the stream** over a **trigger registry**.
Cena's typed-event bus is the natural successor to the former.

## 5. What this means for the dialect decision

1. **Multi-pattern matching architecture dominates single-match speed.** Prioritize an
   engine with `RegexSet`-style set matching and literal prefiltering.
2. **Matching must happen in Rust, not Lua.** With thousands of patterns per line, crossing
   the FFI boundary per pattern would be fatal. Scripts should *register* patterns and
   receive typed events; the matching runs Rust-side against a compiled set.
3. **The engine must support lookaround and backreferences**, or ~85 files' worth of
   capability is simply unavailable.
4. **Line-vs-text anchor semantics must be handled explicitly** — either by defaulting to
   multi-line mode or by documenting it loudly. 148 files are exposed.
