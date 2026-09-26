//! The words Hydra's code and documents use: one word per concept, and what
//! each word is not.
//!
//! `plan/05` §8 makes this binding on code, docs and plans. When a concept
//! has a word here, that is the word. A new concept gets its word added here
//! before a second word for it can spread.
//!
//! It is rustdoc rather than a table in `plan/05` for the reason
//! [`architecture`](crate::architecture) is: **every term links to the item
//! it names**, and the workspace denies broken intra-doc links, so renaming a
//! type without its entry is a red build rather than a glossary that goes on
//! describing code that has gone. The table it replaced on 2026-09-24 had
//! seven terms. Two named designs that are not being built, one named a
//! mechanism the code calls something else, and none of the words the code
//! now turns on -- generation, authority, gate, guard, held -- were in it.
//! **Retired** at the end records the three.
//!
//! The **Not** column is evidence, not taste. A word there is one the old
//! table ruled out, or one the code already uses for something else, so that
//! using it here would give one word two meanings. Where no such word is
//! known the cell is empty.
//!
//! # Words that already mean more than one thing
//!
//! These are live in the code today. None is renamed here; renaming is the
//! author's call. Until one is, **say which**.
//!
//! - **claim** is three unrelated things. Taking the authority
//!   ([`SessionHandle::claim`]). A Hydra command being taken by whoever runs
//!   its word ([`Claimed`], in a module named `claimant`). And a hunting room
//!   being *mine*, with nobody in it who is not in my group: [`claim`], the
//!   port of Lich's `claim.rb`. The collision reaches the
//!   docs: [`Origin::Manual`] says manual input "is not a claimant", meaning
//!   the authority's claimant of `plan/12` §4.1, not the `claimant` module.
//! - **ladder** is three things in the code, none of them `plan/05`'s old
//!   entry (see **Retired**). The reconnect ladder of waits, [`backoff`]
//!   (1, 2, 5, 10, 30 seconds), which the supervisor climbs and which
//!   "the retry ladder" also names. The credential ladder,
//!   [`secrets`](crate::secrets): the keyring, then an environment variable,
//!   then a prompt. And the movement ladder, [`movement`]: Lich's chain of
//!   `elsif` patterns, ported in order. It is also a thing in the game that a
//!   character climbs, which the map and travel tests mean.
//! - **session** is the concept in the table below, one character played
//!   across reconnects: a [`SupervisedSession`]. [`Session`] is a narrower
//!   type, one connection's actor bundled with the handle that drives it.
//! - **state** alone is ambiguous. The lifecycle is [`State`]; what is known
//!   about the character and the world is [`GameState`]. Say "lifecycle" or
//!   "game state".
//! - **event** is [`Event`], what a session publishes, and [`ObservedEvent`],
//!   one of those numbered on an observer's stream. The combat model's
//!   *attack event* is a third, inside one [`Event::Combat`].
//! - **stream** is a game stream window (thoughts, speech, logons:
//!   [`stream_windows`]), the event stream an observer reads, and the merged
//!   streams across characters ([`Merger`]).
//! - **wire** is the game's wire (bytes and markup, `plan/15`) and the
//!   frontend's wire ([`ServerMessage`], [`ClientMessage`] and
//!   `crates/cena-ui/WIRE.md`). "The wire log" is the game's.
//! - **Desk** is three types in two senses. The session's
//!   [`claimant::Desk`] holds the command symbol and the one runner every
//!   Hydra command goes to. A behavior's desk, [`travel::Desk`] or
//!   [`hunt::Desk`], runs that behavior's commands for one session.
//! - **Unknown** is a fact nobody has stated, or one a reconnect invalidated,
//!   and never `0`. [`UnknownTag`] is a tag the parser does not model.
//!   `Claimed::Unknown` is a Hydra command nobody knows.
//! - **held** is a hunt step imported with a guard Hydra has not built
//!   ([`Step::held`]). The authority being held by another claimant is
//!   [`AuthorityHeld`].
//! - **role** is a group member's part, lead, follow or solo
//!   ([`group::Role`]), and a spell's kind in the spell table
//!   ([`spells::Role`]). The combat model's message families have a third
//!   `Role` (add, remove, start, end).
//! - **report** is what a group member tells its leader ([`group::Report`]),
//!   and the `;loot` and `;combat` reports printed for the player
//!   (`crates/cena/src/loot.rs`, `crates/cena/src/combat.rs`).
//! - **script** is not a Hydra concept (see **Retired**), and survives in
//!   three places: the Lich scripts Hydra ports from, bigshot's `script` step
//!   that the importer turns into a sequence, and the **scripted game**, the
//!   fake server that tests talk to.
//!
//! # The wire and the model
//!
//! | Term | Means | Not |
//! |---|---|---|
//! | **Frame** | one typed thing the game told us: [`Frame`] | message, packet, event |
//! | **Runs** | one line's text, split where its style changes: [`Runs`] | |
//! | **Parser** | the **one** thing that turns bytes into frames, one per session, in `cena-protocol` (`plan/12` §3a) | |
//! | **Classifier** | recognises a game fact in one frame or line and remembers nothing: [`CritTables`], [`movement`] | parser |
//! | **Consumer** | needs memory across lines, so lives above the model and reads frames and classifier answers: the combat recorder, a trip | classifier |
//! | **Game state** | what is known about the character and the world, folded one frame at a time: [`GameState`], [`GameState::apply`] | state |
//! | **Unknown** | a fact nobody has stated, or one that [`GameState::invalidate_for_reconnect`] withdrew; never `0`, which is a real value (`plan/12` §5.2) | zero, default |
//! | **Unknown tag** | a tag the parser does not model, kept whole as [`Frame::UnknownTag`] and recorded as an [`UnknownTag`] so a display can show it | |
//!
//! # A session
//!
//! | Term | Means | Not |
//! |---|---|---|
//! | **Session** | one character, played, across reconnects: a [`SupervisedSession`] | connection, account, client |
//! | **Generation** | one connection of a session. [`Generation`] advances on each reconnect, so anything in flight from the last one is stale by construction | epoch |
//! | **Session id** | which session, for its whole life: [`SessionId`]. With a generation it names one connection | |
//! | **Actor** | the task that owns one generation's socket, parser, command queue and game state: [`SessionActor`] | |
//! | **Supervisor** | what outlives a connection, running a new actor per generation and carrying [`SessionCore`] across | |
//! | **Lifecycle** | [`State`]: Connecting -> Authenticating -> Syncing -> Ready -> Reconnecting -> Closed | game state |
//! | **Syncing** | the login burst, before the first prompt after `<endSetup/>`: [`State::Syncing`] | |
//! | **Ready** | the only state a behavior runs in: [`State::Ready`], [`State::behaviors_may_run`] | |
//! | **Reconnect** | wait one rung of [`backoff`], then a full re-login through the [`Connector`] | |
//! | **Attendance** | whether a **person** is using the session: what they type or click, never a behavior's traffic (`crates/cena-session/src/command/attendance.rs`). A connection that lived [`LONG_LIVED`] counts as attended too, unless the server warned it was idle. [`MAX_UNATTENDED_LOSSES`] unattended losses in a row stop the reconnecting | |
//!
//! # Read, observe, act
//!
//! The three seams of `plan/12` §3, and what crosses each.
//!
//! | Term | Means | Not |
//! |---|---|---|
//! | **Snapshot** | an owned, point-in-time copy of the game state, taken at an exact place in the event stream: [`Snapshot`] | view, handle, ref |
//! | **Event** | something the session saw or did, published to observers: [`Event`] (a frame, a combat chunk, a command sent, a notice) | signal, trigger, hook |
//! | **Observer** | reads a snapshot and every numbered event after it ([`SessionObserver::subscribe`], [`ObservedEvent`]); cannot mutate, cannot suppress, confers no authority | |
//! | **Lagged** | what an observer that fell behind is told instead of meeting a silent hole; the recovery is to subscribe again | |
//! | **Notice** | Hydra speaking to the player, not the game: [`Notice`], the port of `Lich::Messaging` | message |
//! | **Handle** | the only path that sends: [`SessionHandle`] | |
//! | **Command** | a line for the game, through the session's [`CommandQueue`] | |
//! | **Hydra command** | a typed line that starts with the command symbol ([`COMMAND_SYMBOL`], `;` unless the character's settings say otherwise). It is Hydra's whether or not anything knows the word, and the game never sees it ([`commands`](crate::commands)) | |
//! | **Manual input** | what a person types, at a page or the terminal: [`Origin::Manual`]. It goes to the head of the queue, never touches the authority, and never aborts the holder (`plan/12` §4.1) | claimant |
//! | **Round trip** | one command and the frames that answer it, until the next prompt or a timeout: [`SessionHandle::send_and_await`], answered with an [`Outcome`]. A timeout means no match in the window, never "it did not happen" | ladder |
//! | **Gate** | a precondition the actor checks against the live state at the moment it writes, not when the behavior asked: [`Gate`], on [`SessionHandle::send_now`] | guard |
//! | **Authority** | the right to send a sequence of commands for a session, held by one claimant at a time as an [`AuthorityToken`]: taken with [`SessionHandle::claim`], given back with [`SessionHandle::release`], and refused, not queued, to a second claimant ([`AuthorityHeld`]) | lock |
//! | **Claimant** | whoever may hold the authority (`plan/12` §4.1): a behavior, later an agent, and never manual input | |
//! | **Preempt** | take the authority from its holder, cooperatively for [`PREEMPT_GRACE`] and then by force: [`SessionHandle::preempt`], [`Preempted`]. Only an explicit stop, or the watchdog, preempts | |
//! | **Refusal** | why the session would not send a command: [`Refusal`] | |
//! | **Quit** | send `quit` and wait for the game to hang up: [`SessionHandle::quit`], told as a [`Farewell`] | |
//!
//! # Behaviors
//!
//! | Term | Means | Not |
//! |---|---|---|
//! | **Behavior** | curated Rust automation that a user configures with data and never programs. A pure state machine and a thin driver ([`Trip::tick`] and [`travel()`]); there is no `Behavior` trait | script |
//! | **Driver** | a behavior's async half: claims the authority, sends what the machine asks for, and races every await against the stop token | |
//! | **Watchdog** | a behavior beats a [`Heartbeat`] each time round its loop, and [`watch`] preempts it when the beats stop for [`BEHAVIOR_WATCHDOG`] | |
//! | **Sync** | the character sync once a login is Ready, asking the game only for what the character store says is stale: [`sync()`] | |
//! | **Trip** | one journey on the map, as a machine: [`Trip`] | |
//! | **Desk** | one behavior's handler for its Hydra commands, one per session: [`travel::Desk`], [`hunt::Desk`] | |
//! | **Batch** | a list of commands sent for the player in order: `;multi` repeats one, `;foreach` runs one on each item that matches ([`batch`]). One of each kind at a time per session ([`batch::Desk`]); a Hydra command in it is run and waited for | chain, script |
//! | **Hunt** | the hunting behavior: [`Hunt`], a pure machine that takes the profile and the state and answers with one thing to do, and [`hunt()`], the driver that runs it | |
//! | **Profile** | the data a hunt runs on, one TOML file: rooms, stances, rest, targets and their routines ([`Profile`]) | script |
//! | **Chain** | how a profile key resolves: the character's, then the profile's, then the global, then the built-in default ([`chain`], `plan/12` §6a.2) | |
//! | **Routine** | the steps taken against a target, in order: [`Profile::routines`] | |
//! | **Sequence** | a named list of steps that a routine step may stand for, such as `volley`, written by hand where bigshot ran a script: [`Profile::sequences`] | script |
//! | **Step** | one line of a routine or sequence: what is sent, and its guards ([`Step`]) | |
//! | **Guard** | a named precondition on a step, from the closed vocabulary Hydra defines ([`Guard`]); with its polarity, a [`Condition`]. A step's guards must all hold, and there is no *or* | gate |
//! | **Held** | a step imported with a guard or shape Hydra does not read yet: kept, with the reason named, and never run ([`Step::held`]) | dropped |
//! | **Import** | a bigshot profile in, a Hydra profile out, with what it could not carry named at the head of the file: [`import()`], [`Import`] | |
//!
//! "Dropped" is the importer's other outcome and a different one: a bigshot
//! key it does not carry at all. A held step is still in the profile.
//!
//! # Hunting as a group
//!
//! `plan/39`. The game's own group is [`group::Group`](cena_session::group::Group);
//! these are Hydra's words for hunting in one.
//!
//! | Term | Means | Not |
//! |---|---|---|
//! | **Role** | a member's part: lead, follow or solo, read off the game's group and never chosen ([`group::Role`], [`group::role`]) | |
//! | **Report** | what one member of a group publishes for the leader to read: its connection, room, rest reason and what keeps it ([`group::Report`]); the leader adds [`group::Leading`] | |
//! | **Muster** | gathering the group: what it does about a member apart from the leader, and until when ([`group::Muster`], [`group::muster`]) | rally |
//! | **Lost wait** | how long each muster wait lasts before the group stops waiting, 90 seconds by default ([`group::Settings::lost_wait`]) | |
//! | **Successor** | the member who leads when the leader is lost: the first present on the `successors` list, else the healthiest ([`group::successor`]) | |
//! | **Rally rooms** | the rooms walked through on the way back to hunt ([`Rooms::rally`]) | rally alone: bigshot's word also names its handshake (`rallying at`) and Troubadour's Rally, which is 1040 by its name (`plan/39` §0e) |
//!
//! # Many sessions, and the frontends
//!
//! | Term | Means | Not |
//! |---|---|---|
//! | **Host** | the table of sessions one Hydra runs: add, remove, one per account, stop them all ([`Host`], [`stop_all`]) | |
//! | **Account** | the login a character is on. The game keeps one character per account online, so the table refuses a second ([`AddError::AccountInUse`]) | session |
//! | **Roster** | which account and game each character is on, holding no secret: [`roster`](crate::roster) | |
//! | **Projection** | a game state turned into the vocabulary a frontend renders, reading no clock and changing nothing: [`SessionView::project`] | snapshot |
//! | **Frontend wire** | [`ServerMessage`] and [`ClientMessage`], versioned by [`WIRE_VERSION`] and written down in `crates/cena-ui/WIRE.md`. No model or protocol type crosses it | |
//! | **Despana** | the embedded browser viewer: one loopback listener inside the binary ([`cena_web`], [`WebServer`]) | |
//! | **Pairing token** | the secret a browser presents before any state is sent; held in memory, for this process only | |
//! | **Hub** | Despana's page for every character: a card each, start, quit, reconnect, and the merged streams. Its requests reach the binary as [`HubRequest`]s through [`HubControl`] | |
//! | **Character page** | one character's own page. No window shows two characters' story text (`plan/29` §5a) | |
//! | **Merged streams** | thoughts, speech, logons, deaths and announcements across characters, each line once: [`Merger`] | |
//! | **Container look** | the main-stream line `In the box you see a, b and c.`; with `;sorter` on, one that is a list end to end shows as one line per category ([`LineAssembler::sort_containers`]) | inventory, which is the `inv` window's feed |
//!
//! # Names
//!
//! | Term | Means | Not |
//! |---|---|---|
//! | **Hydra** | the product: the binary, and anything a person sees | cena, in anything a person sees |
//! | **cena** | the working name: the repository, the crate prefixes, the `CENA_*` variables. A rename is a mechanical sweep at a milestone boundary (`CLAUDE.md`) | a third name |
//!
//! # Retired
//!
//! Terms `plan/05` §8 defined that name nothing being built. Not reused for
//! anything else while they stand here, so an old document still reads
//! unambiguously.
//!
//! | Term | Was | Why it went |
//! |---|---|---|
//! | **Script** | one Lua program in a session | scripting is deferred, not built, and nothing may be designed around it (`CLAUDE.md`, Settled decisions). A proposal to add it later is a live question; until then the word names nothing of Hydra's |
//! | **Adapter** | the per-game implementation, `GameAdapter` | refused: the second game is deferred all-or-nothing, and an abstraction for it is not built ahead of it (`plan/12` §9d) |
//! | **Ladder** | the resend-and-recover protocol around one command | Lich's `fput`. Hydra's counterpart is the **round trip**, and the word is taken by three ladders the code does have |
//!
//! [`AddError::AccountInUse`]: cena_host::AddError::AccountInUse
//! [`AuthorityHeld`]: cena_session::AuthorityHeld
//! [`AuthorityToken`]: cena_session::AuthorityToken
//! [`BEHAVIOR_WATCHDOG`]: cena_behavior::BEHAVIOR_WATCHDOG
//! [`COMMAND_SYMBOL`]: cena_session::command::COMMAND_SYMBOL
//! [`Claimed`]: cena_session::command::Claimed
//! [`ClientMessage`]: cena_ui::ClientMessage
//! [`CommandQueue`]: cena_session::CommandQueue
//! [`Condition`]: cena_behavior::hunt::Condition
//! [`Connector`]: cena_session::Connector
//! [`CritTables`]: cena_session::CritTables
//! [`Event::Combat`]: cena_session::Event::Combat
//! [`Event`]: cena_session::Event
//! [`Farewell`]: cena_session::Farewell
//! [`Frame::UnknownTag`]: cena_session::Frame::UnknownTag
//! [`Frame`]: cena_session::Frame
//! [`GameState::apply`]: cena_session::GameState::apply
//! [`GameState::invalidate_for_reconnect`]: cena_session::GameState::invalidate_for_reconnect
//! [`GameState`]: cena_session::GameState
//! [`Gate`]: cena_session::Gate
//! [`Generation`]: cena_session::Generation
//! [`Guard`]: cena_behavior::hunt::Guard
//! [`Heartbeat`]: cena_behavior::Heartbeat
//! [`Host`]: cena_host::Host
//! [`Hunt`]: cena_behavior::hunt::Hunt
//! [`HubControl`]: cena_web::HubControl
//! [`HubRequest`]: cena_web::HubRequest
//! [`Import`]: cena_behavior::hunt::Import
//! [`LONG_LIVED`]: cena_session::LONG_LIVED
//! [`LineAssembler::sort_containers`]: cena_ui::LineAssembler::sort_containers
//! [`MAX_UNATTENDED_LOSSES`]: cena_session::MAX_UNATTENDED_LOSSES
//! [`Merger`]: cena_ui::Merger
//! [`Notice`]: cena_session::Notice
//! [`ObservedEvent`]: cena_session::ObservedEvent
//! [`Origin::Manual`]: cena_session::Origin::Manual
//! [`Outcome`]: cena_session::Outcome
//! [`PREEMPT_GRACE`]: cena_session::PREEMPT_GRACE
//! [`Preempted`]: cena_session::Preempted
//! [`Profile::routines`]: cena_behavior::hunt::Profile::routines
//! [`Profile::sequences`]: cena_behavior::hunt::Profile::sequences
//! [`Profile`]: cena_behavior::hunt::Profile
//! [`Refusal`]: cena_session::Refusal
//! [`Rooms::rally`]: field@cena_behavior::hunt::profile::Rooms::rally
//! [`Runs`]: cena_session::Runs
//! [`ServerMessage`]: cena_ui::ServerMessage
//! [`Session`]: cena_session::Session
//! [`SessionActor`]: cena_session::SessionActor
//! [`SessionCore`]: cena_session::SessionCore
//! [`SessionHandle::claim`]: cena_session::SessionHandle::claim
//! [`SessionHandle::preempt`]: cena_session::SessionHandle::preempt
//! [`SessionHandle::quit`]: cena_session::SessionHandle::quit
//! [`SessionHandle::release`]: cena_session::SessionHandle::release
//! [`SessionHandle::send_and_await`]: cena_session::SessionHandle::send_and_await
//! [`SessionHandle::send_now`]: cena_session::SessionHandle::send_now
//! [`SessionHandle`]: cena_session::SessionHandle
//! [`SessionId`]: cena_session::SessionId
//! [`SessionObserver::subscribe`]: cena_session::SessionObserver::subscribe
//! [`SessionView::project`]: cena_ui::SessionView::project
//! [`Snapshot`]: cena_session::Snapshot
//! [`State::Ready`]: cena_session::State::Ready
//! [`State::Syncing`]: cena_session::State::Syncing
//! [`State::behaviors_may_run`]: cena_session::State::behaviors_may_run
//! [`State`]: cena_session::State
//! [`Step::held`]: field@cena_behavior::hunt::Step::held
//! [`Step`]: cena_behavior::hunt::Step
//! [`SupervisedSession`]: cena_session::SupervisedSession
//! [`Trip::tick`]: cena_behavior::travel::Trip::tick
//! [`Trip`]: cena_behavior::travel::Trip
//! [`UnknownTag`]: cena_session::UnknownTag
//! [`WIRE_VERSION`]: cena_ui::WIRE_VERSION
//! [`WebServer`]: cena_web::WebServer
//! [`backoff`]: cena_session::backoff
//! [`batch::Desk`]: cena_behavior::batch::Desk
//! [`batch`]: mod@cena_behavior::batch
//! [`chain`]: cena_behavior::hunt::chain
//! [`claim`]: cena_session::claim
//! [`claimant::Desk`]: cena_session::command::claimant::Desk
//! [`hunt()`]: fn@cena_behavior::hunt::hunt
//! [`group::Leading`]: cena_behavior::group::Leading
//! [`group::Muster`]: cena_behavior::group::Muster
//! [`group::Report`]: cena_behavior::group::Report
//! [`group::Role`]: cena_behavior::group::Role
//! [`group::Settings::lost_wait`]: field@cena_behavior::group::Settings::lost_wait
//! [`group::muster`]: fn@cena_behavior::group::muster
//! [`group::role`]: fn@cena_behavior::group::role
//! [`group::successor`]: fn@cena_behavior::group::successor
//! [`hunt::Desk`]: cena_behavior::hunt::Desk
//! [`import()`]: fn@cena_behavior::hunt::import
//! [`movement`]: cena_session::movement
//! [`spells::Role`]: cena_session::spells::Role
//! [`stop_all`]: cena_host::stop_all
//! [`stream_windows`]: cena_session::stream_windows
//! [`sync()`]: fn@cena_behavior::sync::sync
//! [`travel()`]: fn@cena_behavior::travel::travel
//! [`travel::Desk`]: cena_behavior::travel::Desk
//! [`watch`]: cena_behavior::watch
