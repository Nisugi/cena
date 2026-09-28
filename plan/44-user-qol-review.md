# User quality-of-life review

Reviewed 2026-09-26 against commit `e7e573d`. Cena is the repository name; Hydra is the product name used by the application.

## Assessment

The main obstacle to testing is an incomplete user workflow around implemented behaviors. A player can reach a command that runs a feature before they can discover its prerequisites, create its configuration, inspect the effective settings, or understand how to stop it. This affects healing most directly, but appears across hunting, buffing, configuration, and everyday command entry.

The next pass should make **discover → configure → check → run → observe → stop → adjust** work for each exposed behavior. More explanatory text alone will not fix missing editors, inaccessible setup, or settings that can be discarded during an edit.

This is a source-based usability review, supported by existing offline tests. It is not a live game session or a visual browser/accessibility audit. Findings below distinguish implemented behavior from recommendations; proposed commands do not exist unless explicitly listed as current functionality.

## Priorities

P1 means a testing blocker or a settings-integrity problem. P2 means substantial friction or ambiguity. P3 means useful polish after the core workflows work.

| ID | Priority | Finding |
| --- | --- | --- |
| Q01 | P1 | Healing has no implemented setup command or settings screen |
| Q02 | P1 | Hunt setup is advertised even when its backend is disabled |
| Q03 | P1 | Hunt setup cannot edit an existing profile |
| Q04 | P1 | No central help/settings entry point connects the features |
| Q05 | P1 | Keep/spellcaster edits can replace malformed configuration with defaults |
| Q06 | P2 | Hunt setup requires command-language knowledge and hides adjustable thresholds |
| Q07 | P2 | Saving a hunt does not explain the next step |
| Q08 | P2 | Hunt checking does not establish full readiness |
| Q09 | P2 | Settings scope, inheritance, and storage are difficult to inspect |
| Q10 | P2 | Waggle and loot configuration depend on files or migration knowledge |
| Q11 | P2 | Running behaviors lack a consistent status and stop surface |
| Q12 | P2 | Help-like input can be interpreted as an action |
| Q13 | P2 | Startup and map failures give incomplete recovery instructions |
| Q14 | P2 | Command entry lacks recall and feature discovery |
| Q15 | P2 | Preference persistence is inconsistent and unexplained |
| Q16 | P3 | Reports and advanced features are hard to discover |

## Detailed findings

### Q01 — Healing setup is a dead end

**Current journey:** a character without a heal profile types `;heal` and receives `Hunt: no heal profile: write one naming the herb container.` The response does not supply a path, a template, a configuration command, or a link. The same gap appears when an existing profile names a container that cannot be found.

The parser supports healing flags and `stock`/`fill`, but no `set`, `setup`, or `show`. The heal-profile module's comment says a file can be written by `;heal set`; that is documentation drift, not an available feature.

**Evidence:** [heal parser](../crates/cena-behavior/src/hunt/command.rs), function `heal`; [missing-profile response](../crates/cena-behavior/src/hunt/desk.rs), `herbs`/`heal_profile`; [profile and path](../crates/cena-behavior/src/heal/profile.rs); [missing-container responses](../crates/cena-behavior/src/hunt/drive/errands.rs).

**Recommendation:** add a Heal settings page and a small equivalent command surface. Proposed commands: `;heal setup`, `;heal show`, and `;heal set container herb pouch`. Setup should explain the required container, offer observed containers when available, expose optional switches with defaults, and save for the displayed character/instance. A missing profile should open or directly identify this path.

**Acceptance:** starting from no file, a tester can configure healing using only the application, inspect the saved value, and run healing. An unknown container gives a corrective action. Changing settings does not execute healing.

### Q02 — The hunt setup button overpromises availability

The browser always creates “Configure hunt (experimental).” The native host only installs its handler when launched with `--hunt-setup` and a valid map. Without it, the configuration endpoint responds `Native setup is not enabled for this session`, without naming the enabling flag.

There is a second failure path: clicking the button returns silently when the minimap explorer link is hidden. That link is hidden while location is unavailable, the character is disconnected, or map loading/matching fails. Thus access to settings also depends on successful location rendering.

**Evidence:** [button and click handler](../crates/cena-web/assets/app.js), `native-hunt-launch`; [feature flag](../crates/cena/src/frontend.rs), `Frontend::open`/`attach`; [endpoint refusal](../crates/cena-web/src/hunt_setup.rs); [minimap state](../crates/cena-web/assets/atlas/minimap.mjs).

**Recommendation:** project setup availability and its reason into the UI. Put configuration under a stable Settings/Behaviors entry point. If the feature remains opt-in, explain `--hunt-setup` and the restart requirement. Do not require a resolved current room merely to open a configuration workflow when a valid map is available.

**Acceptance:** with and without the flag, map, or known location, the user sees either a working setup route or a specific recovery instruction. No apparently usable button silently does nothing.

### Q03 — Hunt setup supports creation, not iterative editing

The native API offers preview, save-new, and load. Saving uses `write_new` and refuses an existing profile. “Read saved configuration” displays TOML but explicitly does not repopulate the draft. A user who saves a hunt and then wants to change an attack or rest room cannot complete that edit in the form.

**Evidence:** [setup API](../crates/cena-behavior/src/hunt/setup.rs), `Operation`/`configure`; [setup UI](../crates/cena-web/assets/atlas/native-hunt-setup.mjs), `act`.

**Recommendation:** provide a profile list with Edit and Duplicate. Load existing supported fields into the form; preserve unmanaged settings. Add a conflict-aware update path so a save does not silently replace external edits. Identify unsupported fields and allow advanced inspection rather than dropping them.

**Acceptance:** create → reopen → change one field → save → reopen preserves both the changed value and unrelated settings. A concurrent edit is reported. Duplicate creates a separately named profile.

### Q04 — There is no front door for help or settings

The main page has a game-command field and map links, but no general Help, Settings, or behavior catalogue. The command router has individual command families but no global `help` or `settings` family. A new user must already know feature names or read development files.

**Evidence:** [page structure](../crates/cena-web/assets/index.html), [dynamic UI](../crates/cena-web/assets/app.js), [command routes](../crates/cena/src/commands.rs).

**Recommendation:** introduce visible Help and Settings entries plus `;help`. List behaviors with a plain description, setup state, configuration action, help, and stop instructions. Unknown commands should offer help. Every exposed behavior needs a consistent help/setup/show/check vocabulary where applicable.

**Acceptance:** from a fresh character page, a tester can find healing, hunting, spell settings, and travel instructions without guessing a command or opening source files.

### Q05 — Editing can discard an unreadable spellcaster or keep profile

`keep_edit` and `sc_edit` turn both file-read and parse failures into a default profile. A valid edit then writes that default-plus-change over the existing file. A malformed hand-edited TOML file can therefore lose unrelated settings when the user tries a normal command. This is especially relevant while manual files are a primary setup method.

**Evidence:** [configuration editors](../crates/cena/src/hunt.rs), `keep_edit` and `sc_edit`: `read_to_string(...).ok()`, parse `.ok()`, `unwrap_or_default()`, then `std::fs::write`.

**Recommendation:** distinguish missing, unreadable, and malformed files. Only missing files should start from defaults. Preserve an invalid file and report its exact location and parse error; use atomic saves and an explicit recovery/reset operation.

**Acceptance:** a malformed existing file remains byte-for-byte unchanged after an attempted edit. The error names the file and repair route. A genuinely missing profile can still be created.

### Q06 — The hunt form leaves important configuration knowledge implicit

The attack textarea asks for “Explicit native attack steps” without a nearby example or supported-command reference. Town/field command inputs expect users to understand native command syntax versus imported Lich scripts. Recovery thresholds are explained in prose, but the form fixes its return thresholds to experience 80 and mana 90; it does not expose controls for changing them.

**Evidence:** [form fields and fixed `until`](../crates/cena-web/assets/atlas/native-hunt-setup.mjs); [configuration validation](../crates/cena-behavior/src/hunt/setup.rs), `lines`/`candidate`.

**Recommendation:** add contextual examples verified against the parser, a supported-step reference, and explicit threshold controls with units and explanations. Separate common choices from advanced commands. Show unsupported input next to the relevant field.

**Acceptance:** a user can build a simple routine and adjust recovery without learning the TOML schema. Invalid steps are caught before running and explained in terms of what to enter instead.

### Q07 — Saving does not guide the user into checking and testing

The success message says the profile was saved and nothing started. That correctly separates configuration from execution, but supplies neither the command to check it nor the command to run it. Meanwhile, `;hunt list` with no profiles only points to importing bigshot YAML, not to native setup.

**Evidence:** [save response](../crates/cena-web/assets/atlas/native-hunt-setup.mjs), `act`; [empty list](../crates/cena/src/hunt.rs), `list`; [hunt usage](../crates/cena-behavior/src/hunt/command.rs), `USAGE`.

**Recommendation:** after saving, show the profile name, character context, `;hunt check <name>`, `;hunt <name>`, and `;hunt stop`, rendered with the configured command prefix. Offer a separate explicit Start action only after readiness is clear. Link empty-profile responses to creation as well as import.

**Acceptance:** save leaves the character idle and gives a complete, copyable next step. A fresh user is not required to have a legacy profile.

### Q08 — “Reads cleanly” is narrower than “ready to test”

`hunt check` loads the inheritance chain, reports counts/room IDs, held steps and unwritten sequences. Its loot report only checks whether a file exists. It does not call the map-pinning validation used at run time, parse the loot file, or validate the heal/waggle prerequisites used by the selected workflow. A clean result is therefore not an end-to-end readiness check.

**Evidence:** [check implementation](../crates/cena/src/hunt.rs), `check`; [run-time checks](../crates/cena-behavior/src/hunt/desk.rs), `refuses_map`/`loot_profile`/`heal_profile`.

**Recommendation:** report separate configuration, map, dependency, and observed-character readiness. Distinguish blockers, optional fallbacks, and things that cannot be checked until the game provides state. Show effective values, not just counts.

**Acceptance:** a stale pinned map or malformed dependency cannot receive an unqualified ready result. The check sends no game commands. Optional healing is not presented as mandatory for every hunt.

### Q09 — Users cannot readily see which settings apply or why

Hunt settings resolve character → named profile → global → built-in defaults. Lists replace rather than append. Other preferences live in per-character JSON, separate TOML files, travel storage, or browser/session memory. `hunt check` lists source files, but not the origin of each effective value. Setup can reject character overrides with no field-specific repair route.

The default data directory is relative `data`; launching from another working directory can therefore select a different store unless `CENA_DATA_DIR` is set.

**Evidence:** [inheritance and paths](../crates/cena-behavior/src/hunt/chain.rs); [shared settings](../crates/cena-session/src/settings_store.rs); [data directory](../crates/cena-session/src/character_store.rs), `data_dir`; [override refusal](../crates/cena-behavior/src/hunt/setup.rs).

**Recommendation:** one settings UI can sit over the existing stores; this does not require a storage migration. Display character, instance, scope, effective value, source, and persistence. Offer “Use inherited value” separately from assigning a value. Show the resolved absolute data path and support a stable configured location.

**Acceptance:** a tester can explain why a setting has its current value, change the intended scope, and confirm that another character was not accidentally changed. List replacement is explicit.

### Q10 — Related behaviors repeat the file-first setup problem

`waggle` asks the player to write a profile containing `cast_list`, without giving the location or a setup action. Its loader also collapses read/parse failures into the missing-profile response. Loot configuration is exposed through `hunt import-loot`, which serves migrating users but leaves new users without an in-app editor. Spellcaster offers edits but lacks an equivalent general settings display in its command grammar. Keep has a useful `list` path and actionable `keep add` prompt that other behaviors can emulate.

**Evidence:** [waggle/keep handlers](../crates/cena-behavior/src/hunt/desk.rs); [waggle schema](../crates/cena-behavior/src/waggle.rs); [loot schema](../crates/cena-behavior/src/loot/profile.rs); [spellcaster edits](../crates/cena-behavior/src/spellcaster.rs); [keep edits](../crates/cena-behavior/src/keep.rs).

**Recommendation:** add per-character Waggle, Loot, Keep, and Spellcaster settings views. Keep import as an option alongside creation. Expose current values and distinguish absent, invalid, and empty profiles.

**Acceptance:** configure a buff list or loot containers without owning Lich files. Inspect aliases/switches without opening TOML. Parse errors are not reported as missing configuration.

### Q11 — Behavior control reflects internal grouping

Heal, stocking, waggle, spellcasting, and keep run through the hunt desk. `;hunt stop` controls that desk; `;heal stop` is not supported by the heal parser. Starting another behavior through the same desk cancels the previous one. A player may not expect “keep my spells up” and “hunt” to replace each other.

The browser has session lifecycle/group information and shutdown controls, but no dedicated current-behavior panel with its profile, phase, waiting reason, and stop action. Closing a browser viewer does not own the native session lifetime.

**Evidence:** [desk lifecycle](../crates/cena-behavior/src/hunt/desk.rs), `start`/`stop`/`keep_spells`; [command grammar](../crates/cena-behavior/src/hunt/command.rs); [page](../crates/cena-web/assets/index.html); [viewer/session lifecycle](../crates/cena/src/frontend.rs).

**Recommendation:** show active automation and a character-scoped Stop action. Explain replacement before users unknowingly interrupt an active behavior; add natural stop aliases. Make viewer closure versus stopping automation understandable in help. Group controls must state whether they affect one member or the group.

**Acceptance:** the player can identify what is running, why it is waiting, and how to stop it without scrolling the Story. Stopping automation does not require quitting the character.

### Q12 — Asking for help is not consistently read-only

`hunt help` and `hunt setup` parse as profile runs because these words are not reserved. `go2 help` parses as travel to a destination named “help.” `heal help` produces usage text but no setup guidance. Some other command families have their own help handling, so expectations do not transfer.

**Evidence:** [hunt parser](../crates/cena-behavior/src/hunt/command.rs), `RESERVED`/`parse`; [travel parser](../crates/cena-behavior/src/travel/command.rs); [batch help](../crates/cena/src/batch.rs).

**Recommendation:** explicitly handle help for every family and keep help read-only. Handle existing profiles/destinations with colliding names through an explicit run/go form or a documented compatibility path.

**Acceptance:** `;help`, `;hunt help`, `;heal help`, and `;go2 help` show useful instructions and send no game commands.

### Q13 — Startup and map recovery still require developer knowledge

There is no top-level `--help` branch in `main`; argument selection scans for character names and otherwise enters the interactive startup flow. Hunt-related actions, including standalone heal/sc/keep/waggle, require the map-backed desk. Without a map they share “there is no hunting; set the map and start Hydra again,” even when the user asked to heal.

Minimap errors do name `CENA_MAP`, but map-version recovery can tell the user to rebuild the explorer without giving an accessible procedure.

**Evidence:** [startup](../crates/cena/src/main.rs); [argument scanning](../crates/cena/src/play.rs), `characters_in`; [map-backed dispatch](../crates/cena/src/hunt.rs), `open`; [map loader](../crates/cena/src/map_context.rs); [minimap errors](../crates/cena-web/assets/atlas/minimap.mjs).

**Recommendation:** implement offline `--help` and startup validation with supported flags and resolved paths. Add an environment/readiness page. Give feature-specific recovery instructions. Review whether non-travel actions need to be gated on the map at all.

**Acceptance:** help exits before credential prompts or connections. A missing map or mismatched explorer identifies the expected input and exact recovery route.

### Q14 — Repeated testing is unnecessarily laborious

The command field clears after accepted submission. The main application has no command-history keyboard handler, history list, or command completion. A tester adjusting and repeating a command must retype it or use an external clipboard. Its placeholder also does not advertise Hydra's command prefix or help.

**Evidence:** [input markup](../crates/cena-web/assets/index.html); [submit handler](../crates/cena-web/assets/app.js).

**Recommendation:** add per-character Up/Down recall that preserves the current draft, a small searchable recent-command list, and completion/help for Hydra commands. Restoring a command should never submit it automatically.

**Acceptance:** recall/edit/resubmit works by keyboard; changing characters does not insert another character's command. Reconnect does not replay previous input.

### Q15 — Users cannot predict which preferences survive

Sorter settings are explicitly session-only, but the success response just says sorting is on/off. Open stream-window choices are held in a page-local Set. In contrast, spell configuration persists to disk and some map preferences use browser storage. These differences are not clearly surfaced to the user.

**Evidence:** [sorter lifecycle and messages](../crates/cena/src/sorter.rs); [stream window state](../crates/cena-web/assets/app.js); [atlas preferences](../crates/cena-web/assets/atlas/preferences.mjs).

**Recommendation:** decide and label persistence by scope: character, application, browser, or this session. Save ordinary preferences when appropriate. Distinguish “applied” from “saved,” including save failures and when a running behavior reads a changed value.

**Acceptance:** refresh/reconnect/restart has documented, tested outcomes. A session-only change says so before the user relies on persistence.

### Q16 — Existing reporting and utility features deserve navigation

Combat/loot reports, travel bookmarks, sorter, spell aliases, batches, and group hunting have command implementations but little discoverability from the main page. `;loot` refers to reports, while loot behavior configuration lives under hunt import/profile files. A user can reasonably look in the wrong place.

**Evidence:** [report commands](../crates/cena/src/loot.rs), [combat](../crates/cena/src/combat.rs), [travel](../crates/cena-behavior/src/travel/command.rs), [routing](../crates/cena/src/commands.rs).

**Recommendation:** label navigation by user purpose: Reports → Loot/Combat; Settings → Loot collection; Travel → Saved destinations. Explain recording state near reports and retain command shortcuts for experienced users.

**Acceptance:** a user can find both loot reports and loot behavior settings and tell them apart without experimenting with command names.

## What already provides a good foundation

- Native hunt setup binds configuration to character/session identity, validates map identity, separates saving from running, and refuses overwrites.
- The setup form includes inline validation, unsaved-change messaging, named rest selection, and optional advanced TOML inspection.
- Hunt inheritance and check/list commands already provide useful infrastructure for a settings inspector.
- The browser distinguishes connection state, unknown values, and uncertain command delivery; its tests verify that reconnect does not replay commands.
- Travel has useful route preview and bookmark commands. Keep's missing-settings response already gives an actionable command.
- The HTML includes a skip link, labels, and status regions. These are positive foundations, not a substitute for a keyboard/screen-reader audit.

These should be extended rather than replaced wholesale.

## Suggested implementation order

### Pass 1: unblock testing

1. Add global and per-feature help, with a visible browser entry point and read-only behavior.
2. Implement healing setup/show/edit and point every heal setup failure to it.
3. Make hunt setup availability accurate; explain flags/map prerequisites and remove silent clicks.
4. Add saved-hunt editing and clear post-save check/run/stop instructions.
5. Fix malformed-profile preservation before expanding the existing edit paths.

These are the minimum changes that let a tester reliably configure, try, and adjust the two behaviors that prompted this review.

### Pass 2: make settings and automation understandable

Add a shared settings shell over existing stores, effective-value/source inspection, broader readiness checks, remaining behavior editors, and active automation status/stop controls. Expose routine examples and recovery controls. Show settings persistence and application timing.

### Pass 3: reduce daily friction

Add command recall/completion, persistent viewer preferences, report/bookmark navigation, and an offline startup guide. Then perform real browser keyboard, screen-reader, narrow-window, focus, and long-session reviews.

## Current workarounds for testing

These describe current implementation, not the proposed interfaces above.

**Healing:** manually create `<data>/hunt/heal/<instance>_<character>.toml`. The heal filename helper lowercases the combined identity and retains only ASCII letters/digits, `_`, and `-`; use the exact native instance and character identity, not an assumed account/game label. `<data>` is `CENA_DATA_DIR`, or `data` relative to the launch working directory.

Minimal contents:

```toml
container = "herb pouch"
```

Replace the value with the appropriate container name. Omitted fields use the profile's defaults. The current standalone action also requires the configured map-backed hunt desk. `;heal` runs it; `;hunt stop` stops the desk. This creates configuration only; it does not establish that the container/herbs are present or that healing has succeeded.

**Hunting:** the current native setup route requires `--web --hunt-setup`, a valid `CENA_MAP`, and matching explorer data. Use the character page's “Configure hunt (experimental)” button once the minimap has a usable location. Save a new profile, then use `;hunt check <name>` and, separately, `;hunt <name>`. Stop with `;hunt stop`. Reusing the form to overwrite that profile is not supported. `hunt check` has the limitations in Q08.

These workarounds are evidence of what the product should make accessible, not an adequate long-term setup experience.

## User-journey acceptance checklist

Use isolated fixture data first, then a supervised live acceptance session for game outcomes.

| Journey | Required outcome |
| --- | --- |
| Fresh character, no configuration | Help and setup are discoverable from the main page |
| Ask for help | No behavior starts and no game command is sent |
| First heal | Configure container, inspect settings, run, and understand result |
| Wrong/missing heal container | Specific repair route, retaining the existing configuration |
| First hunt without legacy files | Create, validate, save, and find explicit run/stop actions |
| Adjust saved hunt | Reopen editable values, save a change, preserve unrelated fields |
| Inherited hunt setting | See effective value and source; change the intended scope |
| Setup disabled or map missing | Explain cause and exact recovery; no silent buttons |
| Invalid existing TOML | Preserve file and identify error; do not replace with defaults |
| Automation already running | Show current action and explain replacement/stop scope |
| Two characters/instances | Settings and command history remain correctly scoped |
| Refresh/reconnect/restart | Preferences persist as labelled; no command replay |
| Repeated test command | Recall and edit by keyboard without accidental submission |
| Setup with keyboard only | Reach fields/errors/actions and retain useful focus |

## Verification performed

- `cargo test -p cena-behavior --lib hunt::command::tests`: 5 passed.
- `cargo test -p cena-behavior --test hunt_setup --test heal_plan`: 15 passed (6 setup, 9 heal-planner tests).
- `node crates/cena-web/assets/tests/session.test.mjs`: 36 passed.
- Traced command routing, native host setup registration, browser entry points, profile loading/editing, inheritance, and relevant recovery messages directly in source.

No game login was performed. No production behavior or user configuration was changed. A real-browser visual smoke was not run; Playwright was not available through local Node module resolution. The tests above establish selected existing implementation behavior, not completion of the proposed user journeys. Findings derived from control flow, including malformed-file replacement and disabled-setup visibility, were not reproduced against live character data.
