# Fairness note — the standard for judging Lich, Vellum and eohunter

Written 2026-09-17. Read this before any document that evaluates the reference codebases.
It exists because an early draft of the Cena plan judged decade-old decisions by 2026
standards, which produces confident, useless conclusions.

---

## The rule

**They had to work with what they had.**

Every design decision in these codebases was made at a particular time, in a particular
language version, by a particular number of people, under particular compatibility
obligations. A decision that looks wrong in 2026 Rust may have been the only reasonable
option available when it was made — and, crucially, **the fact that better options exist
now does not mean the code was rewritten when they arrived.**

## The specific trap this document prevents

`lib/version.rb:5` currently reads `REQUIRED_RUBY = '4.0'`. Reading that alone, one might
conclude "Lich targets modern Ruby, so its design reflects modern Ruby's capabilities."

**That is wrong.** The Ruby 4.0 requirement landed on **2026-07-16** (PR #1444, *"fix(all):
minimum Ruby 4.0 required & migrate Ruby version check"*) — roughly two months before this
snapshot. Before that, Lich supported substantially older Rubies for years.

The code was **not rewritten because Ruby 4 came out.** It was written against Ruby 2.x and
3.0-era constraints, and it still carries their shape. `lib/global_defs.rb` — the 2,300-line
global-function DSL — has commit history on that path reaching back to at least **2023-09**,
and the Lich lineage substantially predates the current repo (created 2021-03-09; earlier
Lich versions long precede it).

**So: the current minimum Ruby version tells you nothing about the constraints under which
a given line of code was designed.** Always date the decision, not the repo.

## How to judge correctly

For each decision, ask in this order:

1. **When was this actually written?** Use `gh api "repos/OWNER/REPO/commits?path=FILE"`;
   the local clones are shallow and `git log` is useless.
2. **What was available then?** Ruby version, available gems, platform support.
3. **What obligations constrained it?** Backwards compatibility with an installed script
   corpus (474 `.lic` files), third-party frontends Lich does not control (StormFront,
   Wrayth, Genie, Profanity, Saga, Suks), an external protocol (Simutronics' XML/GSL,
   EAccess auth), and a small volunteer team.
4. **Given all that — was it the right call?**
5. **Separately: does the reason survive into Cena's situation?**

Steps 4 and 5 have **independent answers**, and the most valuable findings are where they
diverge: *correct for them, wrong for us*.

## The three verdicts worth distinguishing

| Verdict | Meaning | What Cena does |
|---|---|---|
| **Right for anyone** | Encodes real domain knowledge about these games or client architecture | Keep it, do not re-derive it |
| **Right for them, wrong for us** | A correct response to a constraint Cena does not have | Understand it, then diverge deliberately |
| **Workaround that calcified** | A stopgap that outlived its cause and was never revisited | Do not port it |

A fourth category exists and is the most dangerous: a **transfer trap** — an empirical
finding that is *true* in its original context but whose *cause* does not transfer.

**The archetype:** `lib/gemstone/combat/parser.rb:34-40` documents a measured result that
many small literal unions beat one big union (58µs vs 35µs/line). True — and a Ruby regex
artifact, because Ruby scans large alternations linearly. Rust's `RegexSet` and
`aho-corasick` **invert the conclusion**. Porting that finding's *strategy* would make Cena
slower; porting its *discipline* (measure, gate cheaply) is right.

## On tone

These are working systems that real people depend on daily. Lich has run the GemStone and
DragonRealms scripting ecosystem for years; VellumFE is essentially one person's client with
3,100+ tests; eohunter is 18,000 lines of hunting engine with an unusually honest
self-audit. Cena gets to start clean **because they did the hard work of discovering what
the problem actually is.**

Write like that is true, because it is.
