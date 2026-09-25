# Map-driven Hunt setup — proposal and integration candidate

Status: proposed for Nisugi's review, not an accepted upstream contract.
Baseline: PRs #13 and #14 treated as merged. Existing live behavior remains
unchanged unless a player explicitly runs a profile using the new settings.

## Why

The map already answers where a hunt is. Players should select that destination,
inspect the rooms and creatures, pick their start and recovery locations, and
save a native Hunt profile without copying room numbers or writing scripts.
Geographic ownership, display plates, hunting membership, and recorded creature
habitat remain independent. A human-edited boundary is not new spawn evidence.

## Player flow

1. Open setup from a particular character's Despana page. Character and instance
   come from that native session, not a free-text name or the currently active tab.
2. Choose a named destination in the explorer. Show effective saved membership,
   its review status, source-map fingerprint, creatures, and reference levels.
   Do not claim that level alone makes a hunt safe or suitable for a profession.
3. Pick a starting room inside the selection. Choose town rest, and optionally
   field rest, on the map. Selection never moves the character.
4. Supply explicit native attack steps for selected creatures and separate rest
   command lists. Do not silently choose attacks, enable a catch-all target, or
   turn arbitrary script names into commands. Native sequences are not Lich scripts.
5. Preview the effective native TOML, including inherited settings and any
   conflicts. Refuse stale map identities, unknown rooms, an empty selection,
   or a starting room outside the selection. Generated membership requires
   explicit acknowledgement; stale corrections require boundary review first.
6. Save a new named profile, reload it through the same native loader Hunt uses,
   and show exactly what was saved. No game commands, authority claim, movement,
   selling, or hunting occur when previewing or saving.
7. Initial live acceptance uses an operator-issued `;hunt <profile>` and existing
   stop controls. A future Start button must be a separate deliberate action,
   generation-pinned through the existing command path, never part of Save.

## Small native extensions

- `rooms.allowed`: optional explicit allowed hunting room IDs. Absent means
  legacy behavior. Present but empty is invalid, never unrestricted. Existing
  `rooms.boundaries` remain exclusions; exclusions win and overlap is rejected
  in map-created profiles. Never convert an allowed list into that legacy field.
- Existing `rooms.resting`, `rest.commands`, and `rest.until` remain town/default
  recovery for compatibility. Optional `rest.field` supplies its own room,
  commands, and completion thresholds. Field rest skips the selling round.
- Choose field only for routine mind/mana recovery when fresh native evidence
  confirms no wounds, no bleeding, full health, and no encumbrance. Missing
  evidence, wounds, encumbrance, full bags, or a stranded box select town.
- If eligibility changes while travelling to or resting in the field, abandon
  remaining field commands and switch to town. Do not flip back during the same
  rest cycle. A failed necessary walk stops the hunt, not an invented teleport.
- Hunting-room restrictions apply to hunting/wandering, not legitimate travel
  to the start or recovery location. Outside the allowed hunt, return to its
  start before choosing a target. Emergency escape policy is not invented here.
- Restricted wandering uses a graph containing only allowed rooms and plain
  command exits between them, preventing a shortest-path shortcut outside the
  selection. Scripted/routine/pass-through links are deliberately omitted from
  that graph until we can prove their intermediate movement safe. A selection
  relying on those links may not be fully traversable in this candidate.
- Healing, banking, and selling checkboxes must reflect actual native support.
  This candidate uses existing native rest commands and the town seller, not
  promises of unimplemented heal/bank/script engines.

## One source of configuration

Use the existing TOML profile loader and its precedence: character > named
profile > global > built-in. No second JSON Hunt setup store. Map corrections
remain in their existing store and are copied as an explicit room selection at
save time; later edits do not silently rewrite an active hunt's profile.

For the first integration candidate, Save creates a new named profile and
refuses overwrite. Preview checks higher-priority character overrides and
refuses if they change selected rooms, recovery, or attacks. Existing profile,
global, character, loot, and editor-correction files are never rewritten.
Profile replacement with optimistic concurrency can follow after this contract
is accepted; it is not necessary for the first operator-present test.
Unrelated global/character settings (including preparation and loot policy)
continue to inherit. The new profile layer contains only settings owned by the
form. Preview is invalidated if effective inheritance changes before Save.

The web layer receives an explicitly installed, session-bound configuration
callback. Native behavior owns validation, rendering, and storage. No new web
dependency on the behavior crate and no game-command capability in that callback.
The optional setup surface is authenticated with the existing pairing token and
same-origin checks. Offline preview binds to fixture identity and isolated data,
with no session or connector. It must not impersonate a live character.

## Decisions requested from Nisugi

1. Are the additive allowed-room and field-rest settings the preferred native
   contract? Legacy profile semantics stay unchanged.
2. Should nontrivial scars prohibit field rest as well? Candidate can conservatively
   require an entirely healthy observed body; relax only with an explicit rule.
3. Is create-new-profile sufficient for initial acceptance, with character
   override conflicts displayed rather than automatically changed?
4. Confirm the recovery actions we can expose now; reserve unavailable actions
   visibly instead of pretending existing Lich scripts execute natively.

## Candidate limits, not hidden promises

The first form uses one explicit attack routine shared by its checked creature
names. Native TOML supports richer per-creature routines; a visual routine editor
is a follow-up. The form shows a fixed initial recovery preset (full mind, any
encumbrance, mana below 20%; resume at mind at most 80%, mana at least 90%).
Field and town command lists are independent; native completion thresholds are
also independent, even though this initial form uses the same values for both.
Healing/banking scripts are neither supplied nor installed by Save.

The setup page is a separate, session-bound explorer tab. Its map clicks are
room choices, not movement. Runtime activation is opt-in with `--hunt-setup`.
It prefers artwork-guided layouts where available, remembers setup layout
preferences separately, and keeps both side panels scrollable without moving
the map. Switching to another regional page currently discards an unsaved draft;
finish the first acceptance test within the Landing bundle.

Map fingerprints are deliberately strict: replacing the map requires rebuilding
the matching explorer and reviewing pinned profiles. File writes use the native
create-new-only writer; a disk error is reported, not claimed as a successful save.
No general rest-script engine or emergency escape policy is part of this patch.

## Acceptance gates

Offline first: legacy tests, allowed-room containment, empty/stale rejection,
unknown eligibility, field-to-town escalation, distinct rest commands, no town
selling in field, native save/load equality, overwrite refusal, session isolation,
authentication, map-click selection, and zero game sends during setup.

Then an operator-present disposable-profile test: inspect saved TOML, start a
small known hunt, check permitted wandering, request/observe resource recovery,
verify town fallback, and stop. Record what actually ran. A compiled or replayed
build is **not** evidence of live-game acceptance. No unattended game login is
part of preparing this candidate.
