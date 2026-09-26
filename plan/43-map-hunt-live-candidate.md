# Map-driven Hunt: integration candidate and supervised test

Prepared 2026-09-25. Local branch: `feat/despana-native-hunt-setup`.
Base: `924a080`, treating PRs #13 and #14 as merged. This does not merge them,
publish another PR, or establish Nisugi's acceptance of the proposed extensions.
Contract proposal: [42-map-hunt-setup-proposal.md](42-map-hunt-setup-proposal.md).

## What is ready

An opt-in Despana character-page launcher opens a session-bound setup tab. It
uses our map catalogue and saved hunting corrections, but writes a **native TOML
profile** through Hunt's existing inheritance chain. Save never starts anything.

The candidate adds explicit allowed hunting rooms and optional field rest,
separate from existing town rest. It rejects stale map fingerprints, empty
selections, invalid start/rest IDs and conflicting higher-priority settings.
Existing files are never overwritten. Unrelated settings continue to inherit.
The original JSON setup prototype is not promoted into a second native store.

The current source also provides a shared town/field rest destination picker:
search this regional snapshot by name or room ID, choose the Landing TSC
suggestion, or pick on another local map and return to the hunt camera. Rest
commands and the hunt draft survive that map excursion. The suggestion is not
a claim of safety or nearest reachable town. Route previews use eligible
recorded links only and issue no commands. Favorites/recent destinations are
browser-local, scoped to character, instance and map fingerprint; they do not
import or replace native inherited rest settings. This UI follow-up is not yet
included in the previously archived bundle described below.

The source UI now uses **Save setup** as the normal action: native preview and
validation happen internally, then the exact validated configuration is saved.
Missing required fields are shown inline and beside Save. Changes during
validation prevent saving the older draft. Generated TOML/manual preview and
readback are optional under **Advanced configuration**. Known rest room IDs
accept Enter directly; ambiguous names and unknown IDs require correction.
No draft-only save or existing-profile overwrite has been added.

The shared SVG hit test now accounts for actual SVG scaling/letterboxing. The
browser regression reproduced TSC selecting a neighboring room before this fix;
the fixed test selects TSC precisely. An active picker also takes priority over
transition menus, without changing ordinary browsing behavior.

## Build prepared on Atari's machine

Linux x86-64 development build, debug symbols stripped; **not a Windows build**.
It embeds the explorer assets. The accompanying map matches those assets:

`d5e8ac60659b09dac49f0bf75e9cf2e700b22d8f716cc1b12c0ef407ba22ceff`

Bundle directory:

`/mnt/FastStorage/SSD Games - Non Steam/cena-atlas-integration/target/hydra-native-hunt-20260925-9TogG2`

The bundle contains `hydra`, `hydra-setup-preview`, `gs.map`, the proposal, this
checklist, and separate offline/live launchers. It contains **no credentials,
personal character settings, personal correction files, or live logs**. Keep it
on mounted bulk storage. The Linux launchers refuse the root filesystem.

### Safe rehearsal — no game connection

From the bundle directory:

```bash
bash preview-offline.sh
```

Open the private localhost URL it prints. The page explicitly says **OFFLINE
FIXTURE**. Choose a hunting group, pick start/field/town rooms, enter fixture
commands, acknowledge the selection, then Preview, Save and Reload. Files go to
a new isolated `offline-fixtures/run-*` directory. Ctrl+C closes the server.
Do not share the pairing URL. These fixture commands must not be copied into a
real profile.

### Live session — operator present, not run by the agent

From the bundle directory, only while supervising:

```bash
bash live-test.sh --operator-present YourCharacterName
```

This **logs into the real game** using Hydra's normal credential flow. The
isolated roster may ask for the account. Do not put credentials in commands,
files in this bundle, or chat. OS-keyring access remains the normal native path.

The launcher pins the bundled map and creates `live-test-data`, `live-test-logs`,
and `scratch` beside it. No existing character/Hunt/loot configuration is copied.
`--no-record` disables combat/loot database recording, **not wire/player logs**.
Treat the generated data/logs as private; do not zip them back to Nisugi.

To explicitly read Atari's reviewed boundary files, prefix that launch with:

```bash
HUNT_REVIEW_CORRECTIONS_DIR='/mnt/FastStorage/SSD Games - Non Steam/hydra-hunting-corrections' bash live-test.sh --operator-present YourCharacterName
```

The live host exposes those corrections read-only. Stale ones are withheld and
flagged for review. The boundary editor remains a separate developer workflow.

## Acceptance sequence

1. Begin in a safe known room, with no other automation controlling the character.
   Confirm the live minimap follows that character. Do not treat an unknown or
   mismatched-map location as a successful connection.
2. Open **Configure hunt (experimental)** from that character's minimap. Confirm
   the correct native character/instance and no OFFLINE FIXTURE banner. The
   process must have been launched with `--hunt-setup`; otherwise setup is refused.
3. Choose a small, known hunt. Check its actual membership and creatures. Generated
   boundaries are not human-approved; do not start an unfamiliar high-level hunt
   merely because the form accepts it. Pick a start in the hunt and town rest.
   Enable field rest only after checking its location and retreat paths.
4. Enter safe, character-appropriate attack steps and distinct field/town native
   commands. The initial form shares one attack routine between checked creature
   names. It does not provide herb-healing, banking or Lich script execution.
   Configure necessary recovery actions before testing them; do not use invented
   commands. A separately configured native loot profile is needed for selling.
5. Preview and inspect TOML, then Save and Reload. Confirm the allowed list is
   membership (`rooms.allowed`), not exclusion boundaries. Confirm distinct rest
   commands and no game output caused by saving. Check the new file under
   `live-test-data/hunt/profiles/` and run `;hunt check <name>` before starting.
6. **Separate deliberate action:** issue `;hunt <name>` only when ready. Verify
   wandering stays inside the chosen rooms, and remember travel to start/rest is
   intentionally not confined to them. Have `;hunt stop` ready throughout.
7. Observe routine resource recovery: healthy and unencumbered may use field;
   unknown eligibility must choose town. Do not deliberately injure a character
   to test escalation—the offline driver regression already covers that path.
8. Verify stop promptly ends the hunt. Record the profile, native map hash,
   observed route/rest behavior, and any private log timestamps for diagnosis.
   Closing the browser alone is not the stop mechanism for a native session.

## Evidence gathered (offline, Linux)

- Full workspace: **2,616 passed, 0 failed, 5 ignored**, across 236 test results.
- Workspace clippy, all targets, warnings denied: passed. Formatting: passed.
- JavaScript contract/model suites: 10 test files passed.
- Browser: native setup; boundary editor; visible-group selection; correction
  autosave/reload; minimap/character-bound setup launch; and explorer checks passed. The native setup test also passed
  against the **packaged offline executable and its bundled map**.
- Native observer fixture: host-supplied identity, stale generation refusal,
  preview/save and zero additional game sends during configuration.
- Native driver fixture: injury on an intermediate room cancels the remaining
  field-rest leg, uses the town route, and sends no field-rest command.
- Desk tests: stale/unverified map-pinned profiles refuse before starting.
- Existing saved Darkstone file remained unchanged, SHA-256:
  `e61f500735ef0868b870e776cf9faf753a6b86cb7888759298f1c2902663ee36`.

Logs and screenshots are under the checkout's `target/` directory, prefixed
`native-setup-`, plus `target/atlas-evidence/native-hunt-setup.png`.
The retained `crates/cena/examples/hunt_setup_preview.rs` is an intentional
offline acceptance harness, not a live probe or temporary production command.

## Remaining limits

No live login, movement, attacks, healing, selling, or live stop acceptance has
been performed. Windows compilation/runtime was not tested for this candidate.
Nisugi still needs to approve the allowed-room/field-rest contract. The original
map/editor delivery is separable from these proposed behavior changes.

Wandering deliberately excludes scripted/routine/pass-through crossings. A hunt
depending on those links can be partially unreachable; test such cases before
using them. Profile fingerprints require review after replacing the map. The
form's initial recovery thresholds are fixed and shown; richer per-creature
attack editing and configurable thresholds are follow-ups. Unsaved drafts do
not survive navigation to a different regional page. Existing profiles can be
read back but are not overwritten by this first form.

For another OS, build this branch from source with `cargo build -p cena`; set
`CENA_MAP`, `CENA_DATA_DIR` and `CENA_LOG_DIR` explicitly to test storage, then use
`--character <name> --web --hunt-setup --no-record` under operator supervision.
Do not call that platform verified until its tests and live acceptance run.
