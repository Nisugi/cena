# 55 — The injury doll

**Status: PROPOSED 2026-09-29.** `plan/49` Stage G named the injury doll as work that
*"gets its own plan when it is next"*. This is that plan. The author asked for it on
2026-09-29:

> *"We want to have a couple of options for people who want different things right."*

The author named three dolls:

- **The doll.** An image with coloured, numbered circles at each injury or scar, and a
  calibrator that tells it where each part sits on your image.
- **The doll plus.** The same doll with the dots replaced by wound overlays.
- **The doll infinite.** gs_studio's puppet, which has poses and animations and changes as
  things happen to the character.

The first two exist in VellumFE, *"may need some clean up, performance fixes"*. The third's
API is ready in gs_studio (`DollState::from_game`, `doll_view::paint_state`), and nothing in
Hydra calls it yet.

§1 is what exists, measured. §2 is the shape proposed. §3 lists the stages. §4 holds the
questions for the author.

---

## 1. What exists

### 1a. Hydra: the data is all there, and nothing draws it

**The model.** Everything a doll needs is already in the model:

- `Character.injuries` is a map from part to `Injury { wound, scar }`, ranks 0-3, holding
  only the parts that are hurt (`crates/cena-model/src/state/character.rs:196-225`).
- It is fed by `Frame::InjuryImage` for the `injuries` dialog only
  (`crates/cena-model/src/state.rs:427-433`). A name `InjuryN` sets the wound, `ScarN` the
  scar, and a name equal to the part means the part is whole
  (`crates/cena-model/src/state/character/body.rs:93-125`).
- There are **16 parts** in the wire's spelling, the feet among them
  (`crates/cena-model/src/state/character/body.rs:131-148`).
- `observed_body_parts` records which parts the game has actually stated, since an empty map
  alone cannot prove a healthy body.
- The injury window's own Wounds / Scars / Both radio is read as `InjuryMode`
  (`body.rs:47-91`).
- `StatusInfo` holds the indicators (standing, kneeling, sitting, prone, stunned, bleeding,
  hidden, invisible, webbed, dead) and the statuses read from text (bound, calmed,
  cutthroat, silenced, sleeping, thorned, poisoned, diseased)
  (`crates/cena-model/src/status.rs:52-234`). Debuffs are in `effects`.

**Other players' dolls are left out on purpose.** `injuries-<id>` is another player's doll,
and the model does not claim it (`crates/cena-model/src/state/dialogs.rs:43-46`, `plan/28`
:445-448).

**The GUI** has no injury widget. What a new one reuses:

- **Widget kinds** are declared in `crates/cena-gui/src/widget/kind.rs` and drawn in
  `widget/draw.rs`.
- **A widget's own settings page** comes from `play/options.rs` (`widget_pages`,
  `widget_change`, `keep`).
- **Per-widget options** are saved in the layout as maps keyed by the placed id
  (`crates/cena-gui/src/layout.rs:56-92`).
- **The bar's images** are the pattern to follow for pictures:
  - The picker is a Choice row over the PNGs in the data folder's `overlays` folder
    (`play/options.rs:475-486`, `app/settings.rs:214-259`).
  - The chosen path is saved in the layout.
  - Each picture becomes a texture cached in egui's temp data (`widget/draw.rs:270-292`).
- **That cache never retries a failed load and never notices a changed file.** The doll will
  lean on pictures far more than the bar does, so that is fixed in §3 step 1.

**Constraints that bind the design:**

- **No art is compiled into Hydra.** `include_bytes!` is banned
  (`crates/cena-arch-tests/tests/include_ban.rs:45-72`), and Hydra ships no images. Doll art
  is files in the data folder, or it is drawn in code.
- **The GUI has no file picker.** Settings rows are toggles, numbers, text, choices and
  colours (`crates/cena-ui/src/settings.rs:52-83`).
- **egui is the author's fork, pinned by `rev`** (`ed8b2649`, root `Cargo.toml`).

### 1b. VellumFE's doll

This section reads `reference/VellumFE` at `c1f7953`, which is newer than the `E:` copy.

**There is no "doll" or "doll plus" switch in the code.** VellumFE decides per part:

- A part with overlay art for its level draws that art.
- A part with no art at all draws a generated dot
  (`reference/VellumFE/src/frontend/gui/skin.rs:199-206`,
  `reference/VellumFE/src/frontend/gui/app/widgets/injury.rs:180-209`).
- Where the player chose no picture, a vector body is drawn in code, coloured by the
  7-colour severity palette (`injury.rs:227-441`).

**Where the art lives.** It lives in the player's image pool, `global/images/dolls/`. For each
doll (`reference/VellumFE/src/config/pool.rs:306-392, 635-674`):

- The base image is `<base>.png`.
- A sidecar `<base>.toml` holds the anchors, as `[x, y]` fractions of the drawn image, and
  the dot style (colours, opacity, diameter).
- Overlays are named `<base>_<part>_<level>.png`, where the level is one of `healthy`,
  `injury1`-`3` or `scar1`-`3`.
- Overlays are full-canvas and registered to the base, not placed at the anchors.
- A doll with no layers of its own borrows its group's: `nisugi_bow` uses `nisugi_*`.

**The calibrator.** It lists the parts, and a click on the canvas sets the selected part's
anchor and moves on to the next. It also offers dot colour, size and opacity, a preview rank,
and Save (`reference/VellumFE/src/frontend/gui/app/editors/doll_calibration.rs`).

A larger workbench adds per-condition variants: a prone pose, for instance, chosen by a
`when` rule and a priority
(`reference/VellumFE/src/frontend/gui/app/editors/doll_sets.rs`).

**What is wrong with it.** These are the clean-up and performance fixes the author expected;
every one is cited in the survey behind this plan.

- **Correctness:**
  - **Nerve wounds are dropped in two of VellumFE's three copies of the level table.** The
    popup's copy (`reference/VellumFE/src/data/ui_state.rs:1042-1050`) and the injuries
    panel's copy (`reference/VellumFE/src/frontend/gui/app/widgets/panels.rs:1048-1056`)
    both miss `Nsys1-3`.
  - The part list is written out four times.
  - Dots show rank only as a numeral, and only when a dot is big enough. They use one wound
    colour and one scar colour, not the palette.
  - The simple calibrator previews dots on parts that draw art instead.
  - The art doll's tooltip covers the whole doll, not the part under the pointer.
- **Performance:**
  - Every variant's base and all of its layers are decoded and uploaded on every reload,
    active or not. That is up to 84 full-canvas textures per doll, twice over with grayscale
    on.
  - The sidecars are read three or four times per reload.
  - A doll with no `.toml` is read whole for embedded metadata.
  - About eight allocations are made per frame in the renderer.
- **It writes into the player's own PNG.** Saving a calibration also embeds the sidecar as a
  PNG text chunk (`reference/VellumFE/src/config/pool.rs:1164-1191`), and reading one back
  can write a new `.toml` as a side effect (`pool.rs:899-918`).
- **Bundled art.** vellum-assets ships one pool doll, `dolls/dwarf_ranger.png`, plus skin
  dolls whose part overlays VellumFE's own migration drops. No per-part overlay art exists
  yet.

### 1c. gs_studio's puppet

This section reads `G:\dev\gs_studio` at `65f6ada`, with a clean working tree.

**The API.**

- `DollState::from_game(wounds, scars, statuses)` takes plain `(&str, u8)` pairs and status
  names, and returns gs_studio's own type (`gs_studio/crates/gs_field/src/doll_state.rs:291-315`).
  - It depends on no Hydra crate.
  - It already folds Hydra's spelling: `nsys` and the feet into the legs.
  - It already reads `StatusInfo`'s lowercase names.
- `doll_view::paint_state(ui, rect, cache, family, state, sky, id, gpu_skin)`
  (`gs_studio/crates/gs_field_egui/src/doll_view.rs:351-354`) paints into a rect the host
  allocates.
  - The cache lives in egui's context.
  - `id` is a stable name that gives the doll its breathing, easing and flinch.
  - `sky` can be a steady day to begin with.
- The lit path needs `field_gpu::init(ctx, render_state)` once, from eframe's creation
  context. Without it the doll falls back to an unlit skin on grey.
- The studio's doll tool is the host to copy (`gs_studio/crates/studio/src/tools/doll.rs`).

**What it does.** It draws a pose from the statuses: dead, prone, kneeling, sitting,
sleeping, webbed, stunned, dazed, calmed or standing. On top of the pose:

- cowering, slumping, limping and sway;
- breathing paced by exertion;
- a flinch by region when a wound rank rises, and a turn when the back is hit;
- marks baked on the skin;
- signs above the doll: stars, Zs, drops and bubbles.

**What stands between it and Hydra**, in order of how hard each one stops us:

1. **The egui source.** gs_studio names the fork by `branch = "numpad-support"`, while Hydra
   names it by `rev`. To cargo these are two sources, so egui would build twice and its types
   would not meet. gs_studio must pin the same `rev` as Hydra.
2. **One view id for every doll** (`DOLL_VIEW`, `doll_view.rs:325`). Two characters' play
   windows drawing dolls in one pass overwrite each other. The id must become a parameter.
3. **Its data folder** defaults to `~/.vellum-fe` unless the host calls
   `gs_field::data_dirs::set_base_dir`. Hydra must set its own folder.
4. **32 MB of art compiled into `gs_field`** (`include_dir`). Hydra's include ban scans only
   Hydra's own crates, so this would link, and the binary would grow by 32 MB.
5. **The licence.** Every gs_studio crate says GPL-3.0-or-later. Hydra's manifests declare
   none.
6. **Cost.** A turning doll costs 2-3 ms of CPU a frame on the GPU skin, and 20-26 ms
   without it. Each view keeps its own 2048² atlas, about 64 MB of GPU memory, freed after 300
   frames unseen. The doll keeps asking for repaints while it breathes.

Its `gs_field` also holds the **creature field**. That is a later plan's; nothing here should
make it harder.

---

## 2. The shape

### 2a. One widget, three styles

The three dolls are **one widget kind, Injuries, with a style** chosen on its own settings
page:

| Style | What it draws |
|---|---|
| **Doll** | the chosen picture (or, with none chosen, a body drawn in code), with a dot per hurt part at its anchor: coloured by level, numbered |
| **Doll plus** | the Doll, with a part's overlay drawn in place of its dot wherever the picture has art for that part and level. Where it has none, the dot remains, as VellumFE does |
| **Infinite** | gs_studio's puppet |

Why one kind rather than three:

- **One place to add it.** A player adds "Injuries" once and switches style in place.
- **One layout entry,** and one tooltip and hover path shared by all three styles.
- **One place to fall back.** Infinite falls back to the Doll where there is no GPU.
- **Plus is per part anyway.** "Plus" is not a separate renderer: it is the Doll whose
  picture has art. A picture with a few overlays shows art where it has them and dots
  elsewhere. That is VellumFE's rule, and it answers *"people who want different things"*
  without a second widget.

### 2b. Art is files, VellumFE's convention kept

Doll pictures live in `<data>/dolls/`, next to the bar's `<data>/overlays/`. **VellumFE's
naming is kept exactly:** `<base>.png`, the sidecar `<base>.toml`, and the overlays
`<base>_<part>_<level>.png`, with group sharing. Keeping it means:

- A player's existing VellumFE dolls work by copying the folder.
- Art made for one client works in the other.

Two changes from VellumFE:

- **Hydra never writes into a player's PNG.** The sidecar `.toml` is the only thing saved.
  An embedded text chunk is read once, on import, into a new `.toml`, and the image itself is
  left alone.
- **Anchor keys are written in the wire's spelling** (`leftArm`). Lowercase keys are still
  read, so VellumFE's sidecars load.

The sidecar is written through `cena_session::store::save_text`, atomically, like every
other file Hydra keeps.

### 2c. One table of parts and levels

Hydra already has one table of parts (`ALL_PARTS`) and one model of a rank (`Injury`). The
widget reads those, and nothing else defines them. This is how VellumFE's nerve-wound bug
cannot happen here.

**The default anchors** are VellumFE's (`reference/VellumFE/src/config/skins.rs:616-631`),
plus two for the feet.

**What a dot looks like:**

- Its colour follows its level through the 7-colour palette. That is VellumFE's vector body's
  palette, applied to the dots, which VellumFE never did.
- Its numeral shows the rank.
- It gets a per-part tooltip.

**Which layer shows** follows the game's own Wounds / Scars / Both radio (`InjuryMode`),
which the model already reads. A setting overrides it (§4 question 5).

### 2d. Loading only what is drawn

This is the performance plan, stated as rules, each one a test:

- **Only what is on screen is decoded.** The chosen base, plus the overlays for the parts and
  levels currently shown, are loaded on first use. Nothing else is: not every level, and not
  every variant.
- **Nothing is read from disk on a draw.** The sidecar is read once and cached by path and
  modification time. The folder listing is cached, as the bar's is.
- **One texture cache for every picture the GUI loads**, keyed by path and modification
  time. A changed file is reloaded, and a failed load is retried when the file changes. The
  bar moves onto it.
- **Nothing is allocated per frame** but the hover tooltip's text. The part table is fixed,
  the palette is parsed once, and the per-widget options are looked up by id, not searched
  for.

### 2e. The calibrator

The calibrator is a window, like the trigger editor (`plan/54`), opened from the widget's
right-click menu: *Calibrate doll...*. It shows:

- the parts, calibrated ones marked;
- the canvas, where a click sets the selected part's anchor and moves on to the next;
- Use default;
- the dot's colours, size and opacity;
- a preview rank for wounds or scars.

**The preview is the real renderer.** On the canvas the doll is drawn by the same code the
widget uses, so what you calibrate is what you get: dots where there is no art, and art where
there is. VellumFE's two editors each carried their own copy of the canvas. Hydra's has one.

Per-condition **variants** (VellumFE's workbench) are a later step (§3 step 8). When they
come, their `when` rule is a guard expression from `plan/33`'s vocabulary, the one the hunt
and the triggers already read, rather than a third condition language.

### 2f. Infinite: an adapter, two changes in gs_studio, and one hook in Hydra

**In gs_studio** (the author offered to make these there, and should):

1. Pin the egui fork by the same `rev` as Hydra, and keep the two moving together from then
   on.
2. Make the doll's view id a parameter of `paint_state`.
3. Hydra calls `set_base_dir` with its own data folder. That is a line in Hydra, but
   gs_studio's docs should say so.

**In Hydra:**

- **Depend on `gs_field` and `gs_field_egui`** by git and `rev`, as the binary already does
  on hydra-mapper, and **only from `cena-gui`**. The crates Hydra builds for phones never see
  them.
- **Call `field_gpu::init`** in `cena-gui`'s `run`, from the creation context
  (`crates/cena-gui/src/app.rs:528-535`).
- **An adapter, `widget/doll/infinite.rs`,** turns the character into gs_studio's input:
  - the wounds and scars from `Character.injuries`;
  - the statuses from `StatusInfo`'s active flags and the afflictions;
  - the debuff names from `effects`.

  It then calls `paint_state` with a steady sky and the id `doll:<GAME>:<Name>`. The game's
  own sky and facing are a later step.
- **When it cannot draw, it falls back to the Doll.** That covers no GPU and a puppet that
  does not resolve. The kittest snapshots draw the Doll style, plus one image of the fallback;
  the animated puppet is tested by its adapter, not by pixels.
- **It stops asking for repaints when it cannot be seen.** A widget in a hidden tab or a
  closed drawer does not breathe.

---

## 3. Stages

Each step is committed and tested on its own, on branch `injury-doll`.

1. **The picture cache.** One texture cache for the GUI's pictures, keyed by path and
   modification time, which retries after a change. The bar moves onto it. Tests: a changed
   file is reloaded, a failed one retried, an unchanged one kept.
2. **The Injuries widget, Doll style, with no picture.** The kind, its name and group, and
   its size. The body drawn in code, with a dot per hurt part coloured by the palette and
   numbered, and a tooltip per part. It follows the game's Wounds / Scars / Both radio.
   Snapshots of a whole body, a hurt one, and one with scars. The test character gains
   injuries.
3. **Pictures and anchors.** `<data>/dolls/` listed on the widget's page. The sidecar read
   with its anchors and dot style, VellumFE's lowercase keys included. The default anchors,
   the feet among them. A snapshot over a picture made in the test.
4. **The calibrator.** The window, the canvas drawn by the widget's own renderer, Save
   through `save_text`, and Use default. Tests: a click sets an anchor as a fraction of the
   drawn picture, whatever the window's size; a saved sidecar reads back the same.
5. **Doll plus.** Overlays by VellumFE's naming, group sharing, a part's art drawn in place of
   its dot, and only the shown levels loaded. Tests over pictures made in the test: art
   drawn where it exists, a dot where it does not, an inherited group, and nothing decoded
   for a level not shown.
6. **Bringing VellumFE's dolls over.** `;doll import <VellumFE folder>` copies the dolls
   folder. It turns each embedded sidecar into a `.toml`, never writing into an image, and it
   says what it brought, as `;scripts import` does.
7. **Infinite.** After gs_studio's two changes (§2f):
   - the dependency, with its reasons in `Cargo.toml`;
   - `set_base_dir`;
   - `field_gpu::init`;
   - the adapter;
   - the style on the widget's page;
   - the fallback;
   - no repaints unseen.

   Tests: the adapter's statuses and ranks for a known character; the fallback when the view
   is not initialised; two characters' dolls in one pass, each with its own view id.
8. **Later, each on the author's word:**
   - variants chosen by a guard word (a prone picture, a dead one);
   - other players' dolls from `injuries-<id>`, as a popup;
   - the game's sky and facing for Infinite;
   - Despana's doll.

The architecture page and glossary change in the same commit as each step that adds a name.
The glossary gains *doll*, *anchor*, *overlay* and *style*.

---

## 4. Questions for the author

Each has Claude's recommendation first.

1. **One widget with three styles** (recommended, §2a), **or three widgets** in the Add menu?
2. **The feet.** Hydra's model has 16 parts; VellumFE's doll and gs_studio's puppet have 14,
   folding the feet into the legs.
   - **Recommended:** the feet get anchors of their own in the Doll, defaulting to just below
     the legs, and fold into the legs for Infinite, as gs_studio already does.
   - **Or:** fold them everywhere.
3. **Never write into a player's picture** (recommended, §2b), **or match VellumFE** and
   embed the sidecar in the PNG so a doll travels as one file?
4. **With no picture chosen, the Doll draws a body in code** (recommended: it works the
   moment the widget is added).
   - **Or:** Infinite is the default once it is in.
   - **Or:** a picture ships with Hydra's install. It cannot be compiled in; it would be a file
     the installer puts in `<data>/dolls/`.
5. **Wounds, scars or both.** Follow the game's own radio (recommended: it is what the player
   already set in the game), **or** a setting on the widget, **or** the radio unless the
   widget's setting overrides it?
6. **Infinite's weight.** The 32 MB of puppet art, 2-3 ms a frame and about 64 MB of GPU
   memory while shown. Should that always be in Hydra (recommended while Hydra is desktop
   only), **or** behind a cargo feature a smaller build can leave out?
7. **The licence.** gs_studio's crates are GPL-3.0-or-later, and Hydra declares no licence.
   Linking them makes Hydra's binary GPL. That is the author's call; it is recorded here so
   it is made on purpose.
8. **The puppet's form.** gs_studio has `humanoid`, `troll` and `canine`. Is every character
   `humanoid` for now (recommended), **or** is the race mapped to a form, or chosen by the
   player?
9. **gs_studio's two changes** (the egui `rev` and the view id): the author offered to make
   them in that repo. Recommended: yes, since they are gs_studio's API, and step 7 waits on
   them. Steps 1-6 do not.
