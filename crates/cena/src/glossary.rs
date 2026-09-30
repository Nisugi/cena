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
//!   *attack event* is a third, inside one [`Event::Combat`]. A trigger's
//!   `event` is a fourth: what the model reads a finished line as, a
//!   [`LineEvent`]; call it a **line event**. What a script runner is told is
//!   a fifth, at positions of its own
//!   ([`listening::Event`](cena_agent::scripts::listening::Event)).
//! - **flag** is a name a trigger sets, which the guard word `flag "<name>"`
//!   reads ([`Flags`]), and a creature's `<crtrStatus>` flags, which the
//!   guard words `ascended`, `mini_boss` and the rest read ([`Fact::Flag`]).
//!   Say "a trigger's flag" or "a creature's flag".
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
//!   [`AuthorityHeld`]. A trigger's `held` is what an import kept that Hydra
//!   does not do yet: a Wrayth sound, until sounds are built ([`wrayth`]).
//!   An operation an agent **holds** defends itself and starts nothing until
//!   resumed ([`Control::Hold`](cena_session::operation::Control::Hold), the
//!   author's and LAB's word); the hunt's claim on a room is a fourth, inside
//!   the engine and never public.
//! - **role** is a group member's part, lead, follow or solo
//!   ([`group::Role`]), and a spell's kind in the spell table
//!   ([`spells::Role`]). The combat model's message families have a third
//!   `Role` (add, remove, start, end).
//! - **report** is what a group member tells its leader ([`group::Report`]),
//!   and the `;loot` and `;combat` reports printed for the player
//!   (`crates/cena/src/loot.rs`, `crates/cena/src/combat.rs`).
//! - **category** is the sorter's item type, `gem` or `wand` in a sorted
//!   container look ([`SessionHandle::sort_containers`]), and a trigger's group in the triggers file,
//!   the author's word for what the editor sets (`plan/45` §1 row 3).
//! - **script** is the player's own program, run by a script runner (see
//!   **Scripts**), and three older things: the Lich scripts Hydra ports
//!   from, bigshot's `script` step that the importer turns into a sequence,
//!   and the **scripted game**, the fake server that tests talk to.
//! - **runner** is the desk's command function every Hydra command goes to,
//!   [`Runner`](cena_session::command::claimant::Runner), and a **script
//!   runner**, the process that runs one character's scripts
//!   ([`Runners`](cena_agent::scripts::Runners)). Say "script runner" for the
//!   second.
//! - **relay** is `;to` and `;all`, a line typed on one character and sent
//!   on another ([`relay`](crate::relay)), and the **Lich relay**, the
//!   player's own Lich run against Hydra's connection
//!   ([`cena_agent::lich`]). Say "the Lich relay" for the second.
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
//! | **Snapshot** | an owned, point-in-time copy of the game state, taken at an exact place in the event stream, with the triggers its lines were answered with: [`Snapshot`] | view, handle, ref |
//! | **Event** | something the session saw or did, published to observers: [`Event`] (a frame, a combat chunk, a command sent, a notice) | signal, trigger, hook |
//! | **Line** | a finished line of game text, as the model completed it, published once for every viewer right after the frame that finished it: [`Line`], [`Event::Line`]. What a viewer draws, so it is the line the classifiers and the player log read (`plan/45` §4a); with `;sorter` on, a container look is published as the lines it sorts into, and a character's triggers answer it before it is published ([`Matcher::respond`]). While the player's Lich runs, the lines it showed are published in the game's place, put together the same way (`plan/51` step 3) | display line, story line |
//! | **Shown prompt** | the prompt a viewer draws after the lines it ends: [`Event::Prompt`](cena_session::Event::Prompt), the game's right after its frame, or, while the player's Lich runs, the one Lich passed on, after Lich's lines | the prompt frame, which is the game's and ends a round trip |
//! | **Observer** | reads a snapshot and every numbered event after it ([`SessionObserver::subscribe`], [`ObservedEvent`]); cannot mutate, cannot suppress, confers no authority | |
//! | **Lagged** | what an observer that fell behind is told instead of meeting a silent hole; the recovery is to subscribe again | |
//! | **Notice** | Hydra speaking to the player, not the game: [`Notice`], the port of `Lich::Messaging` | message |
//! | **Creature tag** | three characters of a creature's `exist` id in `targetid.lic`'s alphabet, shown after its name while `.targetid` is on, and taken for the creature by the script's own commands, `tk 7QK` or `tk 7`, or after the game's verb, `kill 7QK`, each sent as `kill #<id>` (`cena_model::targetid`, [`SessionHandle::tag_creatures`]) | a creature's id, which is only numbers |
//! | **Answer** | a [`Notice`] that answers something the player did, a command typed or a room shift-clicked: [`Notice::answer`], shown in the story among the game's lines as Lich's `respond` is. Every other notice is Hydra's own, in the Hydra window: what it is doing, and what went wrong that nobody asked about | reply |
//! | **Player log** | what the player saw and sent, as text, one file per character per day (`plan/25`): written by [`PlayerWriter`](cena_session::PlayerWriter), read back by [`reader`](cena_session::player_log::reader), which `;history` puts on the command line. The command is not `;log`, which is Lich's `log.lic` | the wire log, which is bytes and churns |
//! | **Stretch** | the days between two Eastern midnights where a week or a month begins ([`starts_period`](cena_platform::eastern::starts_period)), named by its first date. The player log cuts its file at each one ([`file_key`](cena_session::player_log::writer::file_key)) so a file never straddles two [archives](cena_session::player_log::archive) | a period, which is a whole month or week: an archive holds one or more stretches |
//! | **Handle** | the only path that sends: [`SessionHandle`] | |
//! | **Command** | a line for the game, through the session's [`CommandQueue`] | |
//! | **Hydra command** | a typed line that starts with the command symbol ([`COMMAND_SYMBOL`], `;` unless the character's settings say otherwise). It is Hydra's whether or not anything knows the word, and the game never sees it ([`commands`](crate::commands)) | |
//! | **Manual input** | what a person types, at a page or the terminal: [`Origin::Manual`]. It goes to the head of the queue, never touches the authority, and never aborts the holder (`plan/12` §4.1) | claimant |
//! | **Injury doll** | the Injuries widget: each hurt part a dot at its place, coloured by its level and numbered with its rank, the wound shown over the scar and each foot folded into its leg (`cena-gui`'s `widget/doll.rs`, `plan/55`). Its *styles* are the Doll, over a picture or a body drawn in code; the Doll plus, with a picture's overlays; Infinite, `gs_studio`'s puppet; and Text, a line a part (`widget/doll_text.rs`) | Wrayth's paperdoll, which it replaces |
//! | **Hydra's own asking** | the one command Hydra sends to learn something rather than to act: `health`, when the model cannot work out whether the nerves' rank is a wound or a scar (`cena-model`'s `character/nerves.rs`, `plan/55` §4a). Sent as [`Origin::Hydra`], once per confusion, after the login and never while dead; the nerve lines in its reply are not shown, the rest is | a sync, which runs at login for what the store says is stale |
//! | **Round trip** | one command and the frames that answer it, until the next prompt or a timeout: [`SessionHandle::send_and_await`], answered with an [`Outcome`]. A timeout means no match in the window, never "it did not happen" | ladder |
//! | **Gate** | a precondition the actor checks against the live state at the moment it writes, not when the behavior asked: [`Gate`], on [`SessionHandle::send_now`] | guard |
//! | **Authority** | the right to send a sequence of commands for a session, held by one claimant at a time as an [`AuthorityToken`]: taken with [`SessionHandle::claim`], given back with [`SessionHandle::release`], and refused, not queued, to a second claimant ([`AuthorityHeld`]) | lock |
//! | **Claimant** | whoever may hold the authority (`plan/12` §4.1): a behavior, later an agent, and never manual input | |
//! | **Preempt** | take the authority from its holder, cooperatively for [`PREEMPT_GRACE`] and then by force: [`SessionHandle::preempt`], [`Preempted`]. Only an explicit stop, or the watchdog, preempts | |
//! | **Refusal** | why the session would not send a command: [`Refusal`] | |
//! | **Agent level** | what an agent may do with one character, set only by its player and kept in the character's settings: [`agent::Level`](cena_session::agent::Level), `off` until raised (`plan/35` §3) | |
//! | **Door** | the only way an agent acts on a session, each act checked against the level: [`agent::Door`](cena_session::agent::Door). The agent crate holds a door and never a [`SessionHandle`] | |
//! | **Operation** | a behavior an agent started, read and steered by its number to its end: a ticket, not a reply ([`Door::perform`](cena_session::agent::Door::perform), [`operation::Report`](cena_session::operation::Report)). Its result keeps apart what the work came to, what it left undone, and whether its authority was released | |
//! | **Request id** | the caller's own id for an act: the same id again, for the same act, is answered as the first time and never done twice ([`agent::Call`](cena_session::agent::Call)) | |
//! | **Takeover** | an agent holding the character: an operation that stopped what ran and holds the authority under the agent's token until it gives it back, the player takes it (`;agent stop`), or it ends ([`agent::TOKEN`](cena_session::agent::TOKEN), [`Door::take_over`](cena_session::agent::Door::take_over)). Nothing resumes by itself afterwards | |
//! | **Denylist** | what an agent may never send to the game at any level: LAB's list, and a `put` that is a drop and the abbreviations of every denied verb ([`agent::refused`](cena_session::agent::refused)). Not a list of what is safe | |
//! | **Approval** | the player's yes to one act an agent asked for above its level: that act, once, on that connection, within [`APPROVAL_LIFETIME`](cena_session::agent::APPROVAL_LIFETIME) ([`SessionHandle::approve_agent`](cena_session::SessionHandle::approve_agent)). It grants nothing further | |
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
//! | **Sequence** | a named list of steps that a routine step may stand for, such as `volley`, with guards read once before its first step, written where bigshot ran a script: [`Profile::sequences`] | script |
//! | **Step** | one line of a routine or sequence: what is sent, and its guards ([`Step`]) | |
//! | **Guard** | a named precondition on a step, from the closed vocabulary Hydra defines ([`Guard`]); with its polarity, a [`Condition`]. A step's guards must all hold, and there is no *or*. A trigger reads the same words (`plan/45` §1 row 2) | gate |
//! | **Held** | a step imported with a guard or shape Hydra does not read yet: kept, with the reason named, and never run ([`Step::held`]) | dropped |
//! | **Import** | a bigshot profile in, a Hydra profile out, with what it could not carry named at the head of the file: [`import()`], [`Import`]. `;trigger import` does the same for a Wrayth settings file's highlights, names and ignores ([`wrayth`]) | |
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
//! | **Board** | one group's shared place: every member's latest report, and what the leader is doing now, found by the leader's name ([`group::Board`], [`group::Boards`]) | a queue of orders: the leader publishes state, not commands (`plan/39` §3) |
//! | **Party** | the group as one member's hunt sees it for one tick, handed to the pure engine as the map is ([`group::Party`]) | the game's group |
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
//! | **Roster** | which account and game each character is on, and whether the player starred it, holding no secret: [`roster`](crate::roster) | |
//! | **Projection** | a game state turned into the vocabulary a frontend renders, reading no clock and changing nothing: [`SessionView::project`] | snapshot |
//! | **Frontend wire** | [`ServerMessage`] and [`ClientMessage`], versioned by [`WIRE_VERSION`] and written down in `crates/cena-ui/WIRE.md`. No model or protocol type crosses it | |
//! | **Despana** | the embedded browser viewer: one loopback listener inside the binary ([`cena_web`], [`WebServer`]) | |
//! | **Pairing token** | the secret a browser presents before any state is sent; held in memory, for this process only | |
//! | **Relay** | a line typed on one character and sent on another, `;to <name> <command>`, or on every running one, `;all <command>`: as if typed there, its command line first. The author's `;queen` from the borg scripts, renamed (`relay.rs`, `plan/47` step 5) | broadcast |
//! | **Hunt panel** | the Hunt widget: what the hunt is doing each turn and why it waits -- *"mana 30%, wants 50%"* -- from the hunt's own [`Status`](cena_behavior::hunt::Status) reports (`plan/47` step 8) | |
//! | **Keybind** | a key, named by its winit code (`Numpad8`, `F13`), and the macro it does on the character whose play window has the keyboard (`plan/47` step 7, `plan/52`). Hydra's defaults are in the code; every character's changes in `keybinds.toml` in the data folder, a character's own in its `.keys.toml` beside its settings | |
//! | **Macro** | what a key does: commands sent as if typed (`\r` between two, `s1.5` a wait), the command input filled, or one of Hydra's actions. The author: *"a macro is a keybind to send one or more commands"* ([`Macro`](cena_gui::Macro), `plan/52` §2) | |
//! | **Window in use** | the widget in a play window last pressed in, the story until one is, marked with a faint amber edge: what the scrolling keys, Find and the tab keys act on (`plan/52` step 4, [`App`](cena_gui::App)) | active window, focused window |
//! | **Macro set** | one of ten sets of keys, 0 to 9, as Wrayth has. Set 0 is always in use and holds Hydra's defaults; a character may use one of 1 to 9 over it, chosen with Alt and a digit, so a key the chosen set binds does its macro and every other key set 0's (`plan/52` step 2) | |
//! | **Play window** | one character's own native window in the GUI: one command input that sends on that character, and its widgets -- the story, the vitals, the hands, the room, Hydra's messages -- in windows inside it. Closing it leaves the character running headless; the hub opens it again ([`App`](cena_gui::App), `plan/47` step 4) | |
//! | **Widget** | one thing a player reads about one character, drawn bare: a bar per vital, each hand its own, the story, the exits. The author: *"individual things, a bar for health is a widget"* (`plan/49` §1) | pane, the M10 word for a window holding several |
//! | **Standalone window** | a window in a play window holding one widget, with a title and a frame (`plan/49` §2) | |
//! | **Custom window** | a window in a play window holding several widgets bare, each in a cell of its own, with one frame for all: the vitals, the loadout and the room come as three (`plan/49` §2, `plan/28` §7d) | container window, `plan/28`'s word for it |
//! | **Tab stack** | several widgets in one cell of a custom window, one showing, a tab for each; a tab not showing counts what its widget said since (*"Hydra 2"*). Made by letting go over the middle of a widget, or one standalone window on another's title bar (`plan/49` §2, Stage A step 5) | tab group, Vellum's separate mechanism |
//! | **Follow** (a widget) | show another running character than its window's -- a party's vitals in one window -- chosen only in the Advanced place: the bottom of the Add-a-widget list, or a widget's right-click menu. A story never follows (`plan/49` §1 rows 3, 7, 8) | |
//! | **Not launched** (the tab) | the hub's third tab: the roster's characters that are not on the table, as cards starred first, each started with a click when its account's password is kept or with the password typed there. Lich's launcher is its reference (`plan/49` Stage C, as the author revised it) | launcher, Lich's separate program; *Launch*, the tab's first name |
//! | **New login** (the tab) | the hub's fourth tab: a login by account, which lists the account's characters to add, star or play; and the kept passwords, each forgotten (`plan/49` Stage C, the author's correction) | the Not launched tab, which holds only the cards |
//! | **Kept password** | one Hydra can read without asking: in the OS keyring, or the account's environment variable. The window keeps one only when its box was ticked, once the login proved it ([`secrets`](crate::secrets)) | saved login, which is the roster's |
//! | **Game state** (the widget) | everything the model holds for a character, as the model prints itself, as a tree with a filter and Copy: for troubleshooting (`plan/49`, the author's addition) | the game's own state, which only the game knows |
//! | **Settings menu** | the one place a player changes a setting, in a window of its own: Hydra's own pages (*Window*, *Keys*), or a character picked by roster name, running or not, and its pages, each one file's settings drawn from a behavior's key table ([`Page`](cena_ui::settings::Page)). A change goes through the writer that behavior's `;` command uses, so the two cannot disagree. The hub's *Settings* button opens it on Hydra's own, a play window's on its character's; every other way in opens it at its place (`plan/50` §6 item 9, §7) | a second editor, which there never is |
//! | **Preset** | a custom window already put together: Hydra's four (Vitals, Vitals row, Loadout, Room) and any a player saved, in one library every character adds from. Placing one places the character's own copy (`plan/49` §2, Stage A step 7) | template |
//! | **Arrange** | the Layout menu's switch that lets a custom window's cells be moved, resized, and dragged out, and lets a standalone window be dropped into one; off, no press rearranges anything (`plan/49` Stage A step 4) | edit mode |
//! | **Hub** | every character at a glance: a card each, start, quit, reconnect, and the merged streams. Two frontends have one: Despana's hub page, and the window's [`Hub`](cena_gui::Hub), in two tabs, Live and Closed (`plan/47`). Requests reach the binary as [`HubRequest`]s through [`HubControl`] | |
//! | **Character page** | one character's own page. No window shows two characters' story text (`plan/29` §5a) | |
//! | **Stream** (a widget) | one of the game's streams in a widget of its own -- thoughts, speech, arrivals, any the character has received -- which the story then leaves out while it is open, so no line is said twice (`plan/49` Stage B step 4) | the merged streams, which are the hub's |
//! | **Merged streams** | thoughts, speech, logons, deaths and announcements across characters, each line once: [`Merger`] | |
//! | **Container look** | the main-stream line `In the box you see a, b and c.`; with `;sorter` on, one that is a list end to end is published as one line per category ([`SessionHandle::sort_containers`]); the model and the player log keep it whole | inventory, which is the `inv` window's feed |
//!
//! # Triggers
//!
//! `plan/45`, M8. What a trigger does is not an **effect**: that word is the
//! model's spell and buff effects.
//!
//! | Term | Means | Not |
//! |---|---|---|
//! | **Trigger** | when a finished line matches, is read as a line event, or a condition becomes true, do something, for everyone or the characters it names: [`Trigger`]. Every trigger is in one file, by name ([`triggers`]) | highlight, which is one thing a trigger can do; event |
//! | **Response** | what a trigger does: a look, a squelch, a substitute, a redirect or a flag. PROPOSED (`plan/45` §3d): the author may choose another word | effect, which is [`Effect`]; action |
//! | **Line event** | what the model reads a finished line as, from a closed vocabulary (`speech`, `attacked`, `incident weapon_reaction`, ...): [`LineEvent`] | event, which is [`Event`] |
//! | **Condition** (a trigger's) | guard words that fire a trigger when they all become true, and again only after staying false for its `rearm`: [`Rule::condition`], [`Edges`]. Each word is a [`Condition`] | alert |
//! | **`only_if`** | guard words that must all hold when a trigger would fire, read against the character and its current target: [`Rule::only_if`] | gate, which is [`Gate`] |
//! | **Trigger's flag** | a name a trigger sets, until cleared or for so many game seconds, or clears; the session publishes each change ([`Event::Flag`]) so a hunt's guard reads it: [`Flags`] | status |
//! | **Look** | a response's colour, background and bold, over the match, a capture group or the line: [`Look`] | style, which is the wire's [`Style`] |
//! | **Paint** | a look, resolved: what one stretch of a published [`Line`] is painted, the best look deciding each of colour, background and bold: [`Paint`]. A name in the room window's players is painted the same way, and only painted ([`Matcher::paint_entry`]) | highlight |
//! | **Master switch** | a category, or one kind of response, turned off for every trigger: the file's `[categories]` and `[responses]` | |
//! | **Attention** | what a trigger calls for beyond the line: a sound, an OS notification, a banner ([`Attention`]); once in its trigger's cooldown, and once for every character that saw the same thing, played by the binary even with no page open | alert, which is one kind: the banner |
//! | **Origin** | where an imported trigger came from, `Wrayth: <file>`: importing that file again replaces what it brought, and it holds a send from a rule the player did not write ([`wrayth`]). Not a command's [`Origin`], who sent it | source |
//! | **Act** | a trigger's line sent as if the player typed it: through the `;` command table first, then to the game as [`Origin::Trigger`], never counted as a person; at most once in the trigger's cooldown and at the character's pace ([`Act`], [`Pace`]) | action, which `plan/12` §6a.3 keeps for a registry not built |
//! | **Approved** | the line a trigger from elsewhere may send (`;trigger approve`): only that line, so a changed one is held again | trusted |
//!
//! # Scripts
//!
//! `plan/46`, M7b: the player's own programs, out of process, in their own
//! language. A script is not a behavior, which is Hydra's own and curated,
//! and not an agent, which has a level and a denylist. The player's own Lich
//! is the other way their scripts run: through the Lich relay (`plan/51`),
//! Lich's scripts in Lich, with Hydra keeping the connection.
//!
//! | Term | Means | Not |
//! |---|---|---|
//! | **Script** | the player's own program, a Lich `.lic` first, run by a script runner in its own language's runtime: started with `;name`, and doing what a Lich script does ([`script`](cena_session::script)). Back from **Retired**: the author, 2026-09-26 (`CLAUDE.md`, Settled decisions) | behavior; the scripted game |
//! | **Script runner** | the process that runs one character's scripts, Hydra's own child: Ruby with Lich's engine (`bridges/ruby`), started on the character's first script and stopped when it leaves the table ([`Runners`](cena_agent::scripts::Runners)) | runner alone, which is also the desk's |
//! | **Local copy** | what a script runner keeps of its character, so a script reads without asking: the agent's projection and what a script needs beyond it, told as `state` events, a chunk's changes before its lines ([`scripts::local`](cena_agent::scripts::local)) | snapshot, which is the session's own |
//! | **Heard line** | a finished line as the game sent it, before `;sorter` and the triggers, published only while a script runner listens: [`Event::Heard`](cena_session::Event::Heard). What a script reads, as Lich's scripts read before its hooks, and what a runner's display hooks answer | line, which a viewer is given |
//! | **Hook** | a script's say in what the player is shown of a line (a **display hook**) or what the player's typing becomes (an **input hook**): Lich's `DownstreamHook` and `UpstreamHook`, answered by its script runner within [`HOOK_DEADLINE`](cena_session::script::HOOK_DEADLINE), past which the line goes as it came. Only the showing and the typing wait on one ([`script::Door::hook_lines`](cena_session::script::Door::hook_lines), [`hook_typing`](cena_session::script::Door::hook_typing)) | trigger, which is the player's own; squelch, which a trigger does |
//! | **Checker** | what reads one of the player's scripts and says which of its lines will not work under Hydra, and why, by Ruby's own parser against the runner's own names: `;scripts check`, [`scripts::checker`](cena_agent::scripts::checker). Run over both script collections, it made `inventory/14-what-runs.md` | census, which measured the collections for `plan/46` from Python |
//! | **Script door** | the only way a script runner acts on a session: listen, send a line as typed, say, start a built-in, and hook what is shown and typed ([`script::Door`](cena_session::script::Door)); `cena-agent` holds it and never the handle | door alone, which is the agent's and checks a level |
//! | **Lich relay** | the player's own Lich for one character, off until `;lich on`: Hydra runs it in pipe mode against a loopback port it holds and keeps the game's connection, hands it the game's bytes and what the player types, sends its lines as [`Origin::Lich`](cena_session::Origin::Lich), and shows what it shows in the game's place ([`cena_agent::lich`], [`script::lich`](cena_session::script::lich), `plan/51`) | relay alone, which is `;to` and `;all`; a script runner, which is Hydra's Ruby and not the player's Lich |
//! | **Lich door** | the only way the player's Lich acts on a session: the game's bytes copied to it, the lines typed for it, its lines sent, and what it showed carried back ([`LichDoor`](cena_session::script::lich::LichDoor)) | script door |
//! | **Lich's symbol** | `;` ([`LICH_SYMBOL`](cena_session::script::lich::LICH_SYMBOL)): a line starting with it is the player's Lich's, never the game's. Hydra's own command symbol also starts as `;` and is tried first, so a player running Lich gives Hydra another, `.` | the command symbol, which is Hydra's |
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
//! | **Adapter** | the per-game implementation, `GameAdapter` | refused: the second game is deferred all-or-nothing, and an abstraction for it is not built ahead of it (`plan/12` §9d) |
//! | **Ladder** | the resend-and-recover protocol around one command | Lich's `fput`. Hydra's counterpart is the **round trip**, and the word is taken by three ladders the code does have |
//!
//! [`AddError::AccountInUse`]: cena_host::AddError::AccountInUse
//! [`AuthorityHeld`]: cena_session::AuthorityHeld
//! [`AuthorityToken`]: cena_session::AuthorityToken
//! [`backoff`]: cena_session::backoff
//! [`batch::Desk`]: cena_behavior::batch::Desk
//! [`batch`]: mod@cena_behavior::batch
//! [`BEHAVIOR_WATCHDOG`]: cena_behavior::BEHAVIOR_WATCHDOG
//! [`chain`]: cena_behavior::hunt::chain
//! [`claim`]: cena_session::claim
//! [`claimant::Desk`]: cena_session::command::claimant::Desk
//! [`Claimed`]: cena_session::command::Claimed
//! [`ClientMessage`]: cena_ui::ClientMessage
//! [`COMMAND_SYMBOL`]: cena_session::command::COMMAND_SYMBOL
//! [`CommandQueue`]: cena_session::CommandQueue
//! [`Condition`]: cena_session::guard::Condition
//! [`Connector`]: cena_session::Connector
//! [`CritTables`]: cena_session::CritTables
//! [`Event::Combat`]: cena_session::Event::Combat
//! [`Event::Line`]: cena_session::Event::Line
//! [`Event::Flag`]: cena_session::Event::Flag
//! [`Attention`]: cena_session::trigger::Attention
//! [`Edges`]: cena_session::trigger::Edges
//! [`Effect`]: cena_session::Effect
//! [`Event`]: cena_session::Event
//! [`Farewell`]: cena_session::Farewell
//! [`Frame::UnknownTag`]: cena_session::Frame::UnknownTag
//! [`Frame`]: cena_session::Frame
//! [`GameState::apply`]: cena_session::GameState::apply
//! [`GameState::invalidate_for_reconnect`]: cena_session::GameState::invalidate_for_reconnect
//! [`GameState`]: cena_session::GameState
//! [`Gate`]: cena_session::Gate
//! [`Fact::Flag`]: cena_session::guard::Fact::Flag
//! [`Flags`]: cena_session::flags::Flags
//! [`Generation`]: cena_session::Generation
//! [`group::Board`]: cena_behavior::group::Board
//! [`group::Boards`]: cena_behavior::group::Boards
//! [`group::Leading`]: cena_behavior::group::Leading
//! [`group::Muster`]: cena_behavior::group::Muster
//! [`group::muster`]: fn@cena_behavior::group::muster
//! [`group::Party`]: cena_behavior::group::Party
//! [`group::Report`]: cena_behavior::group::Report
//! [`group::Role`]: cena_behavior::group::Role
//! [`group::role`]: fn@cena_behavior::group::role
//! [`group::Settings::lost_wait`]: field@cena_behavior::group::Settings::lost_wait
//! [`group::successor`]: fn@cena_behavior::group::successor
//! [`Guard`]: cena_session::guard::Guard
//! [`Heartbeat`]: cena_behavior::Heartbeat
//! [`Host`]: cena_host::Host
//! [`HubControl`]: cena_ui::HubControl
//! [`HubRequest`]: cena_ui::HubRequest
//! [`hunt()`]: fn@cena_behavior::hunt::hunt
//! [`hunt::Desk`]: cena_behavior::hunt::Desk
//! [`Hunt`]: cena_behavior::hunt::Hunt
//! [`import()`]: fn@cena_behavior::hunt::import
//! [`Import`]: cena_behavior::hunt::Import
//! [`Look`]: cena_session::trigger::Look
//! [`Line`]: cena_session::Line
//! [`LineEvent`]: cena_session::trigger::LineEvent
//! [`LONG_LIVED`]: cena_session::LONG_LIVED
//! [`MAX_UNATTENDED_LOSSES`]: cena_session::MAX_UNATTENDED_LOSSES
//! [`Matcher::respond`]: cena_session::trigger::Matcher::respond
//! [`Merger`]: cena_ui::Merger
//! [`movement`]: cena_session::movement
//! [`Notice`]: cena_session::Notice
//! [`Notice::answer`]: cena_session::Notice::answer
//! [`SessionHandle::tag_creatures`]: cena_session::SessionHandle::tag_creatures
//! [`ObservedEvent`]: cena_session::ObservedEvent
//! [`Origin`]: cena_session::Origin
//! [`Origin::Manual`]: cena_session::Origin::Manual
//! [`Origin::Trigger`]: cena_session::Origin::Trigger
//! [`Origin::Hydra`]: cena_session::Origin::Hydra
//! [`Act`]: cena_session::trigger::Act
//! [`Pace`]: cena_session::trigger::Pace
//! [`Outcome`]: cena_session::Outcome
//! [`PREEMPT_GRACE`]: cena_session::PREEMPT_GRACE
//! [`Preempted`]: cena_session::Preempted
//! [`Profile::routines`]: cena_behavior::hunt::Profile::routines
//! [`Profile::sequences`]: cena_behavior::hunt::Profile::sequences
//! [`Profile`]: cena_behavior::hunt::Profile
//! [`Refusal`]: cena_session::Refusal
//! [`Rooms::rally`]: field@cena_behavior::hunt::profile::Rooms::rally
//! [`Runs`]: cena_session::Runs
//! [`Paint`]: cena_session::trigger::Paint
//! [`Matcher::paint_entry`]: cena_session::trigger::Matcher::paint_entry
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
//! [`SessionHandle::sort_containers`]: cena_session::SessionHandle::sort_containers
//! [`SessionId`]: cena_session::SessionId
//! [`SessionObserver::subscribe`]: cena_session::SessionObserver::subscribe
//! [`SessionView::project`]: cena_ui::SessionView::project
//! [`Snapshot`]: cena_session::Snapshot
//! [`spells::Role`]: cena_session::spells::Role
//! [`State::behaviors_may_run`]: cena_session::State::behaviors_may_run
//! [`State::Ready`]: cena_session::State::Ready
//! [`State::Syncing`]: cena_session::State::Syncing
//! [`State`]: cena_session::State
//! [`Step::held`]: field@cena_behavior::hunt::Step::held
//! [`Step`]: cena_behavior::hunt::Step
//! [`stop_all`]: cena_host::stop_all
//! [`Style`]: cena_session::Style
//! [`stream_windows`]: cena_session::stream_windows
//! [`SupervisedSession`]: cena_session::SupervisedSession
//! [`sync()`]: fn@cena_behavior::sync::sync
//! [`travel()`]: fn@cena_behavior::travel::travel
//! [`travel::Desk`]: cena_behavior::travel::Desk
//! [`Trip::tick`]: cena_behavior::travel::Trip::tick
//! [`Trigger`]: cena_session::trigger::Trigger
//! [`Rule::condition`]: cena_session::trigger::Rule::condition
//! [`Rule::only_if`]: cena_session::trigger::Rule::only_if
//! [`triggers`]: mod@cena_behavior::triggers
//! [`wrayth`]: mod@cena_behavior::triggers::wrayth
//! [`Trip`]: cena_behavior::travel::Trip
//! [`UnknownTag`]: cena_session::UnknownTag
//! [`watch`]: cena_behavior::watch
//! [`WebServer`]: cena_web::WebServer
//! [`WIRE_VERSION`]: cena_ui::WIRE_VERSION
