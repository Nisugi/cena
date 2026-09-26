# Despana local map — read-only follow-up to the explorer

## Scope

The character Room pane gains a local map. It follows the native resolved room,
with active focus rooms, subdued surrounding context, gold transition markers,
an explicit transition directory and a link to inspect the current location in
the explorer. Map clicks open reference views; they never walk or send commands.
The camera has a dead zone so normal nearby steps do not constantly recenter it.
Scene DOM is reused within a focus; region data is cached with a two-region bound.

This is a first minimap, not the full explorer squeezed into a small pane: no
independent route executor, live hunting, automatic safety assessment, map
editing or speculative location guesses. The explorer remains independent.

## Native authority and identity

The executable loads `CENA_MAP` once per process. Hunt, travel and presentation share
that immutable `Arc<Map>`. Replacing the file on disk requires restarting Hydra;
a failed initial load also requires restart. The projection uses the existing
`cena_behavior::travel::room_of` resolver, not a browser UID lookup. The game's UID
and the map's room ID are distinct. Unknown/ambiguous results remain unknown.
Coalesced observations do not supply reliable step-by-step history, so this
first projection passes `Whence::Nowhere`, never an invented previous room.

`Sessions::attach_with_map` accepts an optional pure host projection. It runs
over the same native snapshot as the room pane and only while Ready. No new
crate edges couple the model, session, UI or web server to a map implementation.
The authenticated session wire carries the optional native result and map hash;
public atlas endpoints never contain character location or credentials.

The browser requires the native map's SHA-256 to equal the atlas manifest's
source hash, and verifies each regional bundle against the manifest before use.
A different `CENA_MAP` still supports native travel but cannot supply a live
marker on this frozen atlas. Rebuild the atlas from the matching file. This is
deliberately strict until a native scene provider replaces the frozen assets.

Disconnect, non-Ready lifecycle, unknown location, mismatched map, and rooms
excluded from the atlas all hide the marker with a reason. Asynchronous loads
are cancellable and epoch-fenced; an old load cannot resurrect the old character
or room. Views never share character position state.

## Verification and remaining work

Test with synthetic native snapshots and the offline `atlas_preview` server,
never by running the login binary. Unit tests cover native UID resolution,
ambiguity, additive wire compatibility, lifecycle clearing, load races and map
identity. Browser tests cover same-focus DOM reuse, catacomb transitions,
independent views, inspect-only clicks, mismatch and viewer disconnect under the
real server CSP. This is not evidence of a live game login or travel test.

Next layers: live native scenes for arbitrary map revisions; per-user minimap
zoom/pan preferences; approved hunting-region selections handed to the behavior
layer. Hunting setup must retain independent field-rest and town-rest routines,
and emergency escape must not be conflated with ordinary recovery. No second
movement authority or general script runtime is introduced here.

## Mainline integration (2026-09-25)

PR #9 was merged into the atlas feature branch, not `main`. This port reapplies
its minimap to mainline `75495e4`, preserving M6's single startup path, quiet
character sync, and native Hunt registration. No removed M1 demo/probe path is
restored. The native Hunt profile remains unchanged.

The separate setup prototype is not a native Hunt profile. Mainline uses TOML
through `crates/cena-behavior/src/hunt/chain.rs`; its `rooms.boundaries` excludes
rooms in `engine.rs`'s `next_room`, whereas the editor selects allowed rooms.
It also has one resting room, not independent field/town recovery policies.
Do not copy the selected membership into that exclusion list. Atari chose to
ship the map/editor independently and retain setup as a prototype until this
handoff is agreed with Nisugi. These PRs introduce no live Hunt controls.
