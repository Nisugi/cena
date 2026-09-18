# Lich-5 as Arbiter: The Direction, and How Far It Has Actually Gone

**Date:** 2026-09-17
**Snapshot audited:** `E:/Cena/reference/lich-5` at commit `236a9a2c6974339927d68c7bd07895b3f785d8e5`
("fix(all): Initial Lich state update and detachable-client split GS/DR (#1617)", Wed Sep 16 2026).
**Consumer audited:** `C:/Gemstone/eohunter` (43,156 Ruby LOC repo-wide; 27,068 LOC under `scripts/`).

## Scope

This document tests one thesis: that recent Lich-5 development has been moving away from
"a pile of blocking global functions" and toward **Lich as an arbiter for scripts** — a
supported public API over sealed internals, typed event subscriptions instead of text
scraping, mediated command execution, and cross-session coordination. It is written as an
input to Cena's Rust-core / Lua-API boundary design.

Files read in full or in the cited ranges:

| Area | Files |
|---|---|
| API facade | `lib/api/active_sessions.rb` (44) |
| Active sessions service | `lib/internal_api/active_sessions.rb` (645), `active_sessions/registry.rb` (178), `server.rb` (286), `client.rb` (127), `lifecycle.rb` (363) — 1,643 total |
| Combat pipeline | `lib/gemstone/combat/tracker.rb` (612), `processor.rb` (2,436), `parser.rb` (383), `async_processor.rb` (115), `messages.rb` (180), `recorder.rb` (1,061), `defs/*` (4,673) — 9,460 total |
| Event bus and hooks | `lib/common/events.rb` (278), `lib/common/hook_registry.rb` (189), `lib/common/downstreamhook.rb` (62), `lib/common/upstreamhook.rb` (62) |
| Mediated commands | `lib/util/util.rb` (359), `lib/global_defs.rb` lines 1479–1650 (`fput`), 312–360 (`waitrt?`), 1949 (`dothistimeout`) |
| Landed helper PRs | `lib/gemstone/stance.rb`, `lib/stash.rb`, `lib/gemstone/mana.rb`, `lib/gemstone/bank.rb`, `lib/gemstone/fog.rb`, `lib/gemstone/injured.rb`, `lib/gemstone/society.rb`, `lib/gemstone/psms.rb`, `lib/gemstone/group.rb`, `lib/common/spell.rb` |
| Consumer view | `C:/Gemstone/eohunter/docs/core-consumption-audit.md`, `docs/roadmap.md`, `docs/guides/core-dependencies.md`, `docs/hunting-engine-plan.md`, `scripts/eohunter/actions.rb`, `watch.rb`, `world.rb`, `cleanse.rb`, `routines.rb`, `combat.rb`, `maintain.rb` |

`lib/` as a whole is 106,642 Ruby LOC across 955 files (verified 2026-09-17). The arbiter
surface described here is roughly 11,500 of those — about 11%. That ratio is itself a
finding: the direction is real but it is a minority of the codebase.

**Caveat on the denominator (added after verification):** 106,642 is correct, but it is
dominated by generated lookup tables — `lib/gemstone/critranks/` alone is 46,163 LOC and
`lib/gemstone/creatures/` is 7,744 across 628 files. Excluding those two data trees, `lib/`
is ~52,700 LOC of actual code and the arbiter surface is closer to **22%** of it. Both
framings are given because the data/code split matters more for Cena than either ratio:
the crit and creature tables are static data that can ship as data files rather than be
rewritten in Rust.

**Headline verdict, stated up front so the rest can be read honestly:**
the *event pipeline* (section 3) and *mediated command execution* (section 4) are real,
shipped, and consumed in production by a large third-party script. The *public-API facade*
(section 1) is real code with **zero callers in the tree** — it is a declared intention, not
a load-bearing boundary. The *active-sessions service* (section 2) is fully built and
fully tested but **dormant behind a feature flag that is off by default**.

---

## 1. The API / InternalAPI split

### 1.1 What exists

There is exactly one file under `lib/api/`: `active_sessions.rb`, 44 lines. There is one
module tree under `lib/internal_api/`: `active_sessions.rb` (645) plus four files in
`active_sessions/` (954). That is the entire split.

The facade's own header states the rule, and it is explicit about *read-only* being the
point (`lib/api/active_sessions.rb:3-9`):

```ruby
  # Small public facade for the active sessions runtime service.
  #
  # This facade intentionally exposes only read operations. Internal lifecycle
  # registration, transport startup, and server ownership stay inside
  # `Lich::InternalAPI::ActiveSessions`.
  module API
```

Three public methods, all reads (`lib/api/active_sessions.rb:13`, `:27`, `:34`):

| Method | Returns | Delegates to |
|---|---|---|
| `Lich::API.active_session_snapshot` | normalized snapshot Hash | `InternalAPI::ActiveSessions.snapshot` (`:22`) |
| `Lich::API.active_sessions` | the `:sessions` Array | itself, via the snapshot (`:29`) |
| `Lich::API.active_session_service_info` | sanitized service metadata | `InternalAPI::ActiveSessions.service_info` (`:41`) |

Every one of them returns an **inert fallback** rather than raising when the internal
module is absent (`lib/api/active_sessions.rb:14-20`):

```ruby
      return {
        source: 'ActiveSessionsAPI',
        total: 0,
        connected: 0,
        detachable: 0,
        sessions: []
      } unless defined?(Lich::InternalAPI::ActiveSessions)
```

### 1.2 What is sealed, and how

Sealing is done with Ruby's `private_class_method`, applied aggressively inside
`InternalAPI::ActiveSessions`. The sealed set:

| Sealed method | Line | Why it is sealed |
|---|---|---|
| `ensure_service_internal!` | `internal_api/active_sessions.rb:127` (sealed at `:154`) | bypasses the feature-gate re-read on hot paths |
| `register_session_admitted` | `:177` (sealed at `:182`) | feature-gate bypass for lifecycle-owned callers |
| `unregister_session_admitted` | `:200` (sealed at `:205`) | same |
| `service_client` | `:289` | holds the shared auth token |
| `fallback_snapshot` | `:309` | shape contract |
| `service_available?`, `owns_running_server?` | `:325`, `:336` | ownership internals |
| `release_in_process_zombie!`, `claim_ownership_and_start!` | `:347`, `:373` | server ownership |
| `own_lock?`, `acquire_ownership_lock`, `release_ownership_lock` | `:411`, `:424`, `:454` | the cross-process lock |
| `coordination_dir`, `lock_path`, `discovery_path` | `:476`, `:486`, `:494` | on-disk layout |
| `load_discovery`, `write_discovery` | `:502`, `:519` | discovery record incl. the token |
| `cleanup_discovery_temp_file`, `retry_discovery_filesystem_operation`, `retry_windows_sharing_violation` | `:542`, `:562`, `:578` | filesystem retry policy |
| `delete_discovery_if_owned`, `delete_discovery_if_owner` | `:596`, `:608` | ownership-gated unlink |

The token-hiding is stated as a deliberate contract, not an accident
(`lib/internal_api/active_sessions.rb:241-244`):

```ruby
      # Returns sanitized metadata about the current active-sessions service.
      #
      # The public shape intentionally omits the shared auth token while still
      # exposing enough information for diagnostics and operator visibility.
```

And the reason the *admitted* variants are sealed is spelled out at the one call site
that uses them, in `Lifecycle` (`lib/internal_api/active_sessions/lifecycle.rb:280-282`):

```ruby
            # ActiveSessions keeps the admitted-path helpers private so the
            # feature-gate bypass stays local to lifecycle-owned call sites.
            ActiveSessions.send(:register_session_admitted, payload)
```

That `send` is worth naming plainly: the seal is by convention, and the one privileged
consumer defeats it with `send`. In Ruby this is the only tool available. **In Rust it
would be a `pub(crate)` function and the seal would be real.** That is a direct argument
for Cena.

### 1.3 The honest finding: the facade has no callers

`Lich::API.` appears **nowhere** in the tree outside its own definition and its spec. A
grep for the string `api/active_sessions` across the whole repository returns only
`.git/index` and four lines of `.github/workflows/windows_active_sessions.yaml` — which
reference `lib/internal_api/active_sessions*`, not the facade. `lich.rbw:84` requires
`internal_api/active_sessions.rb`; **nothing requires `lib/api/active_sessions.rb`**.

Every real consumer in the tree reaches past the facade straight into `InternalAPI`:

| Consumer | Line | Call |
|---|---|---|
| `lib/common/cli/active_sessions_query.rb` | `:75` | `Lich::InternalAPI::ActiveSessions.query_snapshot` |
| `lib/main/main.rb` | `:887`, `:891`, `:895` | `...::Lifecycle.resolve_session_name` / `.resolve_role` / `.start` |
| `lib/main/main.rb` | `:946`, `:954`, `:1076`, `:1123`, `:1152` | `Lifecycle.clear_listener` / `.update_connected` / `.stop` |
| `lib/global_defs.rb` | `:1660`, `:2084` | `Lifecycle` passed to `OrderlyShutdown`, `Lifecycle.update_listener` |

**Verdict on section 1: the API/InternalAPI split is a stated architectural intention
backed by 44 lines of unreferenced code.** The *sealing* is real and enforced in the
internal module. The *facade* is aspirational. For Cena this is the most useful kind of
finding: Lich's authors have written down exactly what boundary they want, and have not
yet been able to make anything depend on it. Cena can start where Lich wants to end up,
because in Rust the boundary is a compiler-enforced module visibility rather than a
naming convention.

---

## 2. The active-sessions service: Lich coordinating separate OS processes

1,643 lines across five files. This is the part of Lich that most directly anticipates
Cena's multi-session goal — and the part whose design must be most carefully filtered,
because **every line of the transport exists only because each Lich session is its own
OS process.**

### 2.1 The Registry: authoritative in-memory model

`registry.rb` (178 lines) is the state. Its header states the modelling rule
(`lib/internal_api/active_sessions/registry.rb:8-12`):

```ruby
      # In-memory authoritative store of active Lich sessions.
      #
      # The registry intentionally models only live runtime state. It is not a
      # historical store and removes sessions when they can no longer be
      # confirmed as resident in the OS process table.
```

Keyed by pid. `upsert` (`:28`) merges, stamps `started_at` once and `last_seen_at` every
time, and coerces `connected`/`hidden` to booleans (`:40-41`). `snapshot` (`:70`) sweeps
dead pids first, then derives `uptime_seconds` and a `listener` sub-hash, and returns
counts alongside the list (`:81-87`):

```ruby
          {
            source: 'ActiveSessionsAPI',
            total: normalized.length,
            connected: normalized.count { |session| session[:connected] },
            detachable: normalized.count { |session| session[:listener] },
            sessions: normalized
          }
```

**Liveness is pid-based, via signal 0** (`:121-130`):

```ruby
        def self.process_alive?(pid)
          Process.kill(0, pid.to_i)
          true
        rescue Errno::ESRCH
          false
        rescue Errno::EPERM
          true
        rescue StandardError
          false
        end
```

`Errno::EPERM` → alive is the correct call (the process exists, we just cannot signal it),
and `sweep_dead_sessions!` (`:93`) runs on every snapshot, so a crashed session disappears
from the model without any cooperation from the crashed process.

The registry is guarded by one `Mutex` (`:21`) and every read hands out `.dup` copies
(`:63`, `:73`) — callers cannot mutate registry state through a returned record.

### 2.2 The transport: local TCP, ephemeral port, discovery file, shared secret

`server.rb` (286 lines) states why TCP and not a UNIX socket
(`lib/internal_api/active_sessions/server.rb:13-16`):

```ruby
      # The transport is intentionally local-only TCP to keep behavior consistent
      # across Linux, macOS, and Windows. The server delegates all state changes
      # to {Registry}; it does not own lifecycle policy beyond request routing
      # and thread cleanup.
```

Ownership is elected by an **advisory file lock**, not a well-known port. The reasoning
(`lib/internal_api/active_sessions.rb:22-30`):

```ruby
    # Ownership is coordinated with a cross-process advisory file lock
    # (see {acquire_ownership_lock}) rather than a fixed, well-known TCP port.
    # The single process that holds the lock binds an ephemeral port and
    # publishes it in the discovery file; peers read that port to reach the
    # owner. Because the kernel releases an advisory lock the instant its owning
    # process dies, a crashed owner never blocks a successor, and there is no
    # fixed port for a stuck process to squat.
```

The mechanism, end to end:

| Step | Where | What |
|---|---|---|
| Acquire lock | `internal_api/active_sessions.rb:430-432` | `File.open(lock_path, RDWR\|CREAT, 0o600)` then `flock(LOCK_EX \| LOCK_NB)` |
| Bind | `server.rb:58-60` | `TCPServer.new('127.0.0.1', 0)`; real port read back from `@server.addr[1]` |
| Mint token | `internal_api/active_sessions.rb:381` | `auth_token: SecureRandom.hex(32)` |
| Publish | `:519-532` | JSON `{owner_pid, auth_token, port, updated_at}` written `0o600` to a temp file, then `File.rename` (atomic) |
| Discover | `:502` | peers `JSON.parse(File.read(discovery_path))` |
| Authenticate | `server.rb:247-249` | `request[:auth].to_s == @auth_token` |

Protocol: one JSON object per line, one request per connection, four commands
(`server.rb:228-242`): `ping`, `upsert`, `remove`, `snapshot`. Responses are always
`{ok: true, payload: …}` or `{ok: false, error: …}`. Both ends read with a deadline-driven
non-blocking loop (`server.rb:193-217`, `client.rb:99-123`) with `READ_TIMEOUT = 1` second
(`server.rb:22`, `client.rb:25`) and `CONNECT_TIMEOUT = 1` (`client.rb:19`).

The client (127 lines) is deliberately dumb (`client.rb:9-14`):

```ruby
      # Thin JSON client for the local active sessions service.
      #
      # The client intentionally knows nothing about lifecycle semantics. It is
      # only responsible for packaging JSON commands, reading JSON responses,
      # and normalizing failures into a predictable `{ ok: false, error: ... }`
      # shape for higher-level callers.
```

Windows gets its own bounded retry ladder for sharing violations,
`DISCOVERY_FILESYSTEM_RETRY_DELAYS = [0.01, 0.02, 0.04, 0.08, 0.16]`
(`internal_api/active_sessions.rb:71`), applied to rename and unlink
(`:562`, `:578`) while the flock is held throughout.

### 2.3 The lifecycle and its contract

`lifecycle.rb` (363 lines) is the only writer. Its header is the clearest statement of
"what one session may claim about itself" in the codebase
(`lib/internal_api/active_sessions/lifecycle.rb:13-29`):

```ruby
      # ActiveSessions API contract:
      #
      # * A session record being present in the registry means the Lich process
      #   is still known to the active-sessions service.  Presence is not a
      #   promise that the game connection is still usable.
      # * The `connected` field is the authoritative connection signal exposed
      #   to API consumers.  Shutdown code must set it to `false` when the game
      #   connection has ended, before slower teardown work such as script
      #   before_dying hooks, state persistence, socket closeout, and database
      #   closeout runs.
      # * Lifecycle `stop` unregisters the process from ActiveSessions.  That
      #   removal means the process is no longer reporting as an active session;
      #   it should not be used merely to mean "the game connection dropped."
      # * For detachable sessions, the public `connected` value is true only
      #   when both the game connection and detachable listener connection are
      #   active.
```

That last rule is enforced in one expression (`lifecycle.rb:333`):

```ruby
            connected: @connected && (@listener_port.nil? || @listener_connected),
```

Heartbeat cadence is 2 seconds (`:35`), chosen explicitly for failover detection:
"detecting service-owner failover quickly enough for multi-session use" (`:32-33`).
The heartbeat thread is respawn-tolerant and swallows per-tick errors (`:114-118`).

A `@lifecycle_generation` counter (`:49`, bumped at `:106` and `:157`) plus
`registration_current?` (`:317`) prevent a stale in-flight upsert from a previous
start/stop cycle landing in the registry after a restart.

`update_connected` carries a rationale that reads like a postmortem
(`lifecycle.rb:224-229`):

```ruby
        # This method exists because MahtraDR's shutdown testing demonstrated
        # that immediate unregister is too blunt an API signal: it hides a
        # still-running Lich process from ActiveSessions while scripts, saves,
        # socket closeout, and database closeout may still be executing.  The
        # narrower contract is to keep the session present while publishing
        # `connected: false`.
```

The published payload — the complete set of what one session tells the others
(`lifecycle.rb:326-337`):

```ruby
          {
            pid: Process.pid,
            session_name: @session_name,
            role: @role,
            frontend: resolve_frontend,
            game_code: resolve_game_code,
            started_at: @started_at,
            connected: @connected && (@listener_port.nil? || @listener_connected),
            listener_host: @listener_host,
            listener_port: @listener_port,
            hidden: false
          }
```

`role` is one of `'headless'`, `'detachable'`, `'session'` (`:74-79`).

### 2.4 Dormant by default

`FEATURE_FLAG = :active_sessions_api` (`internal_api/active_sessions.rb:40`). The comment
is candid (`:90-93`):

```ruby
      # Until feature-flag plumbing is present in core, this API otherwise
      # remains safely dormant and returns empty snapshots.
```

`enabled?` (`:95`) returns true only if `--active-session-dir=PATH` was passed or the
persisted flag is set; the default is false. So: 1,643 lines, a dedicated CI workflow
(`.github/workflows/windows_active_sessions.yaml`), seven spec files — and off unless
opted in. **This is built infrastructure awaiting a decision, not shipped behaviour.**

### 2.5 For Cena: what is transport accident and what is a real rule

Cena is one binary. Sessions are in-process, so the registry stops being an RPC server and
becomes shared state — probably an `Arc<RwLock<SessionRegistry>>` or a small actor owning
an `IndexMap<SessionId, SessionRecord>`. Sorting the design:

**Drop — pure transport accident:**

| Thing | Lines | Why it goes |
|---|---|---|
| TCP server + accept loop + per-client threads | `server.rb` all 286 | in-process channel or direct lock |
| JSON framing, newline protocol, read deadlines | `server.rb:193-217`, `client.rb:99-123` | no serialization across a process boundary |
| Shared-secret auth token, `SecureRandom.hex(32)` | `:381`, `server.rb:247` | nothing untrusted can connect to an in-process registry |
| Discovery file, atomic rename, `0o600` perms | `:519-532` | no discovery needed |
| Advisory `flock` ownership election | `:424-452` | there is one owner by construction |
| `owns_running_server?`, zombie rebind | `:336`, `:347` | no server to zombie |
| Windows sharing-violation retry ladder | `:71`, `:562-594` | no shared file |
| `at_exit { stop_service! }` | `:642` | no cross-process cleanup |
| pid liveness via `Process.kill(0, pid)` | `registry.rb:121` | in-process tasks are owned; a dropped handle *is* the death signal |

That is roughly 900 of the 1,643 lines. **Do not port them.**

**Keep and enforce in Rust — these are real rules, not accidents:**

| Rule | Lich source | Why it survives the transport change |
|---|---|---|
| **A session's view of another session is a snapshot, not a handle.** Every read hands out `.dup` copies. | `registry.rb:63`, `:73` | This is the whole separation-of-responsibility argument. In Rust: return an owned `SessionSnapshot` value, never `&Session` or an `Arc<Session>` interior. |
| **The cross-session surface is an explicitly enumerated payload, not the session object.** Ten fields, no more. | `lifecycle.rb:326-337` | A Lua script in session A must never be able to reach session B's script table, buffer, or socket. Make the snapshot a distinct type from the live session. |
| **The cross-session API is read-only for consumers; only a session's own lifecycle writes its own record.** Facade exposes reads (`api/active_sessions.rb:3-5`); writes are sealed to `Lifecycle` (`:182`, `:205`). | `api/active_sessions.rb:3-9` | In Rust: `SessionRegistry::snapshot()` is `pub`; `upsert`/`remove` are `pub(crate)` and take a capability token proving the caller *is* that session. |
| **Presence ≠ usability. `connected` is a separate, authoritative field.** | `lifecycle.rb:15-20` | A Cena session that has lost its game socket but is still draining scripts must be visible-but-not-connected, or coordinating scripts will make bad decisions during teardown. This bug was found the hard way (`:224-229`) — inherit the fix, not the bug. |
| **A composite health signal, computed centrally, not by each reader.** `connected && (listener_port.nil? \|\| listener_connected)` | `lifecycle.rb:333` | Cena has the same shape (game socket vs. frontend attachment). Compute once; never make a Lua script AND two booleans together. |
| **Derived read-model fields (`uptime_seconds`, counts) are computed at snapshot time.** | `registry.rb:74-87` | Keeps the stored model minimal and the read model convenient. |
| **A generation counter invalidates in-flight writes across a restart.** | `lifecycle.rb:49`, `:317` | Cena's session restart / reconnect has exactly this race. |
| **Inert fallback, never raise, when the service is unavailable.** | `api/active_sessions.rb:14-20`, `internal_api/…:309` | A Lua script asking "what other sessions exist?" before the registry is up should get an empty list, not an error. |
| **A kill switch.** `FEATURE_FLAG`, off by default. | `:40`, `:95` | Cross-session coordination is the feature most likely to produce emergent misbehaviour across three characters. Ship it gated. |

**The one thing Cena gains for free that Lich cannot have:** because sessions are in-process,
Cena does not need the heartbeat at all. Lich heartbeats every 2 seconds
(`lifecycle.rb:35`) purely to detect that the *owner process* died. Cena's registry learns
of a session's death when its task handle drops. Delete the heartbeat; keep
`last_seen_at` only if something genuinely wants staleness, which for in-process sessions
it probably does not.

**The one thing Cena must add that Lich does not have:** Lich's registry is
observation-only — there is no "send session B a command" verb in the protocol
(`server.rb:228-242` has exactly `ping`/`upsert`/`remove`/`snapshot`). eohunter, wanting
actual cross-session coordination, had to build its own DRb layer instead
(`group.rb`, 1,531 lines; the audit at `core-consumption-audit.md` section 6 says plainly
"Lich has `Group` for the game's roster and no cross-process layer"). Cena should decide
deliberately whether cross-session *messaging* exists, and if it does, make it an explicit
mailbox/command verb with the same read-only-snapshot discipline around state — not a
back door into another session's interior.

---

## 3. The Combat event pipeline: the strongest evidence for the thesis

9,460 lines. This is the flagship of the arbiter direction and, unlike section 1 and
section 2, it is **shipped, on, and consumed by a real third-party script today**.

### 3.1 The general mechanism underneath it: `Lich::Common::Events`

`lib/common/events.rb`, 278 lines, is a process-wide topic bus. Its header explains the
consolidation (`lib/common/events.rb:20-22`):

```ruby
    # Design (extracted from the former Combat::Observers and from HookRegistry,
    # the two places this was previously re-solved):
```

Contract, from the header (`:23-46`):

| Rule | Line |
|---|---|
| Topics are dotted strings; `'combat.*'` matches `'combat.damage'` but not `'combat'`; `'*'` matches everything | `:23-27` |
| Named registration is **idempotent** — re-subscribing under a name replaces, never stacks | `:28-31` |
| Every subscription records `Script.current` as owner; on script death, `persist: false` (the default) subscriptions are swept | `:32-36` |
| Delivery is **synchronous on the emitter's thread**; handlers must be cheap, non-blocking, and must never send game commands | `:37-41` |
| A raising handler is isolated and logged; it never breaks other handlers or the emitter | `:40-41` |
| Deliberately absent: async delivery, queues, history, persistence | `:42-44` |

And the state-vs-events doctrine, which is a direct design instruction for Cena
(`:47-51`):

```ruby
    # State versus events: keep a read model (Go2.status, the Creature
    # registry) for "what is true now" and poll it. Emit events for the edges
    # that polling cannot see - transitions, transients, the moment something
    # changed. Payloads should be the read model's own object or a small Hash.
```

`emit` snapshots its handler list under the mutex before calling any of them
(`:127`), so a handler that unsubscribes mid-emit can still fire once — documented
honestly at `:99-103` ("Not synchronous with in-flight delivery").

There is also an `on_change` notification (`events.rb:158`, used at `messages.rb:176`) so a
subsystem can install or tear down its game-stream hook the moment subscriptions appear
or vanish. That is a genuinely good idea: **zero cost when nobody is listening.**

### 3.2 Flow: raw text → typed events

```
game socket
  → DownstreamHook.run(server_string)             common/downstreamhook.rb:39
      ├─ 'Combat::Tracker::downstream'  (buffering hook)   tracker.rb:517-564
      │     buffer lines; on '<prompt time=' cut a chunk;
      │     discard the chunk unless it names a creature    tracker.rb:533-545
      │       → Tracker.process(chunk, source:)             tracker.rb:299
      │           → AsyncProcessor#process_async (Queue)    async_processor.rb:40
      │               → single worker thread
      │                   → Processor.process(chunk)        async_processor.rb:97
      │                       → Parser.parse_* per line     parser.rb
      │                       → Events.emit('combat.<type>') processor.rb:190/1841/1894/…
      │                       → Combat::Recorder            recorder.rb:460
      └─ 'Combat::Messages::downstream'  (line hook)        messages.rb:131-140
            enqueue(line) → its own Queue → its own worker  messages.rb:105-111
                → Definitions::Messages.scan(line)          messages.rb:89
                → Events.emit("combat.<event>")             messages.rb:99
```

**Two separate hooks, for a stated reason** (`messages.rb:7-15`):

```ruby
# The Tracker's hook chunks on the prompt and only hands a chunk to the
# Processor when it names a creature; most of these lines arrive in
# chunks that name none (an itchy curse, an item limit, a bolt). So the
# messages have their own hook, one line at a time, installed the moment
# the first subscription to a message event appears and removed with the
# last.
```

### 3.3 How it is fed: yes, `DownstreamHook`

Both entry points are anonymous-ish named `DownstreamHook` registrations with
`persist: true`:

- Tracker: `DownstreamHook.add(@hook_id, segment_buffer, persist: true)` where
  `@hook_id = 'Combat::Tracker::downstream'` (`tracker.rb:518`, `:563`).
- Messages: `::DownstreamHook.add(HOOK_ID, hook, persist: true)` where
  `HOOK_ID = 'Combat::Messages::downstream'` (`messages.rb:36`, `:139`).

The Tracker's buffering hook, verbatim on the chunking decision (`tracker.rb:532-545`):

```ruby
              # Process on prompt (natural break in game flow)
              if server_string.include?('<prompt time=')
                chunk = @buffer.slice!(0, @buffer.size)
                source = @buffer_source_invalid ? nil : @buffer_source
                @buffer_source, @buffer_source_invalid = nil, false
                if chunk.any? { |line|
                  (line.include?('<pushBold/>') && line.include?('<a exist=')) ||
                  Definitions::Attacks.attackerless_line?(line)
                }
                  process(chunk, source: source) unless chunk.empty?
```

Note the hook is a **pass-through filter**: it returns `server_string` unmodified
(`tracker.rb:552`). It observes; it does not rewrite. That is the right shape.

### 3.4 Threading and buffering

| Property | Value | Source |
|---|---|---|
| Enqueue is O(1), never blocks the game stream | `@queue.push([chunk, source])` | `async_processor.rb:48` |
| Exactly **one** consumer thread, deliberately | "processing is intentionally single-threaded to preserve event ordering" | `async_processor.rb:19-21` |
| Reason for single-threading | "creature instances are only ever mutated from one thread - no synchronization needed" | `async_processor.rb:8-10` |
| Worker respawns if killed by script death | `ensure_worker` on every `process_async` | `async_processor.rb:31-35`, `:77-86` |
| Messages has its own identical-shaped worker | "One ordered worker, respawned if a script's death took it (the same shape as AsyncProcessor)" | `messages.rb:148-150` |
| Buffer cap | `buffer_size: 200`, oldest dropped and source marked invalid | `tracker.rb:67`, `:548-551` |
| `max_threads` is vestigial | "retained for call-site compatibility" | `async_processor.rb:19` |

`DEFAULT_SETTINGS` (`tracker.rb:59-70`), persisted per-character to `DB_Store`:

| Setting | Default | Meaning |
|---|---|---|
| `enabled` | `false` | off by default; user must `enable!` |
| `track_damage` / `track_wounds` / `track_statuses` / `track_ucs` | `true` | per-family toggles |
| `emit_attacks` | `false` | emit the whole `:attack` blob for recorder-class subscribers |
| `max_threads` | `2` | vestigial; `0` forces inline parsing |
| `debug` | `false` | `:summary` or `:verbose` |
| `buffer_size` | `200` | lines held before a prompt cut |
| `fallback_max_hp` | `350` | when no creature template exists |

Debug mode forces `max_threads: 0` so output orders correctly against the game text
(`tracker.rb:245-255`), stashing the previous value in an **ivar not a setting** —
with the reason given (`tracker.rb:46-51`): persisting `max_threads: 0` to disk would
"leave the character parsing inline forever if debug were never cleanly disabled."

There is also a real ordering bug-fix baked in that Cena should copy. Parse-phase facts
are **deferred and flushed after** the chunk's attack events (`processor.rb:175-202`):

```ruby
        # Message statuses ("You blinded X!"), UCS facts and spell losses are
        # recognised while a chunk PARSES, but the chunk's :attack emits only
        # happen afterwards in persist_event. Emitting them immediately put
        # every such fact in front of the attack it belongs to, so a recorder
        # keying on "the attack currently open" filed it under the PREVIOUS
        # attack (real-feed 2026-09-07: every blind stamped 1-3s before its
        # attack).
```

### 3.5 The subscription API

`Combat::Tracker.on` is a thin, typed sugar over `Events.on` (`tracker.rb:142-147`):

```ruby
          def on(*types, name: nil, persist: false, &block)
            types = [:any] if types.empty?
            topics = types.map { |t| t.to_sym == :any ? 'combat.*' : "combat.#{t}" }
            Lich::Common::Events.on(*topics, name: name, persist: persist, &block)
          end
```

Usage, from the doc block (`tracker.rb:138-140`):

```ruby
          #   Combat::Tracker.on(:damage) { |topic, data| queue << data }
          #   Combat::Tracker.on(:damage, name: 'mybar') { ... } # idempotent
```

The subscriber contract is stated as a contract (`tracker.rb:89-95`):

```ruby
          # Contract for subscribers:
          #   - Callbacks may run on AsyncProcessor worker threads. They must be
          #     cheap and non-blocking, and must NEVER send game commands (fput /
          #     Spell#cast / PSMS.use) - queue work for your own script thread.
          #   - A raising subscriber is isolated and logged; it never breaks other
          #     subscribers or the processor.
```

Message events need neither `enable!` nor creature scanning: "their hook goes up with the
first subscription" (`tracker.rb:85-87`).

### 3.6 Event catalog

**Combat facts** — every payload carries `:id` and `:name` of the creature
(`tracker.rb:97-125`, verified against the emit sites):

| Event (topic `combat.<type>`) | Payload keys | Emit site |
|---|---|---|
| `:damage` | `id, name, attack, amount` | `processor.rb:1894`, `:1921` |
| `:wound` | `id, name, attack, location, body_part, rank` | `processor.rb:2122`, `:2197` |
| `:fatal_crit` | `id, name, attack, location` | `processor.rb:2154` |
| `:amputation` | `id, name, …` | `processor.rb:2144` |
| `:status` | `id, name, status, action: :add \| :remove` | `processor.rb:1945`, `:2064`, `:2287`, `:2296` |
| `:stun` | `id, name, …` | `processor.rb:2258` |
| `:roundtime` | `id, name, …` | `processor.rb:2267` |
| `:ucs` | `id, name, kind: :position\|:position_inbound\|:tierup\|:smite_on\|:smite_off, value, tier` (`tier` 1..3 or nil) | via `emit_fact`, `processor.rb:190` |
| `:spell_loss` | `id, name, spell, spell_name, cause: :dispel\|:death\|nil` — subject may be a **player** (negative id) | `tracker.rb:111-118` |
| `:attack` | the whole parsed attack blob; only when `emit_attacks: true` | `processor.rb:1841` |
| `:recorded_attack` | `protocol, recorder_id, database, file_identity, session_id, attack_id, source` — emitted **only after the attack transaction commits** | `recorder.rb:460` |

`:recorded_attack` deserves a note for Cena: it is a *post-commit receipt* carrying opaque
IDs plus trusted local database identity, so an observer can read exactly that row
(`tracker.rb:119-125`). That is a mature pattern — an event that tells you where the
durable record is rather than duplicating it in the payload.

**Message events** — from `defs/messages.rb:51-71`, which is a machine-checkable payload
contract table (`PAYLOAD_KEYS`, with a `family` and typed `keys` per event). Every payload
also carries `:raw`, the line (`defs/messages.rb:17-18`, `defs/messages.rb:231`).

| Family | Events | Payload keys |
|---|---|---|
| `:disarm` | `:disarm_seen` | `kind: :symbol, noun: :string` |
| | `:sanctum_transform` | `noun: :string` |
| `:hazard` | `:itchy_curse`, `:infected_wound`, `:entangled` | `{}` |
| | `:hive_trap` | `kind: :symbol` (`:apparatus` / `:ground`) |
| `:ambush` | `:ambusher` | `noun: :string` (nil for the shadowy figure) |
| | `:bolted` | `{}` |
| `:hold` | `:rooted` | `{}` |
| | `:unrooted` | `id: :string` |
| | `:item_limit` | `{}` |
| `:bless` | `:bless_shrugged`, `:bless_expired` | `id: :string, noun: :string` |
| `:archery` | `:arrow_stuck` | `id: :string, where: :string` |
| | `:aiming` | `where: :string` (nil when cleared) |
| | `:bond_return` | `what: :string` |
| `:marks` | `:haze_703`, `:rebuke_1614` | `id: :string, on: :boolean` |
| | `:swift_justice` | `charges: :integer` |
| | `:arcane_reflex` | `active: :boolean` |
| `:reaction` | `:weapon_reaction` | `reaction: :string` |

Twenty message events across eight families, each with a declared key type. That typed
table exists so a **user supplement cannot move an event to another family or change its
payload shape** (`defs/messages.rb:43-49`) — a schema, enforced at load.

### 3.7 The definition tables

`defs/` is 4,673 lines across twelve Ruby files plus a 167-line example YAML:

| File | LOC | Holds |
|---|---|---|
| `attacks.rb` | 837 | attack initiation defs (incl. `ATTACKERLESS`, `ROOM_TARGETED`) |
| `supplements.rb` | 822 | the user-supplement loader and validator |
| `statuses.rb` | 518 | status add/remove |
| `outcomes.rb` | 482 | hit/miss/crit outcomes |
| `flares.rb` | 449 | weapon and spell flares |
| `spells.rb` | 369 | spell attack defs |
| `messages.rb` | 240 | the eight non-combat message families |
| `pattern_gate.rb` | 203 | the per-family literal prefilter |
| `spell_losses.rb` | 140 | spells wearing off |
| `ucs.rb` | 128 | unarmed combat facts |
| `damage.rb` | 121 | damage amounts |
| `assaults.rb` | 120 | assault forms |
| `sequences.rb` | 77 | multi-line sequences |

Each family self-gates with a small literal union rather than one giant regex, and the
reason is *measured*, not assumed (`parser.rb:34-40`):

```ruby
        # NOTE ON SCALE: each def family gates itself with a small literal
        # union (PatternGate). A single combined prefilter union was tried
        # and measured SLOWER (58us vs 35us/line on real logs) - Ruby's
        # regex engine scans large alternations linearly, so one big union
        # costs more than several small ones. When def counts grow into the
        # hundreds-per-family, the proven fix is a first-word bucket index
        # (see CritRanks - 2,389 patterns, indexed by first literal word),
```

**For Cena this is the single most transferable section.** The architecture — chunk on
prompt, gate cheaply, parse on a worker, emit typed events with declared payload schemas,
never let a subscriber block the stream — maps almost directly onto a Rust design, and
the Rust version gets to make the payload schema a real `enum` with real types instead of
a Hash and a hand-maintained key table. The measured "many small unions beat one big one"
finding is a Ruby-regex artifact; Rust's `regex` crate with a `RegexSet` or Aho-Corasick
prefilter inverts that conclusion, so **do not port the gating strategy, port the gating
discipline.**

---

## 4. Mediated command execution

### 4.1 The problem it solves

Every script shares one game socket and one downstream text stream. Two scripts that each
send `look #12345` and then read lines until they see something they recognise will steal
each other's replies. `Lich::Util.issue_command` is the arbitration: it makes a
request/response round-trip an *atomic, scoped* operation.

### 4.2 `issue_command`

`lib/util/util.rb:156`. Signature (`:156`):

```ruby
    def self.issue_command(command, start_pattern, end_pattern = /<prompt/, include_end: true,
                           timeout: 5, silent: nil, usexml: true, quiet: false, use_fput: true)
```

What it mediates, in order:

**1. Save/restore of the calling script's three stream flags** (`:162-164`, restored at
`:219-221`):

```ruby
      save_script_silent = Script.current.silent
      save_want_downstream = Script.current.want_downstream
      save_want_downstream_xml = Script.current.want_downstream_xml

      Script.current.silent = silent if !silent.nil?
      Script.current.want_downstream = !usexml
      Script.current.want_downstream_xml = usexml
```

`want_downstream` and `want_downstream_xml` are **mutually exclusive** here: the method
forces the script's buffer into exactly the mode the caller's patterns were written for,
then puts it back. A script that wanted plain text does not silently start receiving XML
because a helper it called needed XML.

**2. An anonymous, script-scoped `DownstreamHook`.** The name is generated
(`util.rb:74-77`):

```ruby
    def self.anon_hook(prefix = '')
      now = Time.now
      "Util::#{prefix}-#{now}-#{Random.rand(10000)}"
    end
```

and registered with `persist: false` and the explicit comment "scoped to this command;
removed in the ensure below" (`util.rb:199`).

**3. A start/end state machine that removes itself.** The hook carries a `filter` flag
(`:159`). Before the start pattern it passes lines through; on `start_pattern` it flips
`filter = true` (`:190`); on `end_pattern` it **removes its own hook** and flips back
(`:172-174`):

```ruby
              if ignore_end || line =~ end_pattern
                DownstreamHook.remove(name)
                filter = false
```

`end_pattern` defaults to `/<prompt/` and accepts `:ignore` for a single-line capture
(`:149`, `:160`).

**4. A timeout that unwinds cleanly** (`:171`, `:215-222`):

```ruby
        Timeout::timeout(timeout, Interrupt) {
          …
        }
      rescue Interrupt
        nil
      ensure
        DownstreamHook.remove(name)
        Script.current.silent = save_script_silent if !silent.nil?
        Script.current.want_downstream = save_want_downstream
        Script.current.want_downstream_xml = save_want_downstream_xml
      end
```

Default 5 seconds. The `ensure` is belt-and-braces: the hook removes itself on the end
pattern *and* on the way out, so a timeout mid-capture cannot leave a filter installed on
the shared stream.

**5. `quiet:` — suppressing the capture from the frontend without breaking it.** This is
the subtlest part and it carries a genuine hard-won lesson. `quiet: true` drops the
captured lines, but drops them through `preserve_quiet_state_tags` (`:177-186`), because
a dropped chunk can carry a frontend *state* tag with it (`util.rb:80-91`):

```ruby
    # Self-closing tags that toggle persistent client display state (as
    # opposed to e.g. <prompt>, which self-heals on the next prompt). The raw
    # server chunk handed to a DownstreamHook proc is not split on line
    # boundaries, so the server is free to bundle one of these onto the same
    # chunk as text that quiet: true is about to drop. Silently discarding
    # that chunk would discard the tag with it -- e.g. dropping a trailing
    # mono-closing <output class=""/> that rode in on the same chunk as the
    # <prompt> a quiet-filtered range ends on leaves the frontend stuck in
    # mono mode until an unrelated, later mono tag happens to close it.
```

`QUIET_STATE_TAGS` (`:99-101`) currently holds one pattern, `/<output class="[^"]*"\s*\/>/`,
combined into `QUIET_STATE_TAG_PATTERN` via `Regexp.union` (`:112`) — with the reason
given at `:103-111`: separate scans would return matches grouped by pattern rather than in
source order, which breaks as soon as a second open/close pair is added.

### 4.3 The two thin wrappers

```ruby
    def self.quiet_command_xml(command, start_pattern, end_pattern = /<prompt/, include_end = true, timeout = 5, silent = true)
      return issue_command(command, start_pattern, end_pattern, include_end: include_end, timeout: timeout, silent: silent, usexml: true, quiet: true)
    end

    def self.quiet_command(command, start_pattern, end_pattern, include_end = true, timeout = 5, silent = false…)
```

`util.rb:226` and `:230`. They differ only in `usexml:` (true vs false) and both force
`quiet: true, silent: true`. Note they take **positional** args while `issue_command` takes
keywords — a legacy seam worth not reproducing.

`silver_count` (`util.rb:245`) is the older, hand-rolled version of the same pattern
(its own `anon_hook`, its own filter flag, its own `ensure`) — evidence that
`issue_command` was extracted from repeated copies of this shape.

### 4.4 The general mechanism: `HookRegistry`

`lib/common/hook_registry.rb` (189 lines) is shared behaviour `extend`ed by both
`DownstreamHook` (62 lines) and `UpstreamHook` (62 lines) — the header says why
(`hook_registry.rb:5-8`):

```ruby
    # Shared behaviour for the down/upstream hook registries. DownstreamHook and
    # UpstreamHook are otherwise near-identical apart from their backing storage
    # and their per-direction +run+, so the registration/bookkeeping lives here
    # and a fix (e.g. to source tracking) lands in one place.
```

Each registration records four things beyond the proc (`hook_registry.rb:52-58`):
the source script name, the owner's `object_id`, the declared `persist` disposition, and a
numeric `priority` (higher runs first; equal priority keeps registration order, `:33-35`).

The `persist` tri-state is the ownership contract (`hook_registry.rb:26-32`):

```ruby
      # +persist+ declares what should happen to the hook when the registering
      # script dies:
      #   * +true+  - keep it (it is meant to outlive the script, e.g. ;alias)
      #   * +false+ - remove it (it is scoped to this script's lifetime)
      #   * +nil+   - undeclared: kept for backwards compatibility, but the death
      #               path warns once so the author can declare intent.
```

`cleanup_on_death(owner_id)` (`:81`) is wired from `ScriptDeath.on_death`
(`downstreamhook.rb:59`), and matches by `object_id` **not name**, so a `force: true`
sibling sharing a name is unaffected (`hook_registry.rb:70-72`).

`DownstreamHook.run` (`downstreamhook.rb:39-54`) is the execution: iterate hooks in
priority order, pass the string through each, `return nil` the moment a hook nils it
(a hook can swallow a line), and **remove any hook that raises** while reporting it:

```ruby
          rescue
            remove(key)
            respond "--- Lich: DownstreamHook: #{$!}"
```

### 4.5 What this implies for Cena

Cena will have *many Lua scripts across several sessions* wanting request/response
round-trips against a per-session stream. The requirements this code proves out:

| Requirement | Lich's evidence | Cena's version |
|---|---|---|
| A round-trip must be **scoped**: capture starts, captures, and tears itself down even on timeout | `util.rb:174-176`, `:215-222` | an RAII guard / `Drop` impl, not an `ensure` a script author can forget |
| Per-script stream-mode flags must be **saved and restored** by the mediator | `util.rb:162-168`, `:219-221` | the mediator owns the mode for the duration; a Lua script never sets it directly |
| Hooks must declare **lifetime relative to their owner**, and orphans must be swept | `hook_registry.rb:26-32`, `:81` | ownership is a `SessionId` + `ScriptId`; a dropped script handle sweeps its filters. Make `persist` **required**, not tri-state — Lich's `nil` case exists only for backwards compatibility and it warns about itself. |
| A raising filter must not take down the stream | `downstreamhook.rb:47-50` | a panicking Lua callback unwinds into an error log and its filter is deregistered |
| Suppressing output must not suppress **protocol state** | `util.rb:80-91`, `QUIET_STATE_TAGS` | Cena parses GSL/XML into structured frames before scripts see them, so "drop the text but keep the state transition" is a *type-level* distinction, not a regex on a raw chunk. **This whole class of bug disappears if the parse happens before the filter.** That is a strong argument for Cena parsing first and exposing typed frames to Lua, never raw chunks. |
| Priority ordering between filters must be explicit | `hook_registry.rb:33-35` | numeric priority, stable within a tier |

**The big one:** `issue_command` is fundamentally a *serialization* primitive that isn't
actually serialized. Nothing stops two scripts from running `issue_command` concurrently
against the same session — they each install a hook, and both hooks see the same lines.
The start/end patterns are the only thing keeping them apart, and only by luck of them
being different. **Cena should make this an actual queue**: per-session, a command
round-trip takes a turn. In Rust that is an async mutex or a command channel with
`oneshot` reply, and the Lua API becomes `session:request(cmd, opts) -> lines`, awaited.
That removes an entire category of cross-script interference that Lich can only mitigate.

---

## 5. The consumer's view: eohunter

### 5.1 Size, first — a correction

The brief describes eohunter as "83k lines." **UNVERIFIED and apparently wrong.** Measured:

| Scope | LOC |
|---|---|
| `scripts/eohunter/*.rb` (the 31 engine files) | 17,993 |
| All of `scripts/` | 27,068 |
| Every `.rb` in the repo (engine + `spec/` + `tools/`) | 43,156 |

144 files total. The engine proper is ~18k lines; ~43k counting its test suite. This does
not weaken the argument — it is still by a wide margin the largest script written
deliberately against the modern core — but the number should be right in a document Cena
is designed from.

### 5.2 Doug's criterion

Stated at `C:/Gemstone/eohunter/docs/core-consumption-audit.md:3-6`:

> Doug's criterion: what share of a script's work is done by Lich's own
> methods, modules and classes, rather than copies of them.

And the governance note at `:9-10`: "The group decides what goes into core. This document
is the list to decide from." That framing matters — this is a *community negotiating an
API boundary*, which is exactly the process Cena will have to run, and worth knowing the
shape of in advance.

### 5.3 The file-by-file score

Reproduced from `core-consumption-audit.md:14-29` (line counts are from the audit's tree
at `07b9ee7`, which is why they differ slightly from my `wc -l` of the current tree):

| File | Lines | Consumes core | Duplicates core |
|---|---|---|---|
| `actions.rb` | 305 | `put`, `get?`, `clear`, `Script#downstream_buffer` | the refusal ladder (`fput`), `dothistimeout`, `waitrt?` |
| `combat.rb` | 269 | `Spell#cast` / `force_cast` / `force_channel` / `force_evoke` / `force_incant`, `Spell#known?` / `affordable?` | the cast answer table (bigshot's) |
| `maneuvers.rb` | 451 | the PSM readers (`CMan`, `Weapon`, `Shield`, `Feat`, `Warcry`) | **nothing** |
| `routines.rb` | 1789 | `Lich::Util.issue_command`, `quiet_command_xml`, `Lich::Stash.equip_hands`, `CMan.available?` | raw sends where core has none |
| `cleanse.rb` | 1689 | `CMan.known?/available?/use`, `Feat.available?`, `Stance.change`, `Stash.equip_hands`, `issue_command` | `mana_pulse`, `wait_rt`, `stand_up` |
| `world.rb` | 927 | `XMLData`, `GameObj`, `Status`, `Effects`, `Map`, `Char`, `Stats`, `Skills`, `Experience`, `Creature`, `Claim`, `Disk`, `Group`, `Bounty`, `Spell` | one private ivar read; ~190 lines of Forge leftovers |
| `rest.rb` | 964 | `Lich::Gemstone::Fog.return`, `Stance.change`, `Script` | **nothing** |
| `travel.rb` | 270 | the go2 script, `Script.start`/`running?`/`kill` | **nothing** |
| `watch.rb` | 228 | `Combat::Messages`, the `:ucs`/`:attack` subscriptions, `DownstreamHook` for the profile's flee text | **nothing: the rules are core's now** |
| `group.rb` | 1531 | `DRb`, `Group.members`/`leader`, `Disk` | nothing in core to consume |
| `profile.rb` | 288 | none | bigshot's `load_settings`/`clean_value` (script-level) |
| `controller.rb` | 872 | `Script` child lifecycle and execution guards, `XMLData`, `GameObj`, `Room`, `Creature`, `Overwatch` | nothing; the LAB seam has no core equivalent |
| `tracking/targets/flee/wander/loot/maintain/survival/engage` | ~4035 | `Stance`, `GameObj.targets`, `Creature`, `Effects` | the rules are bigshot's; nothing is core's |

The load-bearing sentence (`core-consumption-audit.md:31-33`):

> Every send in the engine goes through one of four places: the ladder in
> actions.rb, `Spell#cast`, the PSM readers, or `Lich::Util.issue_command`.

**Four send paths for an 18,000-line engine.** That is what a good arbiter boundary buys,
and it is the number Cena should be aiming at.

I verified the `issue_command` claim directly — every `Lich::Util` call site in the engine:

| Call | File:line |
|---|---|
| `::Lich::Util.issue_command(command, Regexp.union(regex, /…wait…/), timeout: 5)` | `cleanse.rb:1225`, `cleanse.rb:1429` |
| `::Lich::Util.quiet_command_xml("assess ##{id}", ENDS)` | `combat.rb:132` |
| `::Lich::Util.quiet_command_xml("look at ##{id}", …)` | `maintain.rb:538` |
| `::Lich::Util.issue_command("appraise ##{@target.id}", …, quiet: true, silent: true)` | `routines.rb:322` |
| `::Lich::Util.quiet_command_xml("measure ##{id}", …)` | `routines.rb:1019` |
| `::Lich::Util.issue_command("look ##{@target.id}", …, quiet: true, silent: true)` | `routines.rb:1482` |

And the `Combat::Messages` consumption (`watch.rb:226-231`):

```ruby
        events = ::Lich::Gemstone::Combat::Messages.events
        if events.empty?
          tracker.off(handlers.delete(:messages)) if handlers[:messages]
        else
          handlers[:messages] = tracker.on(*events, name: "#{NAME}:messages") { |type, data| message(type, data) }
        end
```

Note what this does: it asks core **which events exist** and subscribes to all of them by
name, rather than hardcoding a list. `Messages.events` is authoritative
(`watch.rb:224`: "Messages.events is authoritative; reload payloads need not list names").
That is a script treating core as a *schema source*. It is the single clearest signal in
either codebase that the arbiter direction is real and being taken up.

### 5.4 The nine (actually twelve) PRs — and which have landed

The audit and `docs/guides/core-dependencies.md:20-31` list them. I checked each against
the lich-5 snapshot at `236a9a2`:

| PR | What it adds | Landed in this snapshot? | Evidence |
|---|---|---|---|
| #1578 | `Lich::Gemstone::Stance.change`, a shared stance setter | **Yes** | `lib/gemstone/stance.rb:137` `def self.change(target, wait: true, force: false, timeout: DEFAULT_TIMEOUT)` |
| #1579 | `Lich::Stash.wield`, `.hands`, `.open_container` | **Yes** | `lib/stash.rb:410` `def self.wield(param, hand: nil)`; `:471` `def self.hands(right: :keep, left: :keep)`; `:359` `def self.open_container(id)`; also `:616 def self.equip_hands`. Spec at `spec/lib/stash_wield_spec.rb` |
| #1580 | `Lich::Gemstone::Mana.pulse` | **Yes** | `lib/gemstone/mana.rb:31` `def self.pulse(spell = nil, timeout: 2)` |
| #1581 | `Lich::Gemstone::Bank`, currency refresh, WEALTH lines | **Yes** | `lib/gemstone/bank.rb` exists, `spec/lib/gemstone/bank_spec.rb` |
| #1582 | an `Injured` cache keyed on an injury fingerprint | **Yes** | `lib/gemstone/injured.rb:16` "Cache variables. These must live on Injured itself (the class body)" |
| #1583 | PSM `command` and `results_regex` readers | **Yes** | `lib/gemstone/psms.rb:259` `def self.results_regex(name, *patterns, results_of_interest: nil)`; per-class wrappers e.g. `psms/cman.rb:777`, `psms/armor.rb:250` |
| #1584 | `Lich::Gemstone::Fog`, the ways home | **Yes** | `lib/gemstone/fog.rb:93` `def self.return(method, rift: false, resting_room: nil)` |
| #1585 | a spell's start message refreshes a refreshable timer | **Yes** | `lib/common/spell.rb:119` `@duration[cast_type][:refreshable] = (span == 'refreshable')`; `:416 def refreshable?` |
| #1586 | `Combat::Messages` — non-combat message families as events | **Yes** | `lib/gemstone/combat/messages.rb` (180) + `defs/messages.rb` (240), entire section 3.6 above |
| #1587 | bounded `fput`: `max_resends`, `interrupt`, `resend_transient`, failure symbols | **Yes** | `lib/global_defs.rb:1486-1498` documents all four; `:1502-1509` reads them |
| #1588 | `Group.broken?` waits on `Lich::Claim::Lock` | **Yes** | `lib/gemstone/group.rb:390-391` `def self.broken?` / `sleep(0.1) while Lich::Claim::Lock.locked?` |
| #1589 | society `command` readers | **Yes** | `lib/gemstone/society.rb` exists, `spec/lib/gemstone/society_command_spec.rb` |

**All twelve have landed.** The eohunter docs are stale on this point — `roadmap.md:15`
still says "#1578 Stance, #1579 Stash wield/hands" are "in review",
`roadmap.md:16` says the Fog stand-in "goes away when #1584 merges", and
`core-consumption-audit.md:153` says "lich-5 #1587 open". The snapshot is at #1617, so
those docs are simply behind. Also listed, ATARI's, and outside the nine:
#1575 (bounded script execution guards), #1576 (combat observation provenance),
#1577 (static-only map route selection) — `core-dependencies.md:34-36`. The provenance
work in #1576 is visible in the `source:` parameter threaded through
`tracker.rb:299`, `async_processor.rb:40-47`, and `processor.rb`.

**What this tells Cena:** in roughly a single release cycle, a motivated script author got
twelve API additions into core rather than writing twelve more copies. The arbiter
direction is not just a stated intention — there is a working process behind it. That
process is the thing Cena needs an equivalent of, more than it needs any individual API.

### 5.5 The `fput` upstreaming and the one genuine disagreement

The audit's proposal (`core-consumption-audit.md:56-67`) was a keyword-option upgrade to
the existing method rather than a parallel one, and that is what landed. Core's `fput`
now reads (`lib/global_defs.rb:1485-1498`):

```ruby
  # Options via a trailing Hash argument: fput('cmd', 'pattern', timeout: 30)
  #   timeout:          seconds with no game response before giving up (60;
  #                     0 disables, the original behavior)
  #   max_resends:      how many times a refusal ("...wait 3", "struggle to
  #                     stand", stunned) may trigger a resend before giving
  #                     up (nil, the original: unbounded)
  #   interrupt:        a callable checked on every wait and before every
  #                     resend; true ends the send at once (nil: never)
  #   resend_transient: on a transient refusal that is not a stun or a
  #                     web (a "can't seem", "don't seem"), resend after a
  #                     quarter second instead of giving up (false, the
  #                     original; bigshot's bs_put resends)
  #   failures:         :false (the original: every failure returns false)
  #                     or :symbol - :no_response, :too_many_resends,
  #                     :interrupted, :dead, :refused - so a caller can
  #                     tell them apart
```

The comparison table the audit built (`core-consumption-audit.md:44-54`):

| | `fput` (before) | eohunter's ladder |
|---|---|---|
| "...wait N" | sleeps N, resends, **no cap** | sleeps N, resends, **capped at 5** |
| "struggle to stand" | `fput 'stand'`, resends, no cap | the same, capped |
| transient (stunned / can't seem / don't seem) | waits out stunned or webbed, else **gives up (`false`)** | waits out stunned or webbed, else **resends after 0.25 s**, capped |
| no answer at all | 60 s, then `false` | 30 s, then a Result |
| the engine's stop | none | checked on every wait |
| dead mid-wait | echoes, returns `false` | a Result the caller handles |

**The disagreement, and how it was resolved:** `resend_transient` landed **defaulting to
`false`** (`global_defs.rb:1507`):

```ruby
  resend_transient = option.call(:resend_transient) ? true : false
```

so core kept its give-up behaviour as the default and made the resend opt-in. The
audit had argued the other way (`:66-67`): "The ladder's cap is what makes the resend
safe; the PR should carry the resend under the cap."

What actually happened in the transient branch (`global_defs.rb:1592-1609`) is more
nuanced than either position, and better than both:

```ruby
      elsif string =~ /Sorry, you may only type ahead/
        return fail_with.call(:interrupted) if wait.call(1)
      elsif resend_transient
        return fail_with.call(:interrupted) if wait.call(0.25)
      else
        Script.execution_sleep 0.1
        script.downstream_buffer.unshift(string)
        return fail_with.call(:refused)
      end
```

The give-up path now **pushes the refusal line back onto the script's buffer**
(`:1601`) and returns a *named* `:refused` rather than a bare `false`. So the caller can
distinguish "the game said no" from "nothing happened."

And eohunter, having got the option, **uses both settings deliberately** —
`scripts/eohunter/actions.rb:194-195` and `:207-208`:

```ruby
        answer = fput(command, max_resends: MAX_RESENDS, timeout: SEND_DEADLINE, interrupt: @interrupt,
                               resend_transient: false, failures: :symbol)
```

```ruby
        fput(command, max_resends: MAX_RESENDS, timeout: SEND_DEADLINE, interrupt: @interrupt,
                      resend_transient: true, failures: :symbol)
```

with `MAX_RESENDS = 5` and `SEND_DEADLINE = 30` (`actions.rb:78`, `:80`). It also had to
add a guard core does not have (`actions.rb:81-88`):

```ruby
      # Refusals the ladder must not resend. fput's transient rung matches
      # "don't seem" (global_defs.rb 1760) and, with resend_transient on,
      # sleeps and sends again up to the cap - but a severed leg is not
      # transient, so the command went out five times and failed
      # :too_many_resends
      PERMANENT_REFUSALS = %r{
        You\sdon't\sseem\sto\sbe\sable\sto\smove\syour\s(?:legs|arms)\sto\sdo\sthat|
        You\sare\stoo\sinjured\sto\sdo\sthat
      }xi
```

**Which default should Cena pick?**

**Cena should default to bounded-resend-with-a-cap — i.e. eohunter's position, not core's —
but only because Cena can also fix the thing that makes core's caution correct.**

The reasoning:

1. Core's `false` default is right *for core*, because core has ~473 installed scripts
   that would change behaviour under it. Cena has zero legacy scripts. The
   backwards-compatibility argument that decided this in Lich **does not apply to Cena.**
2. Capped resend is the better behaviour on the merits. An uncapped resend is a runaway;
   a give-up is a silent failure the script author usually did not handle. A cap plus a
   named failure is the only option that is both safe and honest.
3. But eohunter's `PERMANENT_REFUSALS` guard proves the real defect: **"transient" is
   being inferred from prose.** "You don't seem to be able to move your legs" and "you
   don't seem to have that" are the same regex and opposite meanings. Resending a
   permanent refusal five times is worse than giving up once.
4. So Cena's version should **classify the refusal, not pattern-match the word**. Cena's
   parser already has to understand the game protocol; refusals should come back to Lua as
   a typed `Refusal { kind: Transient | Permanent | Roundtime(Duration) | Stunned | Webbed | Dead }`.
   Then the default policy is trivially correct: retry `Transient` and `Roundtime` under a
   cap, never retry `Permanent`, wait out `Stunned`/`Webbed`, abort on `Dead`.

**Recommended Cena default:** `max_resends: 5`, `timeout: 30s`, resend on classified
transient and roundtime refusals, never on permanent ones, always return a typed result
(`Ok(line)` / `Err(SendFailure::{NoResponse, TooManyResends, Interrupted, Dead, Refused(Refusal)})`)
— never a bare boolean. The interrupt is not optional: `interrupt:` should be implicit,
since Cena knows whether the script is being cancelled and Lua should not have to thread a
callable through 46 call sites the way eohunter had to (`actions.rb:41-48` describes
exactly that pain: "nothing ever supplied a root one, so every `interrupted?` guard … was
inert").

### 5.6 The gap list: where eohunter still had to duplicate core

This is the most directly actionable output of the audit for Cena — it is a list of
API surfaces a serious script needs and Lich does not provide.

| Gap | eohunter's workaround | Audit ref | Cena's API should have |
|---|---|---|---|
| **Raw sends with no core home**: `sheath`, `gird`, `store <hand>`, `reserve`, `rub my <container>`, `aim <part>`, `raise #id`, `open my #id` / `put #id in my #id`, the 650 `prep`/`cast` pair | raw strings through the ladder | `:117-127` | A verified-send primitive is the right answer here, **not** a method per game verb. Cena should expose a typed send with confirmation patterns and let the script vocabulary live in Lua. |
| **Store a hand / put into a named container** | done by hand; "ecleanse and eloot do the same by hand" | `:125-127`, `:153` | container/inventory manipulation as core operations |
| **Debuff level** — parsing "(N)" out of a debuff name | `Me#debuff_level` in `world.rb` | `:111-112` | effects should carry a numeric rank, not need parsing out of a display string |
| **`CreatureInstance#crtr_status_seen?`** | reads the private `@crtr_flags` ivar | `:107-110` | reading another object's private state should be *impossible*, which in Rust it is; so this must be a real accessor or the need must be met another way |
| **`Status#frozen?`** | `Me#frozen?` shim, "exists because bigshot calls a `frozen?` Lich never defined" | `:113-114` | a complete status predicate set |
| **An inbound-swing fact** — `:incoming_swing` | subscribe to `:attack` with `emit_attacks` on, or "ask for a lighter `:inbound` fact in a follow-up" | `:79-81` | event granularity should not force a script to take the heavy blob to learn one boolean |
| **Cross-session/cross-script coordination** | 1,531 lines of DRb in `group.rb`; "Lich has `Group` for the game's roster and **no cross-process layer**" | `:132-135` | **this is section 2's real gap** — see the verdict table |
| **A settings/profile reader** | duplicated bigshot's `load_settings`/`clean_value` | `:26`, `:141-143` | the audit explicitly says this belongs in scripts, not core — Cena should agree and provide only the storage primitive |

The audit is also clear about what is **not** core's (`:129-143`): the rules themselves —
when to rest, whom to fight, when to flee — are the script's. "Nothing in Lich decides
when to rest." Cena must hold that line: the arbiter provides facts, transport, and
mediation; policy is Lua's.

---

## 6. Verdict for Cena

### 6.1 Is the thesis true?

**Partially, and the parts differ sharply in maturity.** Scored honestly:

| Arbiter property | Status in Lich-5 today |
|---|---|
| Typed event subscriptions instead of text scraping | **Real and shipped.** 9,460 lines, consumed in production by eohunter, with a declared payload schema. |
| Mediated command execution | **Real and shipped**, but not actually serialized — it mitigates interference rather than preventing it. |
| A supported public API over sealed internals | **Declared, not load-bearing.** 44 lines with zero callers; the seal itself is real but `send`-able. |
| Cross-session coordination | **Built and tested, dormant behind a default-off flag.** Observation only — no cross-session messaging verb exists. |
| A process for growing the API rather than copying it | **Real.** Twelve PRs landed in roughly one cycle in response to one script's audit. |

So: the direction is genuine, it is being actively driven by real consumers, and it is
roughly 40% complete. **The thesis is a sound basis for Cena's design, provided Cena
designs from where Lich is heading rather than where it has arrived.**

### 6.2 The prioritized capability table

| # | Arbiter capability | Where Lich has it today | Complete or partial | What Cena's Rust core should own | What the Lua API should expose |
|---|---|---|---|---|---|
| 1 | **Typed event bus** | `lib/common/events.rb` (278); topics, wildcards, idempotent named subs, owner-scoped cleanup, isolated handlers (`:23-46`) | **Complete** and good | The bus. Topic registry, per-session scoping, subscriber isolation (a panicking Lua callback is caught and its sub removed), ordered synchronous delivery. Emit is `&self`, never re-entrant into the emitter. | `events.on(topic, fn)`, `events.off(handle)`, `events.emit(topic, payload)`. `persist` **required**, not optional. Handles are opaque. |
| 2 | **Typed game-fact events** | `lib/gemstone/combat/*` (9,460); 11 combat facts + 20 message events with a declared key-type table (`defs/messages.rb:51-71`) | **Complete for combat**; nothing equivalent for inventory, room, group, or economy | Parsing raw protocol → a real Rust `enum GameEvent` with typed variants. The schema is the type, not a comment. Deferred/reordered emit so facts land after the action they belong to (`processor.rb:175-183`). | Subscription by variant name; payloads as Lua tables with stable keys. Expose `events()` so scripts can enumerate the schema (eohunter does exactly this: `watch.rb:226`). |
| 3 | **Never let a subscriber block the stream** | `async_processor.rb` — single ordered worker, O(1) enqueue, respawn-on-death (`:8-10`, `:29-35`) | **Complete**, and the reasoning is sound | The socket task never runs script code. One ordered consumer per session so event order is causal. Backpressure policy explicit (Lich drops oldest at 200 lines, `tracker.rb:548`). | Callbacks that must be cheap — but enforced, not requested: a callback that blocks past a budget is killed and logged. Lich can only *ask* (`tracker.rb:82-86`); Cena can enforce. |
| 4 | **Mediated request/response** | `Lich::Util.issue_command` (`util.rb:156-223`) — scoped hook, flag save/restore, start/end capture, timeout, `ensure` teardown | **Partial**: correct per-call, but concurrent callers are not serialized | A **per-session command queue**. One round-trip at a time, with a `oneshot` reply. Teardown via `Drop`, not discipline. Stream-mode flags owned by the mediator for the call's duration. | `session:request(cmd, {start=, stop=, timeout=, quiet=})` → lines, awaited. Scripts never register raw stream filters to do a round-trip. |
| 5 | **Verified send with typed failure** | `fput` with `max_resends`/`timeout`/`interrupt`/`resend_transient`/`failures: :symbol` (`global_defs.rb:1485-1498`) | **Complete as an option set**; **incomplete** in that refusals are classified by regex, not by type | Refusal **classification** as a parser output: `Transient \| Permanent \| Roundtime(d) \| Stunned \| Webbed \| Dead`. Retry policy over the classification. Cancellation implicit. | `send(cmd, {max_resends=5, timeout=30})` → `ok, line` or `nil, err` where `err` is a typed reason. Never a bare `false`. |
| 6 | **Read models to poll, events for edges** | Stated doctrine at `events.rb:47-51`; realized in `Creature`, `GameObj`, `XMLData`, `Effects`, `Status` (eohunter's `world.rb` consumes 15 of them) | **Complete as doctrine**, unevenly realized | Authoritative per-session state as owned Rust data. Reads hand out **snapshots**, never interior references. | `session.char`, `session.room`, `session.creatures` etc. returning plain tables. Cheap, always-current, never a handle into core's interior. |
| 7 | **Hook/filter ownership and lifetime** | `hook_registry.rb` (189): owner `object_id`, `persist` tri-state, numeric priority, sweep on script death, remove-on-raise (`:26-35`, `:81`; `downstreamhook.rb:47-50`) | **Complete**, except `persist: nil` exists only for legacy and warns about itself | Registry keyed by `(SessionId, ScriptId)`. Lifetime declared at registration and enforced on drop. Priority explicit. A panicking filter is removed, never fatal. | `session:filter(fn, {priority=, persist=})` → handle. `persist` mandatory. Filters see **typed frames**, not raw chunks — which deletes the `QUIET_STATE_TAGS` class of bug entirely (`util.rb:80-91`). |
| 8 | **Public API over sealed internals** | `lib/api/active_sessions.rb` (44) + `private_class_method` throughout `internal_api/` | **Aspirational** — zero callers; the seal is defeated by `send` at `lifecycle.rb:282` | The boundary as a crate/module split: `pub` for the script-facing API, `pub(crate)` for everything else. The Lua binding layer is the *only* thing that can see the script-facing API. | Exactly one namespace. If it is not in the Lua API, no script can reach it — not by convention, by compilation. |
| 9 | **Cross-session visibility** | `internal_api/active_sessions/*` (1,643), dormant behind `FEATURE_FLAG` (`:36`, `:88-91`) | **Built, off by default; observation only** | An in-process `SessionRegistry`. Snapshots, not handles (`registry.rb:63`). An explicitly enumerated cross-session payload (`lifecycle.rb:326-337`). `connected` distinct from presence (`lifecycle.rb:15-20`). Generation counters for restart races (`:49`, `:317`). **No heartbeat** — task-handle drop is the death signal. | `sessions.list()` → array of snapshot tables; `sessions.self()`. Read-only. A script cannot obtain another session's script table, buffer, or socket. |
| 10 | **Cross-session coordination (messaging)** | **Does not exist.** Protocol is `ping`/`upsert`/`remove`/`snapshot` (`server.rb:228-242`). eohunter built 1,531 lines of DRb instead (`group.rb`) | **Absent** | An explicit mailbox: typed messages between sessions, delivered to a session's own executor, never executed in the sender's context. Capability-gated. | `sessions.send(id, msg)` / `events.on("session.message", fn)`. Deliberately narrow. This is Cena's biggest greenfield opportunity and its biggest footgun. |
| 11 | **Feature gating for risky subsystems** | `FEATURE_FLAG = :active_sessions_api` (`:36`), default off (`:93`) | **Complete pattern**, one user | Config-gated subsystems with inert fallbacks, never errors (`api/active_sessions.rb:14-20`). | A script asking for a disabled capability gets an empty/inert answer, not an exception. |

### 6.3 Direction vs. legacy: which corpus should the Lua API be modeled on?

The two corpora point opposite ways:

| | The installed corpus | eohunter |
|---|---|---|
| Size | ~473 scripts (**UNVERIFIED** — no script corpus is present at `C:/Gemstone/*.lic` or `C:/Gemstone/scripts/*.lic` in this environment; the count comes from the workflow brief, not from anything I read) | 17,993 LOC engine, 43,156 with specs (verified by `wc -l`) |
| Style | blocking global functions: `fput`, `waitfor`, `matchwait`, `dothistimeout`, raw `DownstreamHook` regex scraping | four send paths total; typed event subscription; PSM readers; `issue_command` for round-trips |
| Relationship to core | copies core's behaviour into each script | consumes core and **files PRs for what is missing** — twelve landed |
| What it proves | what Cena would break | what Cena should be able to express |

**Cena's Lua API should be modeled on eohunter's direction, without reservation.**

The reasons are decisive:

1. **Cena is not source-compatible with Lich anyway.** The scripting language changes from
   Ruby to Lua. Every one of the 473 scripts has to be rewritten or mechanically ported
   regardless. The usual argument for preserving a legacy API — "don't break what works" —
   has already been conceded by the choice of Lua. Having paid that cost, spending it on
   the *old* API would be a pure loss.
2. **The legacy style is structurally incompatible with Cena's own goals.** Blocking
   globals assume one implicit session, one implicit script context, and a thread per
   script. Cena is multi-session by design and mobile-constrained. `fput` with no session
   argument has no meaning when three characters are logged in. The legacy API would have
   to be broken for multi-session even if the language had stayed the same.
3. **eohunter is a ready-made conformance test.** It is an 18,000-line script that
   deliberately consumes core rather than duplicating it, *with a written list of exactly
   which core capabilities it depends on* (`core-consumption-audit.md`,
   `docs/guides/core-dependencies.md`). If Cena's Lua API can express eohunter, the API is
   adequate for serious work. That is a far better target than "can it run the legacy
   corpus," which measures compatibility rather than quality.
4. **The gap list in §5.6 is free design input.** Someone has already done the work of
   finding where a serious script hits the wall. Cena gets to build those in from the
   start instead of discovering them in year two.

**What that choice costs — stated plainly:**

| Cost | Severity | Mitigation |
|---|---|---|
| Every existing script must be ported by hand; no mechanical translation of `fput`/`waitfor` to an async, session-scoped, typed API | **High.** This is the real price. | Accept it. It is already paid by the Ruby→Lua move. |
| Script authors must learn subscription + async round-trips instead of "send and read lines." A genuine conceptual step up. | **Medium.** | Ship a small compatibility shim in *Lua* (not Rust) providing `fput`-alikes over the real API — the way eohunter's `actions.rb` wraps `fput`. A shim in Lua is deletable; a shim in the core API is forever. |
| Cena's core must do more work up front: protocol parsing to typed frames, refusal classification, a per-session command queue, session registry — before the first useful script runs | **Medium-high.** More before first light. | These are exactly the things that are painful to retrofit. §2 and §4 both show Lich paying retrofit costs right now (the `send` at `lifecycle.rb:282`, the unserialized `issue_command`). Front-load them. |
| The typed-event catalog must be *complete enough* or scripts fall back to raw text scraping and the whole benefit evaporates | **High risk.** Lich has this for combat only. | Do not ship raw-line access to Lua as a convenient first-class API. Ship it as an explicit, discouraged escape hatch, so the pressure runs toward extending the schema — which is precisely the process that produced the twelve PRs. |
| Losing the "installed corpus works" claim costs adoption momentum | **Medium.** | eohunter-class scripts are where the value is; a client that runs the flagship script well beats one that runs 473 scripts adequately. |

### 6.4 The four things to get right before writing any Lua binding

1. **Parse to typed frames first; never hand Lua a raw chunk.** This single decision
   deletes `QUIET_STATE_TAGS` (`util.rb:80-112`), most of `defs/` regex gating, and the
   refusal-classification problem at once.
2. **Snapshots, never handles, across any boundary** — cross-session (`registry.rb:63`),
   script-to-core state reads, and event payloads alike.
3. **Serialize the command round-trip per session.** Lich cannot; Cena can, and the bug
   class it removes is invisible until three scripts are running.
4. **Make the API boundary a compilation boundary**, so §1's aspiration becomes §1's
   guarantee. This is the one place where Rust gives Cena something Lich structurally
   cannot have, and it is free if done on day one and expensive later.

---

## Appendix: unverified or unconfirmed claims

- **"473 installed scripts."** Taken from the workflow brief. No script corpus exists at
  `C:/Gemstone/*.lic` or `C:/Gemstone/scripts/*.lic` in this environment; I could not
  confirm the count or the claim about its style. The direction-vs-legacy argument in
  §6.3 does not depend on the exact number, only on the corpus being legacy-styled — which
  is itself **UNVERIFIED** here, though consistent with `core-consumption-audit.md`'s
  description of bigshot and ecleanse.
- **"eohunter is 83k lines."** Contradicted by measurement (§5.1): 17,993 LOC in the
  engine, 43,156 repo-wide including specs and tools.
- **PR numbers.** I confirmed the *capabilities* named by #1578-#1589 are present in the
  lich-5 snapshot by locating the code. I did **not** verify that each capability arrived
  via that specific PR number — `git log --oneline -60` in the snapshot shows no commits
  referencing those numbers in their subject lines, so the mapping between number and
  code rests on eohunter's docs, not on the lich-5 history.
- **Whether `Lich::API` is loaded at runtime by anything outside this tree.** I verified no
  in-tree `require` and no in-tree caller. An external script could `require` it directly;
  I have no way to check that from these two repositories.
- **eohunter's own doc staleness.** `roadmap.md:15-16` and
  `core-consumption-audit.md:153` describe PRs as open that are present in the lich-5
  snapshot. I report this as a discrepancy between the two sources, not as evidence about
  which is correct about merge status upstream.
- **`processor.rb` (2,436 lines) and `recorder.rb` (1,061 lines)** were read at their emit
  sites and header comments, not line by line. Claims about the event catalog are grounded
  in the emit calls I cite; I do not claim the catalog is exhaustive.
