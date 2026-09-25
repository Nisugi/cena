# Developer hunting boundary editor

An opt-in editing layer for the existing offline explorer. This is preparation
for hunting setup, not a hunter, travel runtime, or change to map topology.

## Try it

Build/run the offline `cena-web` example `atlas_preview`, not the `cena` binary.
Set `CENA_HUNTING_CORRECTIONS_DIR` to an absolute folder on mounted bulk storage
to enable automatic correction files. Without that explicit host setting the
viewer remains read-only and uses the browser-only fallback. On its printed
localhost origin, open:

`/atlas/landing/#room=6385&browse=1&devhunt=1`

The `devhunt=1` fragment enables the **hunting boundary workspace**. It is a
developer UI switch, not a security boundary. The normal explorer reads validated
corrections when its host explicitly supplies the directory; it cannot write it.
The minimap remains read-only. No game command or map-database write is introduced.
Only the explicitly configured developer server enables the bounded correction
file endpoint. Navigation within the explorer retains the flag; returning via the
world directory requires opting in again.

Developer editing defaults to **Artwork-guided layout**, using the existing
reference-coordinate adapter instead of compressed native interior skeletons.
Areas without artwork retain native geometry; missing coordinates within an
artwork area remain explicitly schematic. This is not a claim that every map
has been visually verified. The layout selector still offers native comparison.
Its choice is remembered per region and separately for editor/ordinary viewer
in browser storage. A new localhost origin starts with the readable editor
default again. Changing layout never changes hunting membership or map data.

1. Click a hunting group in the left list or its name on the map. No group is
   selected by default. Both controls select the **same edit target**, owned by
   the editor; the map overlay has no independent selection while editing.
   The map and toolbar stay on screen while the group list scrolls. Secondary
   room details, display controls, route preview and backups use a separate
   scrolling inspector. Ordinary non-developer browsing is unchanged.
2. The toolbar names the target. Choose **Add rooms** or **Remove rooms**, then
   click tiles or Shift-left-drag a box. Each change names the affected group.
   Normal left- or right-drag always pans. Inspect never edits membership,
   including on Shift-drag. Selecting another group returns to Inspect; map
   labels remain selection controls even while an edit tool is active.
   Escape also returns to Inspect. A right-drag never clears a route; a plain
   right-click on its destination still does. Editing does not select a route.
3. Cyan rings show selected rooms, green dashed rings additions, red crosses
   removals. Hunting colors and existing safe connecting highlights preview
   corrected memberships, including unsaved drafts. The selected hunt takes
   color priority where selections overlap; other hunts are subdued.
   Undo/removal updates this preview immediately. Creature details remain
   source evidence, not inferred spawns in newly added rooms.
4. Undo reverses an entire gesture. Reset (under Review, notes & backups)
   restores generated membership and
   can itself be undone. Autosave preserves undo history. Empty selections are permitted.
5. Edits and notes autosave after a short debounce when the developer file host
   is enabled. The status shows the actual saved path. No filename or file picker
   is needed. Import is a separate, optional advanced control.

Selections may overlap. Changing one never removes rooms from another. Edits
are restricted to existing, non-closed rooms on the selected area map. Creating
new hunting identities and selections spanning multiple area maps are not part
of this first editor. Drafts survive switching areas/selections within a mounted
explorer; unsaved drafts trigger the browser's unload warning.

## Persistence and trust

`hunting-corrections.mjs` is the pure, reusable selection/delta/validation module.
`hunting-editor.mjs` owns the optional UI and mount-local interaction state.
Corrections are not written into the region bundle, native map, creature
associations, or shared dataset. A later hunting setup can consume the validated
effective memberships without parsing this UI. Ordinary hunting highlights use
the validated effective boundaries; creature details retain their separate source
evidence. Developer colors are explicitly a local boundary preview, not new
evidence or upstream-approved boundaries.

Files are named `<area>-YYYY-MM-DD.hunting.json` using the browser's local date.
The first edit creates the file; subsequent edits update that day's area file
with all its saved hunts. Older daily files remain. Startup loads the latest
dated records, even when the localhost port changes. Editing during an in-flight
save stays dirty until the newer edit is acknowledged. Failed writes remain
visibly unsaved and retain the unload warning.

The v1 JSON document contains `schema` and `records`. Each record contains:

- `area`, `hunt`, `name`: selection identity and a display label.
- `mapSha256`: source native-map fingerprint.
- `baseRooms`, `creatures`: generated baseline and creature identities.
- `added`, `removed`: explicit room-ID membership delta.
- `notes`: optional human review notes.

Room IDs are native map room IDs, not game UIDs or display coordinates. Labels
are not used to guess identity. No exit, coordinate, plate, or ownership fields
are accepted. Imported text is rendered as text, never markup or commands.

Browser-only fallback key: `hydra.hunting.corrections.v1`. Saving checks whether another
tab changed the stored value since load and refuses to overwrite unseen work.
Failed/quota-denied writes do not advance the saved state. Invalid existing
storage is left untouched and blocks writes rather than silently starting over.

In browser-only fallback, storage belongs to an origin, including its port. The native preview
server may choose a different port on restart. Export/import is the portable
path for that fallback; browser-local save alone is not a filesystem backup. Exports include saved
records plus unsaved drafts and do not mark drafts as locally saved. Keep those
files on mounted bulk storage. Import is explicit, validated and confirmed;
matching saved records are replaced, unrelated records retained. Export first
if previous revisions are needed. Existing browser-only work needs a one-time
export/import when moving into file mode on a different origin.

`WebServer::open_hunting_editor` opts into file writes; ordinary `open` does not.
`with_hunting_corrections` adds only read access. The normal Hydra host uses that
read-only option when `CENA_HUNTING_CORRECTIONS_DIR` is set; the offline example
opts into the editor writer. Neither path loads or writes native Hunt profiles.
The filesystem module locks the configured folder for one writer process, requires
exact same-origin JSON writes, checks optimistic revisions between tabs, rejects
path traversal and unexpected fields, and writes through a synced temporary file
and atomic rename. Changed external files cause a conflict, not an overwrite.
Room IDs and deltas are validated, but storing a correction does not certify its
habitat claims. No arbitrary filesystem paths are accepted from the browser.
The active dataset is bounded to 5 MB, retained daily files to 32 MB/2,000 files;
archive older daily files if that limit is reached. Other folder contents are untouched.

## Snapshot changes

Map fingerprint, generated room membership, creature identity, or room
availability changes withhold the saved correction and show generated rooms.
The developer must explicitly load the surviving old selection for review,
inspect it, and explicitly Save against the new snapshot (not autosave that review).
Missing/closed/out-of-area IDs
are listed, never rebound to a similar title. Old entries without a matching
hunting identity remain exportable; they are not guessed onto another hunt.

This is an editing preview and portable correction dataset. It does not promote
corrections upstream or automatically approve habitat, access, or safety.

## Verification

- Pure model tests cover delta round trips, unchanged sources, overlaps, undo,
  empty membership, changed snapshots, missing rooms, strict bounded imports,
  conflicting tabs, failed storage and rectangle coordinates.
- Native browser regression uses the real CSP/server and checks default-off,
  click/rectangle editing, route isolation, save/reload, export/import and
  explicit snapshot review. It uses no game session.
- The selection regression reproduces the original failure (click Roa'ter,
  Add, edit a room: the previous UI still targeted Outer Ward). It checks the
  persisted correction identity, map-label and keyboard selection during
  editing, no implicit first selection, and fixed map/tools while the group
  list scrolls, including a smaller desktop viewport.
- Native file tests and an additional browser regression cover automatic dated
  files, updates during an in-flight save, reload across ports, folder locking,
  invalid filenames/dates, external changes and origin/revision guards.
- Existing explorer and minimap regressions remain required.

## Named destinations and native Hunt handoff

`hunting-catalogue.mjs` combines generated grounds and valid corrections;
`hunting-destinations.mjs` consolidates populations under named places without
inventing spawn evidence. Dark Cavern, its tunnels and Dark Lair become one
selection with both troll kings and Sheruvian harbingers. Deep Mist remains a
separate place. Existing split records survive; saving the consolidated selection
is explicit, and a canonical saved selection takes precedence even when empty.

Exact native room tags supplement habitat UID evidence in the generated sidecars
(for example, nightmare steeds). No fuzzy matching or adjacency propagation is
used. No coordinates, exits, native area assignments or personal corrections are
changed by that regeneration.

Atari approved shipping this editor separately from the setup prototype on
2026-09-25. Native Hunt now uses TOML, exclusion boundaries and one rest location;
the prototype selects allowed rooms and separate field/town recovery choices.
The prototype remains outside this PR. See `plan/30-despana-live-map.md` for the
handoff gap; no browser-owned hunting engine or automatic conversion is added.
