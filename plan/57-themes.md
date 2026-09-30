# 57 — Themes: a recipe, its pins, and the whole palette generated

**Status: PROPOSED 2026-09-30.** The author, 2026-09-30: *"Themes eh! We need to come up with
a solid theme plan"*, and of VellumFE's: *"vellum was a mess. I will admit it. One thing I want
to bring from vellum in some capacity though is the harmony stuff."* Nothing is built. §1 holds
the author's answers, §5 the steps, §6 what is still open. This is the themes part of
`plan/49` Stage F; skins (art) are a plan of their own, later (§1 item 3).

**Credit.** The harmony generator is **Niffy's** work: the prototype
`vellum-palette-harmony.html`, then its port in VellumFE. The author: *"Niffy is responsible
for the harmony and controller additions."* Hydra's port says so in its module's first lines
and wherever Hydra lists who made it.

---

## 1. The author's answers

| # | Asked | Answered |
|---|---|---|
| 1 | Does harmony generate the whole palette, or game text only as in VellumFE? | *"whole pallete"* |
| 2 | Is the Generate view the main way to make a theme, a per-colour editor behind it for pins? | *"sure"* |
| 3 | Is art (images: frames, button faces) in this plan? | *"art separate"* |
| 4 | One theme for Hydra, or one per character? | One for Hydra with an accent per character, and: *"there needs to be a way to change a theme for the character only, some people might want that."* |
| 5 | Light themes? | *"yep light for sure see harmony"* |
| 6 | Does the web page take themes? | *"I think the webui should also accept the themes ... Despana will be a theme option I expect, while despana's features will probably be the default webui. With mobile having an older style webui like vellums if we can't come up with a better phone ui."* |
| 7 | How many themes ship? | *"We can start with 2 yes."* |

What follows from them:

- **A theme is not a page design.** Item 6 separates two things that share a name: *Despana
  the theme* (its colours, a choice in the GUI and the web alike) and *Despana the web page*
  (its features, the web's default). A theme changes how either looks; it does not choose
  which page the web serves. The phone's page is its own question, not this plan's.
- **Light costs one input, not a second palette.** The generator takes the background and
  holds every colour to a contrast floor against it (§2), so a light theme is a light
  background and the same recipe.

## 2. What exists, measured

### 2a. Hydra today

- **Hydra sets no theme.** `grep -rnE "set_visuals|set_style|set_fonts" crates/cena-gui/src`
  finds nothing. egui's defaults follow the system's light or dark (`plan/53` §6 says the
  same of the map).
- **76 colour literals in 14 files**, outside the `tests` folders:
  `grep -rnE "Color32::(from_rgb|from_rgba_unmultiplied|from_rgba_premultiplied|from_gray|from_black_alpha|from_white_alpha|[A-Z_]{3,})|hex_color!" crates/cena-gui/src --include=*.rs | grep -v "/tests"`.

  | Where | What the colours mean |
  |---|---|
  | `crates/cena-gui/src/bar.rs:18` to `:34` | nine bars: health, mana, stamina, spirit, blood, stance, encumbrance, mind, level |
  | `crates/cena-gui/src/text.rs:58`, `:59`, `:173` to `:186` | game text: speech, whisper and thought, links, Hydra's own text, amber, creature, player, object, wrong |
  | `crates/cena-gui/src/widget/doll.rs:53` to `:63` | seven injury levels and the drawn body |
  | `crates/cena-gui/src/widget/status.rs:138` to `:152`, `:194` to `:197`, `:306` | six groups of indicator, four kinds of effect, the pulse |
  | `crates/cena-gui/src/widget/minimap.rs:30` to `:45` | nine of the map: background, room, lines, door, you, route |
  | `crates/cena-gui/src/play/arrange.rs:246`, `crates/cena-gui/src/play/holders.rs:497`, `crates/cena-gui/src/play/calibrator.rs:146`, `crates/cena-gui/src/play/draw.rs:223` | chrome: the arrange veil, a faint grid line, the calibrator's mark, an alert's text |
  | the rest (`WHITE` as an image's tint, `TRANSPARENT`, `PLACEHOLDER`, a trigger's colour converted) | not a theme's: they say "no colour" or carry someone else's |

- **32 reads of egui's own visuals in 14 files**
  (`grep -rcE "visuals\(\)|\.visuals\.|style\(\)\." crates/cena-gui/src`). These follow a
  theme the moment one is set.
- **A bar's colour is saved with the layout** as three bytes
  (`crates/cena-gui/src/bar.rs:125`, and the saved form in
  `crates/cena-gui/src/layout/tests/looks.rs:50`). INFERRED from those two: a saved layout holds
  the colour whether the player chose it or not, so a theme could not change it. Step 0
  settles it (§5).
- **The web page has twelve tokens** as CSS variables, `crates/cena-web/assets/style.css:4`
  to `:15`: six surfaces and lines, amber, text, muted, two font stacks. Several of the GUI's
  literals are these same values (`text.rs:178` is `--amber`; `minimap.rs:41` is `--text`).

### 2b. VellumFE, `reference/VellumFE` at `c1f7953`

| File | Lines | Taken? |
|---|---|---|
| `src/core/harmony.rs` | 888 | **yes**, the engine, whole |
| `src/core/harmony_skin.rs` | 519 | later, with art: it renders frames and panels from the same harmony |
| `src/theme.rs` | 4,360 | no. `AppTheme` is 56 colours named by screen (`editor_border`, `browser_item_focused`, `form_checkbox_checked`), most of it built-in themes written out field by field |
| `src/frontend/gui/skin.rs` | 4,009 | no, art |

(`wc -l`; the 56 by `sed -n 14,180p src/theme.rs | grep -cE "^\s+pub [a-z_]+: Color"`.)

**What harmony is** (`src/core/harmony.rs`):

- **Inputs** (`HarmonyParams`, `:346`): a seed colour, the background, a scheme, and four
  dials: hue variance, a WCAG contrast floor against the background, the least distance
  between two roles, and the room title's contrast against its own plate.
- **Eight schemes** (`:159`): monochrome, analogous, complementary, split, triadic, tetradic,
  golden, compound. Each is a list of hue offsets round the OKLCH circle.
- **Roles.** Eleven of game text (`ROLES`, `:265`), each given a slot in the scheme and a
  nudge of lightness and chroma so two roles on one hue stay apart; five of the prompt
  (`PROMPT_ROLES`, `:305`).
- **Anchored roles.** A colour that carries meaning keeps its hue whatever the scheme: the
  target and roundtime stay alarm red (`ALARM_HUE`, `:233`), stunned amber, hiding violet
  (`:302`, `:303`). Harmony moves only their lightness and chroma, for reading.
- **Pins** (`:363`): a role the player fixed is kept as written; the rest harmonise round it.
- **What it promises**, each a test in the file: every colour clears the contrast floor,
  roles are distinct from one another, a pin survives, an anchored role stays in its band,
  the same input gives the same output.
- **It depends on nothing**: strings and numbers in, strings out (`:9` to `:11`).

**What not to copy.** Colours named by screen rather than by meaning; themes written out in
code field by field; the theme and the appearance each kept in two places and copied by hand
(`plan/50` line 170). VellumFE's three variants (high contrast, colourblind, low blue light,
`src/theme.rs:429`) are filters over a finished theme; §6 asks whether Hydra wants them.

## 3. What a theme is

Three layers, each optional in a theme. A fourth, art, is left room and built later.

| Layer | Holds | How it gets its values |
|---|---|---|
| **Palette** | every colour Hydra draws, named by what it means | generated from the recipe; a pin fixes one |
| **Type** | the UI's font and the story's, their sizes, line spacing | written |
| **Shape** | corner radius, stroke widths, padding and density, shadows, scrollbar width | written |
| *Art* | *frames, button faces, icons* | *a later plan; harmony's skin half generates them* |

**Can a button's shape change?** Its corners, outline, fill, shadow and padding, yes: those
are Shape, and egui's style carries them to every stock control at once. A button *drawn
differently* (a bevelled stone face) is an image, and that is art.

### 3a. The palette's tokens

Named by meaning, never by screen. The list is fixed in step 0 from the 76 literals and
egui's visuals; its groups:

| Group | Tokens | Generated as |
|---|---|---|
| Surfaces | canvas, surface, raised, inset, line, strong line | free, low chroma, stepped from the background |
| Text | text, muted, disabled, selection | free, held to the contrast floor |
| Accent | accent, link, Hydra's own text | free, the seed's slot |
| Game text | VellumFE's eleven roles and the room title's plate; creature, player, object | free, a slot each, as VellumFE |
| Prompt | VellumFE's five | anchored, as VellumFE |
| Signals | danger, warning, good, wrong | anchored: red, amber, green |
| Vitals | the nine bars | anchored: health stays red, mana blue, stamina green, spirit gold |
| Injuries | the seven levels, the body | anchored, a ramp from warning to danger |
| Status | the indicator groups, the four kinds of effect, the pulse | anchored to the signal or vital each echoes |
| Map | the nine | free, from the surfaces and the accent; the route anchored |

Anchoring the vitals is this plan's one addition to the engine's idea: VellumFE generated
game text only, and the prompt's roles show how a colour with a meaning is held.

### 3b. The file

A theme is a file in a `themes` folder beside Hydra's own settings (`window.toml`), named for
the theme. Built-in themes are in code; a file holds only what it says, as the keys do
(`plan/52`).

```toml
name = "Ember"
base = "Hydra"          # what it does not say, this theme says

[recipe]
seed = "#c9733a"
background = "#14110f"
scheme = "split"
variance = 1.0
contrast = 4.5
separation = 0.09

[pins]                  # kept as written; the rest harmonise round them
health = "#cd4d4d"

[type]
story = "Iosevka"
story_size = 14

[shape]
corner = 3
density = "roomy"
```

The palette is computed from the recipe and the pins when the theme is loaded, never saved.
A theme made wholly by hand is one with every token pinned.

### 3c. Which theme, and what wins

1. **Hydra's theme**, chosen on Hydra's own settings, in `window.toml`.
2. **A character's own theme** (§1 item 4), named in that character's settings file. Its play
   window wears it; the hub and every other character keep Hydra's.
3. **A character's accent**: one colour, in the same file, over whichever theme it wears. It
   is a pin on `accent`, so the generator keeps it readable.
4. **A widget's own setting** over the theme. A bar's colour picker gains *Theme's*, meaning
   no setting of its own.
5. **A trigger's look** over all of it, as now.

### 3d. Where the code goes

- **`cena-ui`** gets the theme: the tokens, the file's form, the harmony engine, the built-in
  themes. It is pure, has `toml` already, and is what both `cena-gui` and `cena-web` depend
  on, so no edge is added to `ALLOWED_EDGES`. Colours are `cena_model::trigger::Color`
  (`crates/cena-model/src/trigger.rs:228`), the one colour type there is.
- **`cena-gui`** gets one module that turns a theme into egui's visuals, style and fonts, and
  a token into a `Color32`. It is the only place a colour literal may be written.
- **`cena-web`** is sent the theme and sets CSS variables from it.
- **The binary** writes the files, through the settings pages' one writer (`plan/50` §7).

## 4. The rule, and its test

**No colour literal outside the GUI's theme module.** A new architecture test scans
`cena-gui` for `Color32`'s constructors and named constants and fails on any outside that
module, excepting the three that mean no colour (`WHITE` as an image's tint, `TRANSPARENT`,
`PLACEHOLDER`). It is written in step 0 with the sweep, not after (`plan/05` §0): without it
the next widget brings a literal and a theme changes most of the screen.

## 5. Steps, each a commit

On a branch `themes` once the author approves.

0. **Tokens and the sweep.** The token list in `cena-ui`, each token holding the value its
   literal has today; the GUI's theme module; the 76 literals replaced; the architecture
   test. No theme is applied yet, and today's values are not a theme (§6 item 1): egui keeps
   following the computer's dark mode, so *nothing looks different, and the GUI's images
   (`kittest.toml`) must not change.* Settle the saved bar colour (§2a): a layout saves a
   colour only when the player chose one, and a saved colour equal to the old default is
   read as none.

   **BUILT 2026-09-30.** 61 tokens (`crates/cena-ui/src/theme.rs`, `Token::ALL`;
   `Palette::bare` holds each literal's value); the GUI's `crates/cena-gui/src/theme.rs`,
   `theme::color(ctx, token)` read from egui's own data so a theme (step 2) and a window's
   own (step 3) have a place to go; the test
   `crates/cena-arch-tests/tests/colour_literals.rs`, which scans `cena-gui`'s code lines
   for `Color32`'s constructors and named colours outside the theme module and test code.
   A bar's saved colour is `Option`al, `Look::themed(token)` reading the old default as
   none where the layout is read (`play/draw.rs`, `play/options.rs`).

   *One thing does look different.* Text on a lit indicator was always black, and a doll
   dot's numeral always white; both now pick black or white by the colour behind them
   (`theme::readable_on`, the bars' own rule), which a light theme needs. Four images
   changed for it and were reviewed: `status` (*Standing* and *Poisoned* read white),
   `injuries_hurt` and `injuries_picture` (the numeral on a light grey scar reads black),
   and `calibrator` (the same dots).
1. **Harmony.** The engine ported whole into `cena-ui` with its sixteen tests, Niffy
   credited. The roles widened to §3a's table. New tests: every vital stays in its band under
   every scheme; every token clears the floor on a dark and on a light background.

   **BUILT 2026-09-30.** `crates/cena-ui/src/theme/oklch.rs` (the colour maths, over `Rgb`
   rather than hex strings) and `crates/cena-ui/src/theme/harmony.rs` (`Recipe`, `Scheme`,
   `generate`, `hue_variants`, `seed_swatches`), Niffy credited in each module's first
   lines. Every token has a `Role` (`Token::role`): *free* in a slot of the scheme; *anchored*,
   keeping the hue of its bare colour, so the vitals' bands are measured from what Hydra has
   always drawn rather than typed; a *surface* from the background; the room's *plate*; or
   *fixed*, for chrome that is not a colour. Each token keeps its distance within its `Group`
   (`Token::group`: text, vitals, injuries, status, map, marks, chrome), as `VellumFE` kept the
   prompt's apart from the story's. A 62nd token, `RoomPlate`, which no widget draws yet.
   `crates/cena-ui/src/theme/harmony/tests.rs` holds the promises: the same recipe gives the
   same palette; every drawn token clears the floor on four backgrounds, two of them light,
   under all eight schemes; tokens in a group stay apart; a pin survives; an anchored token
   keeps its hue under every scheme and seed; health is still red and mana still blue with a
   green seed on the golden scheme; the plate hits its spread. Nothing applies a palette yet:
   that is step 2.
2. **The theme applied.** The file's form, `base`, recipe and pins; egui's visuals set from
   the surfaces and text; the two built-ins, **Despana** (the default, from the web page's
   twelve tokens) and a **light** one generated from a recipe; the choice on Hydra's *Window*
   page, and *Follow the computer*: a theme for dark and one for light, egui's reading of the
   system picking between them. The images change here, once, and are reviewed.
3. **A character's own.** Its theme and its accent in its settings file; its play window
   wearing it. UNVERIFIED: whether egui lets one window take a style the others do not; if it
   cannot be set per window, it is set at the top of each window's drawing. Checked first.
4. **Shape.** Corner radius, strokes, padding, density, scrollbar width, from the theme into
   egui's style and into Hydra's own drawing (`crates/cena-gui/src/bar/shape.rs:81` fixes a
   bar's corners at 3 today).
5. **Type.** The UI's font and the story's, and their sizes. Where fonts come from is §6's.
6. **The editor.** A *Theme* page in the settings menu: Generate first (seed, scheme, the
   dials, a preview of the whole palette and a sample of story text, live), then each token
   with its pin, then Type and Shape. *Save as* makes a file; a built-in is never written over.
7. **The web.** The theme sent to the page as a new wire message (`crates/cena-ui/WIRE.md`,
   a version's step), the page's CSS variables set from it. The page's design is not touched.
8. **Wrayth's colours.** `<presets>` and `<palette>` of a Wrayth settings file, which the
   trigger importer names and leaves (`plan/45` line 530), read into a theme's pins.

Steps 0 to 3 are the theme; 4 to 8 can come in any order after.

## 6. Open

1. ~~**Is the default today's look?**~~ **ANSWERED 2026-09-30, no.** The author: *"what
   you're calling the default theme is just egui respecting my computers dark mode. There is
   no theme applied. I'm not sure all black should be a theme."* So today's look is not
   shipped as a theme. Claude's recommendation, taken into step 2 unless the author says
   otherwise: Despana is the default, the second built-in is light, and following the
   computer's mode is a choice between a dark and a light theme.
2. **VellumFE's three variants** (high contrast, colourblind, low blue light): wanted? High
   contrast is already the recipe's contrast dial. The other two are filters over the finished
   palette, some 150 lines. Claude's recommendation: not now; the dial first.
3. **Where fonts come from.** (a) a `fonts` folder the player drops files into, beside
   `themes`; (b) the system's installed fonts, which needs a crate to list them. Claude's
   recommendation: (a) first, (b) when someone asks.
4. **A `;theme` command** beside the page (`;theme <name>`, `;theme mine <name>`)? Claude's
   recommendation: yes, the two forms only; the editor is the rest.
