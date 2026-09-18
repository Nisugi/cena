## USER  (2026-09-18T02:23:42)

https://github.com/elanthia-online/lich-5
https://github.com/Nisugi/VellumFE

Here's a thought lich-5 + vellumFE = one new rust program?
Give it some real thought. What does lich have that vellum doesn't?

---

## CLAUDE  (2026-09-18T02:23:46)

I'll look at both codebases before answering, since the interesting part is what lich actually does beyond being a proxy.

---

## CLAUDE  (2026-09-18T02:25:37)

Both repos are cloned fresh. I'll have two agents survey them in parallel so I can compare capabilities rather than skim.

---

## CLAUDE  (2026-09-18T02:26:02)

While those run, one thing I want to check myself: the third-party script ecosystem, since that's the real lock-in for lich regardless of the core.

---

## CLAUDE  (2026-09-18T02:26:07)

Waiting on the two survey agents now. Nothing else to gather until they report.

---

## CLAUDE  (2026-09-18T02:29:43)

Both repos are surveyed. Short version: Vellum has already eaten most of Lich's plumbing. What's left in Lich is one thing, and it's the thing that doesn't port.

## What Vellum already duplicates

Vellum (about 310k lines) already has its own version of most of the Lich core: eAccess login for GS and DR, the Wrayth XML parser and a full game-state model, mapdb loading with Dijkstra travel, native Jinx repo fetching, bounty, group, experience, encumbrance, and an automation lease with go2, foreach, sorter and item moving. It also parses about a quarter of the mapdb's Ruby wayto edges into native walk actions. Most of the direct-connect work was explicitly done to remove the Lich dependency on mobile. So on the transport, protocol, and data side, "merge" mostly means "delete the Ruby copy".

## What Lich has that Vellum doesn't

Roughly in order of how hard it is to replace:

- **A programmable runtime with 15 years of scripts on it.** Vellum has no embedded language at all. Lich's value is `eval` against a per-script binding, threads per script, goto via re-eval, and about 200 global functions (waitfor, matchfind, dothistimeout, fput, move, checkhealth and friends). Everything on top is dr-scripts, elanthia-online/scripts, and third-party repos. You have 77 scripts in your own local install alone. Vellum's own go2 executor hands the un-transpilable 75 percent of wayto edges to Lich via semicolon-e for exactly this reason.
- **Ruby embedded in data.** Map waytos, timeto edges, and spell formulas are StringProcs, meaning executable Ruby stored in JSON. A Rust program can't consume the shared mapdb or spell-list without a Ruby interpreter or a full rewrite of the data format that the whole community shares.
- **The game-knowledge libraries.** Infomon, PSMs, Spell, Effects, Stash, the creature template library, critranks, armaments, societies, the new combat tracker, and the DR commons (DRC, DRCI, DRCT and so on). About 19k hand-written lines for GS plus 13k for DR, before the 144k of generated tables. These are what scripts actually call into.
- **Being a proxy for nine frontends.** Wrayth, Wizard, Profanity, Genie, Avalon, Frostbite, Saga, Suks, and Vellum itself. Detachable multi-client, SAL launching, frontend registry, hosts-file redirection. A merged Rust program serves one frontend and forks the community.
- **Script-facing persistence and hooks.** UserVars, CharSettings, GameSettings marshalled into sqlite, upstream and downstream hook registry with owner cleanup, Watchfor, the events pub/sub, script lifecycle guards and supervised children, the semicolon-command dispatcher, and the lich5-update script repo sync.
- **Lich WebUI.** Ruby-backed panels that Vellum already renders natively over a WebSocket bridge.

## My honest take

A ground-up "Lich in Rust" is the wrong merge. The core runtime is maybe 10k lines and would be a fun port. The problem is the API contract is Ruby semantics: dynamic bindings, method_missing settings proxies, monkeypatched String, threads you can kill with Thread#raise, and data files that are Ruby. You can't be compatible without being Ruby, and if you're not compatible you've launched a new scripting platform with zero scripts. That is a bigger community lift than any amount of Rust.

Three paths that could actually work, best first:

1. **Vellum owns the session and hosts Lich as a component.** You're already most of the way there: the SSH launcher cold-starts headless Lich, and the semicolon-e bridge exists. Make Vellum the process that spawns and supervises `lich.rbw` in pipe mode, exposes one config, one launcher, one install. To the user it's one program. Lich keeps serving other frontends and the script ecosystem stays intact.
2. **Embed CRuby in the Vellum binary** with magnus or rutie, and run Lich's lib in-process with Vellum's parser and network replacing Lich's `games.rb` and xmlparser. Genuinely one executable. Cost: Lich's runtime is entangled with its own XMLData singleton and thread pumps, and you'd be maintaining a fork of lich-5 forever. Mobile builds get much harder.
3. **Add a native scripting tier to Vellum** (Rhai or Lua) for the automation people actually write today: simple triggers, hunting loops, timers. Keep Lich for the heavy scripts. This is the "new platform" route, but scoped so it doesn't need to replace dr-scripts to be useful. Realistically most Vellum-native automation so far has been done this way already, just in Rust instead of a script language.

If you do want one codebase eventually, path 1 now sets up path 2 later without betting the ecosystem on it.

---

## USER  (2026-09-18T02:46:11)

nope it needs to be one program. for mobile devices. all of your obstacles I think we can overcome. 
I think we can make an entirely new program that does what lich and vellum does all in one. Able to consume the ruby scripts either through live transation or through an external translation. Live would be better and I've already proven the ability to shim gtk ui's into a webui duplicate. I think a rust compiled binary would be able to handle a ruby translator without showing any kind of lag.

---

## CLAUDE  (2026-09-18T02:47:14)

Agreed then. Here's how I'd actually build it, given both codebases as they stand today.

## The one decision that matters: don't translate Ruby, host it

"Live translation" of Ruby into something else is the trap. Lich scripts lean on eval, method_missing, Thread.new, regex globals like $1, and data files that are literally Ruby (every mapdb wayto, every spell formula). Any translator that isn't a Ruby interpreter will hit a script it can't run within the first week, and the ecosystem stops trusting it.

The winning move is the reverse: embed a real Ruby VM inside the Rust binary, and re-host Lich's pure-Ruby libraries on top of a Rust core that replaces Lich's plumbing. Scripts run unmodified. Vellum's parser, state, network, mapdb, and UI become the substrate. That's "one program" and it's zero-translation compatible.

Speed is a non-issue. Lich scripts are I/O bound, blocked on the next game line 99 percent of the time. The interpreter choice is about semantics and mobile build, not lag.

## Which Ruby VM

| Option | Compatibility | Mobile build | Verdict |
|---|---|---|---|
| CRuby via magnus | Full. Threads, eval, binding, Marshal all just work | Cross-compiles for Android NDK. iOS is possible but painful, no JIT, static lib | Best compatibility, riskiest build |
| mruby | No Thread, no Marshal, partial eval/binding, no C gems | Trivial on both, designed for embedding | Needs a Thread-to-Fiber shim and a Marshal replacement |
| Artichoke (Ruby in Rust) | Incomplete and stalled | Trivial | Not ready |
| Own interpreter | You define the subset | Trivial | Same trap as translation |

My recommendation is CRuby if the cross-compile spike succeeds, mruby as the fallback. The first two weeks of the project should be nothing but proving CRuby links into the iOS and Android shells you already have. That's the go/no-go, and everything else is known-doable engineering.

## Architecture

Four layers, only two of them new:

1. **Vellum core, as is.** Network, eAccess, parser, GameState, mapdb, layout, all four frontends.
2. **Script runtime crate, new, in Rust.** This is Lich's script.rb, sharedbuffer, hook registry, watchfor, settings store, and the semicolon-command dispatcher, rewritten as Rust host primitives. Script registry and lifecycle including pause, kill, supervised children, and execution guards. A per-script downstream cursor over the line buffer, which is what waitfor and matchfind block on. Upstream and downstream hooks with owner cleanup. Settings persistence in rusqlite. Rough size is 15k to 25k lines.
3. **Ruby compatibility layer, mostly vendored from lich-5.** A rewritten global_defs that maps the 200 globals onto the Rust primitives. XMLData as a facade over Vellum's GameState. GameObj, Room, Map, and Stash backed by Rust data. StringProc works unchanged because there's a VM, so Vellum's wayto transpiler becomes an optimization instead of a coverage gap. Spell, Infomon, PSMs, Effects, the creature library, and the entire DR commons tree can be pulled in close to verbatim since they're pure Ruby over XMLData and the globals.
4. **Your GTK-to-WebUI shim** covers the handful of scripts that draw windows.

## The two problems the phone forces on you

**Blocking.** Lich's model is one OS thread per script that parks in waitfor. CRuby gives you exactly that, with the Rust host waking a condvar when a matching line lands. With mruby you'd run each script as a Fiber under a Rust scheduler and shim Thread.new onto fibers, which works for most scripts but breaks anything that expects preemption. Another reason to fight for CRuby.

**Background execution.** iOS kills your socket when the screen locks. Android will eventually too. A hunting script running on the phone dies mid-swing. So "one program" needs to mean one binary with two roles: the same executable runs headless on a home box or VPS as the session owner, and the phone instance attaches to it. You already have the SSH launcher and detach protocol. Now both ends are the same code, and the scripts run wherever the session lives. This also sidesteps the App Store rule against downloading executable code, since the phone can be a pure viewer when it has to be.

## Phasing

1. **Spike.** Embed CRuby via magnus in desktop Vellum. Run one real .lic script against Vellum's parser with a minimal waitfor and fput. In parallel, attempt the iOS and Android static builds. Two weeks, go/no-go.
2. **Runtime crate on desktop.** Full script lifecycle, buffer cursors, hooks, settings. Port global_defs. Target: the dr-scripts suite and the elanthia-online core scripts run unmodified, validated against real logs.
3. **Vendor the libraries.** Spell, Infomon, PSMs, GameObj, Map, Stash, DR commons. Retire the Ruby-side parser and network entirely.
4. **Mobile build and the headless role.** Same binary, session owner plus attached viewer.
5. **Then** optionally add a native Rhai or Lua tier for people who never want Ruby. Not before.

If you want, I can write this up as a design doc in the VellumFE repo with the crate boundaries, the host-primitive API the Ruby layer calls, and the spike checklist.

---

## USER  (2026-09-18T02:47:14)

and by ruby translator I just mean to translate the scripts into whatever scripting language we design at run or prerun.

---

## CLAUDE  (2026-09-18T02:47:21)

That changes the question to "how much of Ruby do the scripts actually use", which is measurable. Let me census the real corpus before I opine.

---

## CLAUDE  (2026-09-18T02:49:05)

Now I have real numbers instead of vibes. I pulled every script from dr-scripts, elanthia-online/scripts, and your local install, 487 scripts and 268k lines, and grepped for which Ruby features they actually use.

## What the corpus really uses

The scripts are mostly plain, boring Ruby. The features that appear in a majority of files are classes and modules, case/when, regex with $1 captures, blocks and lambdas, rescue, Time, Hash and Array methods, and the Lich API surface: Settings, hooks, watchfor, Infomon, Spell, GameObj, Room, Map, and the DR commons.

The scary dynamic stuff is confined to a small tail:

| Feature | Scripts | Notes |
|---|---|---|
| eval | 10 | bigshot, lnet, go2-style wayto execution, cartographer, combat-trainer |
| binding | 11 | mostly the same files |
| method_missing | 21 | cheap to support in a dynamic VM, not actually hard |
| Fiddle FFI | 6 | Win32 only, will never run on a phone anyway |
| Gtk windows | 47 | your GTK-to-WebUI shim covers these as an API facade |
| Thread.new | 41 | needs a real scheduler, this is the one that matters |

So a target language has to be a Ruby subset, not a new syntax. Not because of dogma, but because 268k lines of classes, blocks, regex captures, and exception flow have no cheap translation into anything that isn't Ruby-shaped. Inventing syntax buys you nothing and costs you the corpus and every author's muscle memory. Extend via stdlib and host APIs, not grammar.

## The design that follows

**Parse with Prism, execute in your own Rust VM.** Prism is Ruby's official parser, written in C with an official Rust crate. It parses the full language in milliseconds, so you never write a Ruby parser, and "live" versus "pre-run" stops being a choice: you parse at script load, optionally cache lowered bytecode on mobile for startup. The whole project becomes "write an interpreter for Prism's AST", which is a well-bounded problem.

**What the VM must implement**, in priority order from the census:

1. Object model: classes, modules, include and extend, blocks, procs, lambdas, yield, exceptions, catch and throw, Struct, OpenStruct, respond_to, is_a, send, define_method, method_missing, instance_variable_get. All of these are natural in a dynamic VM. Only eval and binding are hard, and those are 10 files you can wrap or hand-port.
2. Core types with Ruby semantics: String, Symbol, Integer, Float, Array, Hash, Range, Regexp with $~ and $1 globals, Time, Comparable, Enumerable. Regex via fancy-regex, which you already use for Lich's Ruby-authored gameobj patterns. Expect a short list of Onigmo divergences to paper over.
3. Concurrency: Thread, Mutex, Queue, sleep. Implement as green threads on a Rust scheduler where every blocking Lich call (waitfor, sleep, get, matchtimeout) is a yield point. That gives you Lich's thread-per-script model without OS threads and makes pause and kill deterministic, which is nicer than what CRuby gives Lich today.
4. Stdlib shims backed by Rust crates: yaml, json, net/http, fileutils, time, uri, ostruct. That's the entire top-required list except gtk and fiddle.
5. The Lich API layer: the 200 globals, XMLData over Vellum's GameState, GameObj, Map with StringProc waytos now executing natively in the VM, Settings on rusqlite, hooks, watchfor, Script lifecycle. Then vendor Spell, Infomon, PSMs, and the DR commons as Ruby source running on your VM.

**Test oracle for free.** Every script runs today under CRuby. Record game log sessions, replay them into both Lich and your VM, diff the upstream commands. That's how you know the interpreter is right, script by script, before anyone in the community touches it.

## Size and sequencing

A Ruby-subset VM on Prism is roughly 30k to 50k lines of Rust, about the size of one Vellum frontend. The Lich runtime layer is another 15k to 25k. The heaviest scripts on the metaprogramming list, bigshot, go2, cartographer, lnet, are also the ones Vellum already partly implements natively, so they can be last.

Order I'd take:

1. Prism crate linked into Vellum, AST walker that runs a 184-line median script with echo, waitfor, fput, and Settings. Two to three weeks to a proof.
2. Object model and core types until 200 of the 222 dr-scripts load and run against replay logs.
3. Green-thread scheduler and hooks. Now the multi-thread scripts and the hooking ones work.
4. Vendor the Lich Ruby libraries, delete the parser and network they no longer need.
5. Mobile builds, plus the two-role binary so a session can live on a home box while the phone attaches. Background execution on phones is an OS limit, not something the language fixes.

The census script and corpus are in my scratchpad if you want the raw per-file breakdown. I'd also be happy to write this up as a design doc in the VellumFE repo with the VM feature matrix and the replay-oracle harness spec.

---

## USER  (2026-09-18T02:50:48)

Hmm and what are our other options? What about a full port of what lich offers and forget the existing scripts, they can be recreated in something new. People hate ruby anyways.

---

## CLAUDE  (2026-09-18T02:51:33)

Fair question, and the clean break is more defensible than it sounds. Here's the full option space with honest costs.

## The options

| Option | Runtime work | Corpus | Mobile | Main risk |
|---|---|---|---|---|
| A. Ruby-subset VM on Prism | Big, 30k to 50k Rust | Runs unmodified | Fine | Long tail of Ruby semantics, and you're maintaining a Ruby |
| B. Clean break, new language | Same Lich runtime port, plus a much smaller embed | Zero, rewrite everything | Fine | Adoption. Day one it runs nothing people use |
| C. Clean break plus a Ruby-to-new transpiler for the boring 90 percent | B plus a Prism-based translator | Bootstraps most scripts, hand-fix the rest | Fine | Translator quality, and translated code is ugly to maintain |
| D. Embed CRuby via magnus | Smallest | Runs unmodified | iOS painful, Android OK | Cross-compile and binary size, plus you never escape Ruby |
| E. No language, declarative triggers plus Rust-native automations | Vellum's current path | None | Best | Ceiling. Someone always needs a loop with state |
| F. WASM plugin host | Medium | Zero | Fine | Great for tools, awful for a hunting script someone writes at 11pm |

## What a full port of Lich actually costs, language aside

This part is the same for A, B, and C, so don't let it swing the decision:

- **Runtime**: script lifecycle, buffer cursors and matching primitives, hooks, watchfor, settings store, semicolon commands. Roughly 15k to 25k Rust.
- **Game libraries**: Spell, Infomon, PSMs, Effects, Stash, GameObj classification, societies, armaments, the combat tracker, and the DR commons. About 30k lines of hand-written Ruby game logic, maybe 20k in Rust, plus 144k lines of generated tables that become JSON.
- **Data with embedded Ruby**: mapdb waytos and spell formulas. Without a Ruby VM you have to define a replacement expression format and migrate the shared data. That's a community-wide data change, not just your repo.

## The real cost of B is DragonRealms

"People hate Ruby" is true of authors, and there are maybe a dozen active ones. Players don't care what language it is. What they care about is whether their stuff works on day one. For GS that's bigshot, go2, loot, and a handful of others, and Vellum already owns go2 natively. Rewriting the GS load-bearing set in a new language is a few months of your time and is honestly plausible.

DR is different. The 222 dr-scripts are the game. Nobody plays DR without them, they're maintained by a group you don't control, and they lean on the DRC commons, which is another 8k lines. A clean break either drops DR support or commits you to rewriting and then maintaining a fork of dr-scripts forever. That's the single decision hiding inside "forget the existing scripts".

## If you go clean break, which language

- **Lua via mlua**: tiny, proven on iOS and Android, coroutines give you blocking-style waitfor naturally, fastest to embed. Weak strings and no regex, so you bolt on a regex library. 1-indexing annoys people.
- **TypeScript on QuickJS**: biggest author pool by far, JSON native, types help script quality. One QuickJS runtime per script on its own thread gives you Lich's blocking model exactly. Larger binary than Lua, still small.
- **Rhai or Rune**: Rust-native and pleasant, but tiny communities and no editor tooling. You'd be the ecosystem.
- **Python via RustPython**: authors would like it, but it's slow, large, and immature on mobile.

I'd pick TypeScript if you want authors to show up, Lua if you want the smallest most boring runtime.

## My recommendation

Go with C, not B. Build the Lich runtime and libraries in Rust behind a language-neutral host API. Pick TypeScript or Lua as the first-class language. Then write a Prism-based translator that converts the boring Ruby subset, which the census says is most of the corpus, into the new language as a one-time bootstrap. It doesn't have to be perfect, it has to get dr-scripts and the GS core to 80 percent so humans finish the rest. The language-neutral host API also means a Ruby-subset VM can be added later if DR adoption demands it, without redoing anything.

The one question I'd settle before writing code: are you willing to carry DR, or is this GS-first with DR as a later ask? That answer decides whether the translator is a bootstrap tool or a hard requirement.

---

## USER  (2026-09-18T02:51:33)

So what would be the best scripting language for a rust backend?

---

## CLAUDE  (2026-09-18T02:52:07)

Short answer: **Luau via mlua**, with **TypeScript on QuickJS** as the alternative if you'd rather optimize for author pool than runtime simplicity. Here's why, scored against what this program actually needs.

## What the embedding has to do

- Run on iOS and Android, which rules out any JIT engine. V8 and deno_core are out on iOS.
- Blocking-style scripts. Authors write `waitfor("You feel")` and expect the line to block. That needs cheap per-script coroutines or threads with deterministic pause and kill.
- Heavy regex and string work over game lines.
- Sandboxing, since scripts come from repos, and hard kill of a runaway script.
- Hot reload and per-script state isolation.
- Editor tooling and a language people will actually write at 11pm.

## The contenders

| Language | Embed crate | Mobile | Blocking model | Regex/strings | Tooling | Verdict |
|---|---|---|---|---|---|---|
| Luau | mlua | Proven, it's Roblox's engine | Coroutines, ideal | Weak, host provides regex | luau-lsp, type annotations | Best runtime fit |
| TypeScript on QuickJS | rquickjs | Fine, no JIT needed | Runtime per script on its own thread | Excellent | Best in class | Best author fit |
| Rhai | rhai | Fine | No coroutines, no classes | Basic | Minimal | Too small for hunting scripts |
| Rune | rune | Fine | Async native | Decent | Immature | Interesting but you'd be the community |
| Python | PyO3 on CPython 3.13 | Officially supported since 3.13, still rough | Threads, GIL is fine for I/O bound | Excellent | Excellent | Closest to Ruby ergonomically, 30MB binary and mobile embed pain |
| Boa (JS in Rust) | boa | Fine | Same as QuickJS | Good | TS tooling | Not production-ready yet |

## Why Luau wins on the runtime side

- **Coroutines map exactly onto Lich's script model.** Each script is a coroutine on a Rust scheduler. Every blocking host call yields. Pause, kill, and timeouts become scheduler operations instead of thread signals, which is strictly better than what Lich has.
- **Built to host untrusted user scripts in a game.** Sandboxing, memory and instruction limits, and interrupt callbacks are first-class, not bolted on.
- **Tiny and fast.** No allocator surprises, no GC pauses that matter, compiles to a few hundred KB.
- **Gradual typing** gives you real type checking for the host API without forcing it on casual authors.
- **mlua is mature.** Exposing Rust structs and functions is a few lines each, and your GameState can be handed in as userdata without copying.

The weak spot is strings and regex. Lua patterns are not regex. You solve it by exposing a host regex module backed by fancy-regex, which you already ship, so `re.match(line, "^You (\\w+)")` returns captures. That is a day of work and removes the objection. The 1-indexing complaint is real but people get over it.

## When to pick TypeScript instead

If your goal is to attract new authors rather than convert the existing dozen, TypeScript has ten times the pool, the best editor experience, and JSON everywhere. QuickJS embeds cleanly and one runtime per script on its own OS thread gives blocking semantics for free. Costs are a bigger binary, no coroutines so kill is thread-based like Lich today, and slower host calls than Luau. It's a good choice, just not the lightest one.

## What I would not pick

Python is tempting because it's the closest thing to the Ruby scripts and translation would be nearly mechanical, but embedding CPython on iOS is still rough and you'd be carrying a 30MB interpreter into a mobile app. Rhai and Rune are pleasant Rust-native languages, but neither has a community, and you'd be writing every tutorial yourself.

Decision rule: runtime simplicity and mobile first means Luau. Author growth first means TypeScript. Either way, keep the host API language-neutral so switching or adding a second language later is a binding layer, not a rewrite.

---

## USER  (2026-09-18T02:57:48)

ok a lot to think about but let me add another wrinkle. What if we also added multi session?

---

## CLAUDE  (2026-09-18T02:57:52)

You've already written a proposal on this, so let me read it and check what Vellum's multiaccount module does before answering.

---

## CLAUDE  (2026-09-18T02:58:38)

Multi-session is actually the strongest argument for the new program, and your own proposal is the evidence. Those 567 lines exist because Lich is one Ruby process per character with global state, so coordination means discovery files, advisory locks, tokens, per-session listeners, and owner-death handoff. In one Rust process a session is a struct and there are N of them. Most of that document evaporates.

## What changes in the design

The catch is that it only evaporates if you build it in from day one. Retrofitting multi-session onto a single-session core is how Lich got here.

- **No global state, anywhere.** No XMLData singleton, no current-client global, no "the" GameState. Vellum's core already has an AppCore with state inside it, so this is mostly a discipline of making AppCore a per-session value and putting a SessionManager above it. Every parser, buffer, hook registry, settings scope, and script scheduler is keyed by session id.
- **One scheduler per session.** A stuck or CPU-heavy script on character A must never stall B. With Luau coroutines that's one scheduler task per session on the shared tokio runtime. Kill, pause, and shutdown are per session.
- **Script ownership stays with one session.** Keep the distinction from your proposal, because it's still right in-process. A script is owned by the session that started it. It can read any other session's state for free, since it's shared memory behind a snapshot, and it can request actions on another session, but those requests go through that session's command queue and upstream hooks so the target's own scripts see them. Emergency decisions like fleeing and death handling stay local to the session.
- **Cross-session API becomes trivial.** Something like `sessions()`, `session("Bob").state.vitals`, `session("Bob").send("stance defensive")`, plus a pub/sub bus for events. That's the EOHunter leader and follower pattern without DRb, hunt IDs over sockets, or acknowledgement tables. Order receipts become return values.
- **Attach protocol multiplexed from the start.** The headless host runs four characters. A phone opens one connection and gets all four as tabs, or picks one. If you design the detach wire format as single-session now, you'll rewrite it.
- **Auth and settings already fit.** One eAccess login yields a character list, multiple accounts are just multiple logins, and settings are already scoped by game and character. Keyring holds all of it.

## What Vellum already has

The multiaccount module models peers across separate Vellum instances over the web sidecar, with staleness and group clustering. In the merged program that model stays, but the transport for local peers becomes an in-process channel. Keep the sidecar path for characters running on another machine, so the two-machine case still works.

## What gets harder

- **UI.** Focus, input routing, per-session notification and sound policy, and where alerts from a background character surface. This is real design work and it's the part users judge.
- **Resource use on phones.** Four parsers and four script schedulers on a phone is fine for CPU, but four sockets in the background is exactly what mobile OSes kill. Multi-session makes the headless host role a requirement rather than a nice-to-have.
- **Testing.** The replay oracle becomes multi-log: replay four recorded sessions in lockstep and assert the cross-session script produced the right commands on each.

## The net effect on the earlier plan

It doesn't change the language choice or the runtime port, it just adds one rule: everything is per session, and session is an explicit value in the host API from the first commit. The cost is small if you start that way and enormous if you don't. It also gives you a clean answer to the maintainers' review of your Lich proposal: rather than fighting process boundaries in Ruby, the coordination contract they were debating becomes the native shape of the new program.

---

## USER  (2026-09-18T02:59:45)

And just to make sure I understand fully, what vellum lacks that lich has is scripting. Lich is a scripting engine, it's modules and all are basically just for that period. So saying just a scripting engine is wrong. We would need to recreate all the methods and stuff in rust. Hrm

---

## CLAUDE  (2026-09-18T03:00:14)

Yes, that's the right understanding, and it's the real cost. Lich's value isn't the interpreter, it's the 200 globals and the dozen libraries scripts call into. Every one of those needs a Rust equivalent exposed to the new language. But the scope is smaller than the repo size suggests, so let me size it honestly.

## Lich by layer, and what Vellum already covers

| Layer | Lich lines | Vellum already has it? | Left to build |
|---|---|---|---|
| Generated data tables (creature templates, crit tables, armaments) | 147k | Bestiary is baked from the creature templates | Convert to JSON, no logic to write |
| Transport, auth, frontend launching, detach, GUI login, update, Jinx | ~25k | Yes, nearly all of it | Little |
| Parser, XMLData state, GameObj, inventory, map, pathfinding | ~8k | Yes | GameObj classification rules from the YAML |
| Script runtime: lifecycle, buffers, hooks, watchfor, settings, semicolon commands | ~7k | No | All of it, ~15k Rust |
| The 200 globals | 2.4k | Partial, via automation lease | All of it, but see below |
| Game knowledge: Spell, Infomon, PSMs, Effects, Stash, societies, combat, bounty, group | ~19k GS | Bounty, group, exp, effects, encumbrance | Spell, Infomon, PSMs, Stash, societies, combat tracker |
| DR: drinfomon and the DRC commons | ~13k | Only login and exp | All of it, if you carry DR |

Hand-written Lich is about 60k lines. Vellum already has an equivalent for roughly a third. The remaining two thirds is the port, call it 35k lines of Ruby logic becoming maybe 30k of Rust plus bindings.

## The globals are less scary than the count

Most of the 200 are one-liners over state that Vellum already tracks. checkhealth, maxmana, checkstance, checkroom, checkpcs, checkleft, stunned, prone, and about 60 more are a field read. The direction helpers are 14 wrappers on one move function. The detachable client block is 15 functions you don't need. The real work is maybe 30 functions with logic:

- the wait and match family, which is the buffer cursor design
- dothistimeout, selectput, and matchfind semantics with their exact edge cases
- move and multimove, including the failure-line tables that decide whether a move succeeded
- empty_hands, fill_hands, and the Stash container logic
- cast and the Spell object with its prep, cost, duration, and stacking rules
- waitrt and the roundtime math

Those thirty are where scripts break if you get them subtly wrong, and they're the part worth porting by reading the Ruby line by line with a replay log next to it.

## The libraries are the actual grind

Spell, Infomon, PSMs, and the DR commons are years of accumulated game knowledge: regex for every status line, which spells stack, cman costs by rank, society step logic. None of it is hard, all of it is tedious, and it's the reason I floated hosting Ruby instead. Since you've ruled that out, the mitigation is to pull as much as possible into data rather than code. Spell definitions, PSM tables, society tasks, and status-line regex should be JSON or TOML that the Rust engine loads, so the port is mostly transcription, and so the community can maintain them without touching Rust.

So the honest picture: a new scripting language is the small part. Rebuilding the Lich API and libraries is the project, and it's on the order of 45k to 60k lines of new Rust and data. That's comparable to what you've already built in Vellum, which is the best evidence it's doable, just not quick.

---

## USER  (2026-09-18T03:02:38)

Rebuilding the api isn't the hard part in my context. THe information there, it's just changing it. It's the unknown that's troublesome. So Let's say we port all the methods and modules from lich out of ruby and into rust. The world is at our fingertips. What would you choose for scripting? I think string and regex matching is important and needs to be quick. I also think performance on mobile is important. and a really cool ask would be multi session capable so I can run 2 or 3 + characters at a time and swap between them with the front end.

---

## CLAUDE  (2026-09-18T03:03:04)

**Luau.** With your three requirements it's not close.

## Why

- **Regex speed isn't the language's job.** Put matching in Rust. Scripts register patterns, the host compiles them once with regex or fancy-regex, folds every active watchfor and waitfor into one RegexSet or Aho-Corasick pass per incoming line, and only wakes the script whose pattern hit, handing it the captures. Scripts never scan lines in a loop the way Lich scripts do today. That single design choice makes matching faster than Lich by an order of magnitude regardless of language, and it means the language just needs cheap host calls. Luau has the cheapest.
- **Fastest embeddable interpreter that runs on iOS.** No JIT allowed there, and Luau's interpreter is tuned exactly for that constraint. It's roughly two to five times faster than QuickJS on interpreter benchmarks and lighter on memory. Roblox runs it on low-end phones with thousands of scripts.
- **Coroutines are the script model.** Each script is a coroutine. waitfor yields, the host resumes it when a line matches. Pause, kill, and timeouts are scheduler operations. No OS threads, no thread-raise hacks.
- **Multi-session is one Luau state per session.** States are cheap, tens of KB. Character A's scripts run in A's state on A's scheduler, B's in B's. A hung script can't touch B, memory is isolated, and swapping the frontend between them is just which session the view points at. Cross-session calls go through a host API with the same ownership rule as before: read anyone, command only through their queue.
- **Sandboxed and killable by design.** Instruction limits, memory caps, and interrupt callbacks are built in, since Luau was designed to run untrusted user scripts inside a game.
- **Gradual types.** Casual authors write untyped Lua. Serious authors get real type checking against a typed host API, and luau-lsp gives editor completion for every Lich-equivalent function.

## What you give up

Lua patterns instead of regex in the language, which you replace entirely with the host module. 1-based indexing. A smaller author pool than JavaScript, though bigger than Ruby's Lich pool by a wide margin because of Roblox and game modding.

## What a script looks like

```lua
local s = session()          -- the owning character
s.watch("^You feel drained", function(m)
  s.fput("stance defensive")
end)

while true do
  s.waitrt()
  if s.health.percent < 50 then
    s.cast(101)
  end
  local line = s.waitfor("^A (.+) arrives")
  s.fput("attack " .. line.captures[1])
  s.sleep(1)
end
```

Followers in a multi-session hunt read `sessions()` and call `session("Bob").send(...)`, which lands in Bob's command queue and passes through Bob's hooks, so Bob's own scripts see it.

## Runner-up

TypeScript on QuickJS, if you decide author growth matters more than raw mobile performance. It's a fine engine and the tooling is better, but it costs you coroutines and roughly half the interpreter speed. For the requirements you just listed, Luau.

---

## USER  (2026-09-18T03:07:08)

ok I can get behind lua. Now next question. Can you come up with a plan? It needs to obey the rules that vellum embodies. Separation of responsibility. So we need to do a proper inventory of lich a proper inventory of vellum and come up with a proper plan for building something new that incorporates the two. We can call it Cena. I want to do this properly though. So launch some agents to gather all the reports and data. Save everything here "E:\Cena".  Mobile is a driving factor in our choices but remember we are a desktop app as well and probably primarily haha. So we want to keep in mind all the features both offer.

---

## CLAUDE  (2026-09-18T03:07:19)

Setting up E:\Cena with reference clones, then fanning out inventory agents.

---

## CLAUDE  (2026-09-18T03:10:10)

Seven inventory agents are running. Meanwhile, you've created the repo. Let me check it and wire E:\Cena to it (no pushes).

---

## CLAUDE  (2026-09-18T03:10:16)

The remote is empty, so I'll initialize E:\Cena as the working tree pointed at it, and exclude the reference clones from tracking.

---

## CLAUDE  (2026-09-18T03:10:37)

I've set up the workspace and saved project context. Now waiting on all seven inventory agents before writing the plan, since the plan has to be grounded in their output.

---

