//! How Hydra is put together: the crate graph, the flow of one line of game
//! text, the three seams, and the tests that hold each of them in place.
//!
//! This page is the map of the workspace for a reader who has the generated
//! reference open. It says what each crate is, which way the arrows point,
//! what happens to one line from the game, and which rule keeps each seam
//! where it is. **Every item it names is an intra-doc link**, and the
//! workspace builds its docs with broken links denied (`Cargo.toml`,
//! `[workspace.lints.rustdoc]`), so this page cannot go on naming something
//! that has moved or gone. That is why it is rustdoc and not a Markdown file:
//! a hand-written summary drifts, and the architecture tests scan `.rs` files
//! rather than `.md` ones, so here its citations and its length are checked
//! as well.
//!
//! It is not the specification. `plan/12` decides what gets built and wins
//! every disagreement; `plan/05` is how it is written; `CLAUDE.md` carries
//! the one-screen version. This page describes the tree as it stood on
//! **2026-09-28**, and where it gives a number it gives the command.
//!
//! # One binary, many heads
//!
//! Hydra is one process that plays several characters against the live game
//! at once. Each character is a **session**: one actor that owns its socket,
//! its parser, its command queue and its typed game state, running as one
//! spawned task. Sessions share nothing. There are no process globals
//! (`every_static_is_allowlisted`), and anything that reads across sessions is
//! a consumer above them, never a component between them. A frontend attaches
//! to a session and never owns it: the native session keeps running when the
//! last browser tab closes. Automation is curated Rust behaviors configured by
//! data, and Hydra embeds no scripting language. What else acts on a
//! character runs outside the process and comes in through a door the session
//! hands out: an agent, the player's scripts, and the player's own Lich
//! (**Outside programs**, below).
//!
//! # The crate graph is the architecture
//!
//! Thirteen crates, one per layer, with dependencies pointing one way: down.
//! Cargo enforces the acyclic half. The other half -- a forbidden edge that
//! happens to close no cycle, such as the projection reaching into the
//! session -- is `crate_dependency_edges_match_the_plan`
//! (`crates/cena-arch-tests/tests/layering.rs`), which asserts the resolved
//! graph **equal** to its `ALLOWED_EDGES` table. A missing edge fails as
//! surely as an extra one, which is what makes that table a statement of the
//! architecture rather than a floor under it. This is the graph it asserts:
//!
//! ```text
//! cena              the binary: Hydra                  agent, behavior, gui, host, platform, session, ui, web
//! cena-agent        MCP for an outside program         map, session, platform*
//! cena-gui          the window (egui)                  session, ui
//! cena-web          the embedded browser viewer        session, ui
//! cena-ui           projection and wire vocabulary     model
//! cena-host         the table of sessions              session, platform*
//! cena-behavior     curated behaviors                  session, map, platform*
//! cena-session      one character, as one actor        model, protocol, platform
//! cena-model        typed game state and game data     protocol
//! cena-map          the map's vocabulary               --
//! cena-protocol     bytes in, frames out               platform
//! cena-platform     the pipe, the login, the log       --
//! cena-arch-tests   the rules the compiler cannot say  --
//!
//! * a dev-dependency only, for the scripted game its tests talk to;
//!   `a_dev_only_edge_stays_out_of_the_shipped_graph` keeps it out of the binary.
//! ```
//!
//! From outside the workspace, the binary alone also takes hydra-mapper's
//! layout engine, `cena-map-layout`, and the map Hydra ships, `cena-gs-map`,
//! both at one pinned commit, for the minimap (`plan/53` §7). The engine's own
//! `cena-map` is patched to this workspace's, so there is one `Map`. The
//! layering test's table covers the workspace's crates, not these.
//!
//! | Crate | Start reading at |
//! |---|---|
//! | [`cena_platform`] | [`ByteSource`](cena_platform::ByteSource), and [`eaccess`](cena_platform::eaccess) for the login |
//! | `cena-protocol` | [`Frame`](cena_session::Frame), the vocabulary, and [`Runs`](cena_session::Runs), one line's styled text |
//! | `cena-model` | [`GameState`](cena_session::GameState) |
//! | `cena-map` | [`Map`](cena_behavior::travel::Map) |
//! | [`cena_session`] | its crate page, which gives the reading order |
//! | [`cena_behavior`] | [`travel`](cena_behavior::travel) |
//! | [`cena_ui`] | [`SessionView`](cena_ui::SessionView), and `crates/cena-ui/WIRE.md` for the contract |
//! | [`cena_gui`] | [`Sessions`](cena_gui::Sessions), and [`run`](cena_gui::run) for the window |
//! | [`cena_web`] | [`WebServer`](cena_web::WebServer) |
//! | [`cena_agent`] | [`Characters`](cena_agent::Characters), and `crates/cena-agent/CONTRACT.md` for the contract; a script runner's [`scripts`](cena_agent::scripts), `SCRIPTS.md` |
//! | [`cena_host`] | [`Host`](cena_host::Host) |
//! | `cena-arch-tests` | `crates/cena-arch-tests/tests/layering.rs` |
//!
//! Three crates are named without a link because the binary has no direct
//! edge to them. `cena` may reach `cena-model`, `cena-protocol` and `cena-map`
//! only through the crates that use them, and an intra-doc link obeys the same
//! graph. Their entry points above are the re-exports `cena-session` and
//! `cena-behavior` make of types they themselves use, which is the condition
//! `plan/05` §-1 sets for a re-export.
//!
//! # What happens to one line of game text
//!
//! ```text
//! socket --bytes--> Parser --Frame--> GameState::apply --> consumers
//! cena-platform     cena-protocol     cena-model           cena-session and above
//! ```
//!
//! 1. **Bytes.** A session's bytes come through a
//!    [`ByteSource`](cena_platform::ByteSource): a
//!    [`LiveSource`](cena_platform::LiveSource) over TLS to the game, or a
//!    [`ReplaySource`](cena_platform::ReplaySource) over a recording, which is
//!    how a whole session replays in a test with no network. The source yields
//!    bytes, not lines and not frames: line reassembly belongs to the parser,
//!    and a frame-yielding transport would need the parser below the crate it
//!    lives in. While the player's Lich runs, the bytes are also copied to it
//!    as they arrive, before the parser, and carried, never read
//!    ([`LichDoor`](cena_session::script::lich::LichDoor)).
//! 2. **Frames.** `cena-protocol`'s `Parser` is the **one** thing that turns
//!    bytes into structure (`plan/12` §3a). It is per-session and stateful,
//!    hand-rolled because the wire has no root element, and it never errors on
//!    unknown input: an unmodelled tag becomes
//!    [`Frame::UnknownTag`](cena_session::Frame::UnknownTag) and a tag with no
//!    closing `>` becomes [`Frame::MalformedTag`](cena_session::Frame::MalformedTag).
//!    The [`Frame`](cena_session::Frame) vocabulary is ported whole from
//!    `VellumFE`'s `ParsedElement`, and every fact the markup encodes survives
//!    into it, because nothing above may re-read markup to recover one.
//! 3. **State.** The session folds each frame into its
//!    [`GameState`](cena_session::GameState) with
//!    [`GameState::apply`](cena_session::GameState::apply). Everything under
//!    `cena-model`'s `state/` that recognises a game fact is a **stateless
//!    classifier** over one frame or line: the crit tables
//!    ([`CritTables`](cena_session::CritTables)), the item-type patterns
//!    (`gameobj`), the movement lines ([`movement`](cena_session::movement)).
//!    The test for which side of that line something is on: **does it need to
//!    remember anything?** A consumer that does -- a combat tracker, a trip --
//!    lives above the model and reads frames plus classifier answers. Nothing
//!    is lost on the way: every field of a consumed frame lands in the state,
//!    is routed elsewhere, or carries the author's written decision to drop it
//!    (`plan/05` Rule 2.2a). An unknown tag is recorded into the state
//!    ([`UnknownTag`](cena_session::UnknownTag)) so that it reaches a display,
//!    not merely survives.
//! 4. **Consumers.** The session's own first: the round trip waiting for its
//!    answer, the combat recorder, the player log, and the line itself,
//!    finished once, answered by `;sorter` and the character's triggers, and
//!    published for every viewer ([`Event::Line`](cena_session::Event::Line)).
//!    Then whoever observes
//!    ([`SessionObserver`](cena_session::SessionObserver)): the behaviors, the
//!    terminal, the window ([`cena_gui`]), the projection
//!    ([`SessionView::project`](cena_ui::SessionView::project)) that the web
//!    server encodes ([`ServerMessage`](cena_ui::ServerMessage)) for a
//!    browser, an agent, and a script runner, which reads each line as the
//!    game sent it, before `;sorter` and the triggers
//!    ([`Event::Heard`](cena_session::Event::Heard)).
//!
//! **With the player's Lich running, what is shown is Lich's.** What Lich
//! writes for a frontend is parsed as the game's is and put together into
//! lines the same way, and the window and the browser draw those in the
//! game's place, so Lich's squelches and its scripts' messages show. The
//! game's own parse is still everything else: the model, the log, what the
//! triggers do, and what a script runner hears (`plan/51` step 3).
//!
//! Nothing above `cena-protocol` sees a raw byte or an unparsed string
//! (`plan/05` Rule 2.1). `wire_text_reaches_the_public_api_only_through_rule_2_2`
//! scans every crate's public surface for a raw-text escape, and each one it
//! allows must name the rule that compels it
//! (`every_raw_text_escape_names_the_rule_that_compels_it`).
//!
//! # A session: one actor per connection, one supervisor per character
//!
//! [`SessionActor`](cena_session::SessionActor) is one task and one `select`
//! loop over one socket. It holds its source by value and
//! [`run`](cena_session::SessionActor::run) consumes it, so "one actor, one
//! connection" is structural rather than a convention. It owns the parser, the
//! [`CommandQueue`](cena_session::CommandQueue), the `GameState` and the
//! recorder; observation requests are answered by that owner in one
//! synchronous turn, which is what gives a snapshot and its event stream an
//! exact fence.
//!
//! [`SupervisedSession`](cena_session::SupervisedSession) is what outlives a
//! connection. It runs a **new actor per generation** and carries the durable
//! parts across in [`SessionCore`](cena_session::SessionCore). Between
//! connections, in this order:
//! [`GameState::invalidate_for_reconnect`](cena_session::GameState::invalidate_for_reconnect)
//! makes what cannot be trusted `Unknown` -- roundtime, stance, room, hands;
//! never `0`, which is a real value (`plan/12` §5.2) -- then the
//! [`Generation`](cena_session::Generation) advances so anything in flight is
//! stale by construction, [`State::Reconnecting`](cena_session::State::Reconnecting)
//! is published, one rung of the [`backoff`](cena_session::backoff) ladder is
//! waited racing the stop token, and the [`Connector`](cena_session::Connector)
//! is asked for a transport. Every reconnect is a full re-login, which is why
//! the credentials live for the session, and why the connector is implemented
//! in the binary: `cena-session` must not learn a login protocol, and
//! `cena-platform`, where the login lives, sits below the trait it would have
//! to implement. Reconnect is bounded: a fatal login error stops it at once,
//! and [`MAX_UNATTENDED_LOSSES`](cena_session::MAX_UNATTENDED_LOSSES) drops
//! with no command sent stop an idle session.
//!
//! The lifecycle is [`State`](cena_session::State):
//!
//! ```text
//! Connecting -> Authenticating -> Syncing -> Ready -> Reconnecting -> Closed
//! ```
//!
//! `Syncing` is the login burst; `Ready` is the first prompt after
//! `<endSetup/>`, and behaviors run **only** there
//! ([`State::behaviors_may_run`](cena_session::State::behaviors_may_run)), so
//! "no automation during a reconnect" is a state, not a convention.
//! `plan/12` §5.1's `Degraded` is deliberately not built: leaving it needs a
//! targeted re-sync that does not exist, and a state with an entry and no
//! exit is a trap.
//!
//! **A panic kills one session, not the process.** Each session is a spawned
//! task on the [`Host`](cena_host::Host)'s table, and the release profile
//! keeps `panic = "unwind"`, asserted by `no_profile_aborts_on_panic`. What
//! it does not yet survive is the session itself: an actor panic unwinds
//! through its supervisor, so that character ends rather than reconnects
//! ([`cena_session::actor`]'s docs record why).
//!
//! # Three seams: read, observe, act
//!
//! `plan/12` §3 names them, and in Rust they are types rather than an API
//! wall:
//!
//! | Seam | Type | Rule |
//! |---|---|---|
//! | **read** | a [`Snapshot`](cena_session::Snapshot) of the `GameState` | read-only; no parsing, no sending |
//! | **observe** | [`SessionObserver::subscribe`](cena_session::SessionObserver::subscribe): a snapshot and the numbered [`ObservedEvent`](cena_session::ObservedEvent) stream that continues it | cannot mutate, cannot suppress; confers no authority |
//! | **act** | [`SessionHandle`](cena_session::SessionHandle) | the only path that sends |
//!
//! **Observation joins without a gap and lags out loud.** The snapshot and
//! its stream share one fence, every event after the snapshot's cursor is
//! delivered, and a subscriber that falls behind gets a `Lagged` marker
//! rather than a silent hole; recovery is to subscribe again (`plan/12` §6).
//! State is latest-wins and events are lossless within a bounded window:
//! vitals may coalesce, "you were stunned" may not vanish.
//!
//! **Acting is arm-before-send, structurally.**
//! [`send_and_await`](cena_session::SessionHandle::send_and_await) is one
//! call with no public send-then-wait pair, so the race cannot be written. A
//! round trip owns the frame stream from its bytes going out until the next
//! prompt frame, or its timeout: the game carries no command ids, so
//! attribution is temporal (`plan/12` §4.4). The answer is a typed
//! [`Outcome`](cena_session::Outcome), and `Timeout` means "no match within
//! the window", never "it did not happen". Instant actions go by
//! [`send_now`](cena_session::SessionHandle::send_now), with a
//! [`Gate`](cena_session::Gate) the actor evaluates against the live state at
//! the moment it writes, because a precondition a behavior checked before a
//! roundtime is stale by the time the command goes out.
//!
//! **One owner at a time, and typing is not an owner.** The session holds one
//! [`AuthorityToken`](cena_session::AuthorityToken); a behavior takes it with
//! [`claim`](cena_session::SessionHandle::claim) to run a sequence and gives
//! it back with [`release`](cena_session::SessionHandle::release) on every
//! exit. A second claimant is refused with
//! [`AuthorityHeld`](cena_session::AuthorityHeld), not queued: silent queueing
//! is how an attack fires four seconds after the fight ended. Manual input
//! ([`Origin::Manual`](cena_session::Origin::Manual)) is not a claimant. It
//! goes to the head of the queue, runs its round trip, and the holder
//! continues, so answering a whisper never aborts a hunt (`plan/12` §4.1, the
//! correction everything else rests on). Only an explicit stop preempts,
//! cooperatively for [`PREEMPT_GRACE`](cena_session::PREEMPT_GRACE) and then
//! by taking the token. A command from a browser is `Origin::Manual` too, the
//! same path as one typed at the terminal, which is why no frontend can get
//! this rule wrong on its own.
//!
//! # Behaviors are curated Rust
//!
//! [`cena_behavior`] holds the automation, and it is Rust that a user
//! configures with data, never Rust that runs a user's code. The shape that
//! has paid off is **a pure state machine and a thin driver**:
//! [`Trip::tick`](cena_behavior::travel::Trip::tick) is told where the walker
//! is and what time it is and answers with what to do -- no socket, no clock,
//! no random numbers -- and [`travel`](cena_behavior::travel::travel()) is the
//! driver that claims the authority, feeds it frames, sends what it asks for,
//! and races every await against the stop token. A replay drives the machine
//! frame by frame in a test. Hunt, M6's behavior (`plan/30` §3), takes the
//! same shape, with its policies as an enum consulted in priority order under
//! **one** holder of the authority: `plan/12` §4.2's supervisor behavior, not
//! several actors racing for the queue.
//!
//! There is still no `Behavior` trait. What the behaviors share is three
//! lines each, and a trait over it would leave every signature different; the
//! rule of three has not been met (`plan/05` §-1). A wedged behavior is
//! detectable: it beats a [`Heartbeat`](cena_behavior::Heartbeat) each time
//! round its loop, and [`watch`](cena_behavior::watch) beside it preempts it
//! when the beats stop for
//! [`BEHAVIOR_WATCHDOG`](cena_behavior::BEHAVIOR_WATCHDOG). A behavior that
//! ends because the session decided -- stopped, refused, disconnected -- ends
//! with a [`BehaviorError`](cena_behavior::BehaviorError).
//! [`sync`](cena_behavior::sync::sync) is the smallest one: the character sync
//! that runs once a login is `Ready`, asking the game only for the groups the
//! character store says are stale.
//!
//! # Many heads: the session table
//!
//! [`Host`](cena_host::Host) is what knows one Hydra runs several characters
//! (`plan/29`). A session knows nothing of any other; the table adds and
//! removes them while Hydra runs, gives each its
//! [`SessionId`](cena_session::SessionId), refuses a second character on an
//! account that already has one online
//! ([`AddError::AccountInUse`](cena_host::AddError::AccountInUse), because the
//! game would knock the first off), and stops them all in order
//! ([`stop_all`](cena_host::stop_all)). It is built to sit behind one shared
//! lock: [`Host::take`](cena_host::Host::take) removes a session at once, and
//! the slow part, `quit` and waiting for the game to hang up, is
//! [`Hosted::stop`](cena_host::Hosted::stop), awaited with the lock released,
//! so no frontend waits on another character's goodbye. A session that ended
//! on its own stays on the table so a hub can say why, until the character is
//! added again: [`Host::ended`](cena_host::Host::ended) names it, and the
//! binary takes it off first, so a lookup by name never finds the dead one.
//!
//! It is a crate rather than part of the binary because its callers include
//! the web hub, and a frontend cannot depend on the binary. Yet `cena-web` has
//! no edge to it either: a hub's add, remove, reconnect and shutdown travel as
//! a [`HubRequest`](cena_ui::HubRequest) to whoever registered a
//! [`HubControl`](cena_ui::HubControl), which is the binary, which owns the
//! table. Anything that reads across sessions -- the merge of thoughts,
//! speech, logons, deaths and announcements into one stream
//! ([`Merger`](cena_ui::Merger)) -- is a consumer of N sessions above them,
//! never a component shared between two.
//!
//! # Frontends: a projection, a wire, a server inside the binary
//!
//! [`cena_ui`] is pure presentation.
//! [`SessionView::project`](cena_ui::SessionView::project) turns a
//! `GameState` into the small explicit vocabulary a frontend renders, given
//! the lifecycle and the server time by its caller, so it reads no clock and
//! changes nothing. No model or protocol object is ever serialized to a
//! frontend; what crosses is [`ServerMessage`](cena_ui::ServerMessage) and
//! [`ClientMessage`](cena_ui::ClientMessage), versioned by
//! [`WIRE_VERSION`](cena_ui::WIRE_VERSION) and written down in
//! `crates/cena-ui/WIRE.md`. The crate depends on no UI toolkit
//! (`cena_ui_depends_on_no_ui_toolkit`), and the frontend input vocabulary
//! ([`validate_command`](cena_ui::validate_command)) lives here rather than in
//! any frontend.
//!
//! [`cena_web`] is Despana, the embedded viewer.
//! [`WebServer`](cena_web::WebServer) is one loopback listener inside the
//! binary that serves the bundled assets, carries snapshots and deltas to a
//! browser over a `WebSocket` through one bounded pump per session, and
//! carries commands back into the session's queue as manual input. A pairing
//! token, held in memory for this process only, is required before any state
//! is sent. One listener serves a **hub** page -- a card per character, start,
//! quit, reconnect, and the merged streams -- and a page per character,
//! because no window may show two characters' story text (`plan/29` §5a). A
//! viewer arriving or leaving never starts or stops the native session; the
//! binary's [`frontend`](crate::frontend) owns the server's lifetime
//! independently of any tab.
//!
//! [`cena_gui`] is the desktop window (`plan/47`, `plan/49`), and what the
//! binary opens with no arguments: a hub of cards, one per character, and a
//! play window per character built of widgets, observing the same sessions
//! as Despana and sending through the same manual path. A log window per
//! character (`plan/25` step 8) reads the player log back through
//! `cena_session::player_log`'s reader, off the window's thread, after the
//! writer is asked to flush. The trigger editor (`plan/54`) is a window over the
//! one triggers file: the binary reads it into [`Book`](cena_ui::triggers::Book)
//! and makes each change through the writer `;trigger` uses, and the window runs
//! a form through the real matcher to test it.
//!
//! # Outside programs: an agent, scripts and the player's Lich
//!
//! Three kinds of program outside Hydra act on a character. Each comes in
//! through a door of its own that the session hands out, never the
//! [`SessionHandle`](cena_session::SessionHandle) itself, and all three are
//! served by [`cena_agent`], whose sources never name the handle
//! (`the_agent_acts_only_through_the_door`).
//!
//! | Who | Door | What it may do | Read |
//! |---|---|---|---|
//! | an **agent**, over MCP on loopback | [`agent::Door`](cena_session::agent::Door) | what the character's level allows, each act checked; the player asked for one above it; a denylist at every level | `plan/35`, `crates/cena-agent/CONTRACT.md`, [`Characters`](cena_agent::Characters) |
//! | the player's **scripts**, in a script runner (Ruby with Lich's engine) | [`script::Door`](cena_session::script::Door) | send lines, read the game's lines and a local copy of the state, and answer hooks within a deadline | `plan/46`, `crates/cena-agent/SCRIPTS.md`, [`scripts`](cena_agent::scripts) |
//! | the player's own **Lich**, through the Lich relay | [`LichDoor`](cena_session::script::lich::LichDoor) | read the game's bytes as they came, send lines, have what the player types, and show its text in the game's place | `plan/51`, [`cena_agent::lich`], [`script::lich`](cena_session::script::lich) |
//!
//! **The Lich relay keeps Hydra's connection to the game.** Lich runs in
//! pipe mode against a loopback port Hydra holds
//! ([`run`](cena_agent::lich::run)), is handed the game's bytes as they
//! arrive, and what it writes to that port is sent as its lines
//! ([`Origin::Lich`](cena_session::Origin::Lich)). A typed line is Hydra's
//! with Hydra's command symbol, Lich's with Lich's
//! ([`LICH_SYMBOL`](cena_session::script::lich::LICH_SYMBOL)), and otherwise
//! Lich's while it runs, for its aliases and hooks; both symbols start as
//! `;` and Hydra's is tried first, so a player running Lich gives Hydra
//! another. A Lich started after the login is handed a login built from the
//! model; it stays up through a reconnect; and one that stops reading is let
//! go rather than fed a stream with a hole in it. It is off until the player
//! turns it on for the character ([`lich`](crate::lich)).
//!
//! # The binary is the join
//!
//! `cena` is where the ends meet that no crate may hold together. It reaches
//! the live game in every mode; nothing in the workspace runs it, and only the
//! author does (`CLAUDE.md`, Credentials).
//!
//! | Module | Job |
//! |---|---|
//! | [`play`](crate::play) | the one run path: settle every login up front, put each character on the table, run until Ctrl-C |
//! | [`connector`](crate::connector) | the real login behind [`Connector`](cena_session::Connector); the reason it is here is above |
//! | [`secrets`](crate::secrets) | the credential ladder: the OS keyring, then an environment variable, then a prompt that does not echo |
//! | [`roster`](crate::roster) | which account and game each character is on; holds no secret |
//! | [`setup`](crate::setup) | what every session is given: wire log, stores, combat recorder, player log, and how each is flushed |
//! | [`commands`](crate::commands) | Hydra's own command line: the symbol decides, the word routes, nothing reaches the game |
//! | [`history`](crate::history) | `;history`: the player log read back, its days, tail, a time window and search, over [`reader`](cena_session::player_log::reader) |
//! | [`travel`](crate::travel) | the map loaded and travel's desk registered on that command line |
//! | [`learn`](crate::learn) | the character sync, once a login is `Ready` |
//! | [`frontend`](crate::frontend) | the web server's lifetime, when `--web` asked for one |
//! | [`gui`](crate::gui) | the windowed run: the hub on the main thread, the session table on the runtime's workers |
//! | [`pages`](crate::pages) | the settings menu's pages for one character, each change through its `;` command's own writer |
//! | [`triggers`](crate::triggers) | the one triggers file, given to each session, and `;trigger` |
//! | [`relay`](crate::relay) | `;to` and `;all`: a line typed on one character and sent on another, or on every one |
//! | [`agent`](crate::agent) | `--agent`'s MCP listener, and `;agent`, the player's control of what an agent may do |
//! | [`scripts`](crate::scripts) | `;name args`: the player's Lich script in the character's script runner |
//! | [`lich`](crate::lich) | `;lich`: the player's own Lich for a character, through the Lich relay, on or off |
//! | [`watch`](crate::watch) | the terminal's view: Hydra's own lines, tagged by character, and no game text |
//! | [`interrupt`](crate::interrupt) | Ctrl-C as an orderly quit from every phase; a second one exits at once |
//! | [`ask`](crate::ask) | asking the person at the keyboard for what a login needs |
//! | [`hunt`](crate::hunt) | `;hunt`: a hunt on a profile, its settings, a group led, stopped; the profile chain read for the menu |
//! | [`perform`](crate::perform) | what an agent may run at the `behaviors` level, the author's allowlist, and how each is started |
//! | [`batch`](crate::batch) | `;multi` and `;foreach`: a command many times, or once for each thing |
//! | [`loot`](crate::loot), [`combat`](crate::combat) | the ledger's and the combat recorder's reports |
//! | [`sorter`](crate::sorter) | `;sorter`: a container's contents one line per category, kept per character |
//! | [`doll`](crate::doll) | `;doll`: the injury doll's import |
//! | [`attention`](crate::attention) | a trigger's sound and notification, played once per occurrence across characters |
//! | [`launcher`](crate::launcher), [`proven`](crate::proven) | the Not launched tab's asks of the binary, and what a login proven `Ready` leaves behind |
//! | [`general`](crate::general), [`hunt_pages`](crate::hunt_pages), [`travel_page`](crate::travel_page) | the settings menu's pages over the character's own file, the hunt profiles and travel's settings |
//! | [`map_context`](crate::map_context), [`atlas`](crate::atlas) | the one map per process, and its areas laid out for the minimap |
//! | [`hunt_setup`](crate::hunt_setup) | Despana's hunt setup handler over a session observer |
//!
//! (The table had left these out; a reader of *Outside programs* above most
//! needs [`perform`](crate::perform) and [`hunt`](crate::hunt). The review
//! of 2026-09-29.)
//!
//! # What holds it in shape
//!
//! A rule that is not enforced is a wish (`plan/05` §0), so the rules the
//! compiler cannot express are tests in `crates/cena-arch-tests/tests/`,
//! written when each rule was adopted rather than retrofitted:
//!
//! | Rule | Test | File |
//! |---|---|---|
//! | dependencies point one way, and the graph equals the table | `crate_dependency_edges_match_the_plan` | `layering.rs` |
//! | the projection knows no toolkit; the model and the map do no file I/O | `cena_ui_depends_on_no_ui_toolkit`, `model_does_no_file_io`, `map_does_no_file_io` | `layering.rs` |
//! | an agent acts only through the session's door, which checks its level, and a script runner only through its own | `the_agent_acts_only_through_the_door` | `layering.rs` |
//! | no process globals: every `static` is allowlisted with a reason | `every_static_is_allowlisted`, `no_static_mut_anywhere` | `architecture.rs` |
//! | one owning field each for the roundtime, the server clock and the session handle | `roundtime_has_a_single_owning_field` and its two siblings | `single_owner.rs` |
//! | nothing above the protocol sees raw text | `wire_text_reaches_the_public_api_only_through_rule_2_2` | `raw_text_escapes.rs` |
//! | every file under its cap, exceptions justified, caps only turn down | `no_source_file_exceeds_its_line_cap`, `the_cap_ratchet_only_turns_down` | `file_rules.rs`, `ratchet.rs` |
//! | a `lib.rs` or `mod.rs` re-exports and wires; it does not implement | `facade_files_stay_facades` | `file_rules.rs` |
//! | game names stay under game modules | `game_names_outside_game_modules_are_flagged` | `file_rules.rs` |
//! | no `include!`; an `include_str!` embeds only what the scans read | `no_source_file_is_included_from_outside_the_scan` | `include_ban.rs` |
//! | every `path:line` citation in code and in `plan/` resolves | `every_path_rooted_citation_resolves` | `citations.rs` |
//! | every member inherits the workspace lints, and none reopens a denied one | `every_member_crate_inherits_the_workspace_lints` | `lints_are_inherited.rs` |
//! | a panic unwinds; no profile aborts | `no_profile_aborts_on_panic` | `file_rules.rs` |
//! | every rule `plan/05` tags as test-enforced names a test that exists | `every_covered_rule_names_a_test_that_exists` | `ratchet.rs` |
//!
//! The lints are the other half, and they sit in the root `Cargo.toml` so
//! that every crate inherits them: `missing_docs`, broken and private
//! intra-doc links, `unwrap_used`, `expect_used`, `panic`, `todo` and
//! `unsafe_code` are all denied, and clippy's pedantic group is on. Clippy at
//! `-D warnings` is part of the build, not an optional pass.
//!
//! # Measured, 2026-09-28
//!
//! | | count | command |
//! |---|---|---|
//! | workspace members | 13 | `sed -n '/^members/,/^\]/p' Cargo.toml \| grep -c '"crates/'` |
//! | known wire tags | 126 | `grep -cE '^    "[^"]+",$' crates/cena-protocol/src/tags.rs` |
//! | `Frame` variants | 56 | `awk '/^pub enum Frame \{/,/^\}/' crates/cena-protocol/src/frame/vocabulary.rs \| grep -oE '^    [A-Z][A-Za-z0-9]*' \| sort -u \| wc -l` |
//! | files under `cena-model`'s `state/` | 116 | `find crates/cena-model/src/state -name '*.rs' \| wc -l` |
//! | architecture test files | 10 | `ls crates/cena-arch-tests/tests/*.rs \| wc -l` |
//!
//! # Where to read next
//!
//! In dependency order, each crate page saying what to read within it:
//! [`cena_platform`] for the pipe; `cena-protocol` through
//! [`Frame`](cena_session::Frame) for the vocabulary; `cena-model` through
//! [`GameState`](cena_session::GameState) for what is known;
//! [`cena_session`] in the order [`queue`](cena_session::queue),
//! [`lifecycle`](cena_session::lifecycle), [`command`](cena_session::command),
//! [`actor`](cena_session::actor), [`supervisor`](cena_session::supervisor);
//! [`cena_behavior`] with [`travel`](cena_behavior::travel) first;
//! [`cena_ui`], [`cena_gui`] and [`cena_web`] for the viewers; [`cena_host`] for the table;
//! [`cena_agent`] for the agent, a script runner and the Lich relay;
//! and [`play`](crate::play) for how the binary ties them together. The
//! words this page uses, and the ones that already mean two things, are in
//! [`glossary`](crate::glossary). For the reasoning behind any of it,
//! `plan/12` first and `plan/05` beside it.
//! `research/` holds designs that were reversed or deferred, and is never
//! instructions.
