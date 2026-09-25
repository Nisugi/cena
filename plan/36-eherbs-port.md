# 36 — The Heal behavior: eherbs, measured and ordered

**Status: PROPOSED 2026-09-25, stages built in order.** The author, 2026-09-25: *"port
eherbs, the healing, the survivalist/distiller stuff, the stock (means adding all the shop
data into the model crate)"*. This is the record of what eherbs does, what Hydra already
has for it, and the order it is built in. The rules of `plan/31` apply unchanged: a pure
planner over the state, a thin driver, replies read as a closed set.

## 0. Measured

`reference/scripts/scripts/eherbs.lic`, clone commit `7fd0b97` (2026-09-17), version
2.2.1. Identical in the herb table to the live install at `C:\Gemstone\lich-5\scripts`
(`diff` over lines 827-1113, empty).

| What | Lines | Measured by |
|---|---|---|
| the whole script | 3,639 | `wc -l` |
| the herb table, `known_herbs` | 827-1113, **247 herbs** | `awk 'NR>=827 && NR<=1113' eherbs.lic \| grep -c "name:"` |
| herb kinds in the table | **23** (19 injuries, blood, poison, disease, lifekeep, raisedead) | `grep -o "type: *'[^']*'" \| sort \| uniq -c` |
| the dose monitor, `exec_str` | 255-348 | a `DownstreamHook` reading doses left |
| `next_herb_type` | 3200-3310 | which injury is treated next |
| `use_herbs` | 1916-1987 | find, fetch, eat or drink, until healed |
| survival kit and distiller | 2981-3076, 2469-2518 | `analyze`, `look in`, `point #kit at dose` |
| stocking | 1691-1896, 2214-2302 | counts against minimum doses, a shopping list, `order`/`buy` |
| `fill` | 1642-1689 | one of each kind the town sells |
| `escort`, `deader` | 1475-1640 | healing another character |
| GTK setup | 525-826 | never: the profile is a file |

## 1. What Hydra already has

- **The wounds and scars**, with Lich's composites written with eherbs in mind:
  `cena-model/src/state/character/body.rs` (`LIMBS` omits feet because `eherbs.lic:3222`
  reads `Scars.limbs == 3` as a severed limb; `TORSO` includes the eyes because eherbs'
  `organ` herbs cover them).
- **Poison and disease** as status, the health bar, the hands, the inventory's containers.
- **Travel by tag** (`;go2 herbalist` resolves), the bank's reading half (`plan/20` §0b), and
  the selling round's `deposit`/`withdraw` steps (`plan/31` Stage 4b).

## 2. The stages

### Stage 1 — the stock: the herb table in the model

**BUILT 2026-09-25.** `tools/extract_herbs.rb` → `cena-model/data/herbs.tsv` (247 rows, the
header records the clone commit and eherbs' version); `cena-model/src/herbs.rs` (`HerbKind`,
`Severity`, `Area`, `Hurt`, `Source`, `Herb`, `herbs()`, `herb_for`, `sold_in`,
`location_stocked`, `is_drinkable`, `bundles`); `cena-model/src/state/doses.rs` (`Doses`,
`GameState::doses`, fed at each prompt, kept across a reconnect). Tested in
`cena-model/tests/herbs.rs`, the table's count pinned to the extractor's header so a row
that fails to parse cannot vanish silently.

`known_herbs` extracted whole into `cena-model/data/herbs.tsv` by `tools/extract_herbs.rb`,
which evaluates the literal and fails on any key it does not know (`port-data-whole`). Every
field: `name`, `short_name`, `type`, `store_doses`, and every `location` in order, including
eherbs' non-shop markers (`Do Not Buy`, `Forageable`, `Skinnable`, `Alchemical`).
`cena-model/src/herbs.rs` types it: `HerbKind` as the closed vocabulary of 23, the table,
lookups by name and by what a town sells, eherbs' drinkable rule and its major-blood list.

The **dose monitor** is a classifier: the lines `exec_str` hooks (*You have N bites left*,
*has a few doses left*, *That was the last drop*, the purchase, the bundle) become a fact
per item id, kept in the model as the doses a herb has left, three-valued like the rest.

### Stage 2 — the healing

**BUILT 2026-09-25.** `cena-behavior/src/heal/`: `choose.rs` (`next_kind`, eherbs' order
and its `--spellcast`/`--ranged` order, the severed-limb test that reads the right hand twice
ported as written), `plan.rs` (`Healer`: the herb in hand, else fetched from the container
found by its name's words, `eat my`/`drink my`, a kind with no herb skipped and named, a
kind used twenty times without healing given up on, the herbs put back), `reply.rs`,
`profile.rs` (`HealProfile`, `<data>/hunt/heal/<instance>_<character>.toml`). In the hunt:
`Said::Heal` and `Phase::Healing` on arriving at the resting room hurt, after the selling
round and before the rest commands; `;heal [spellcast] [ranged] [blood]` runs the same
driver with a machine that heals once (`Hunt::heal_only`). Tested in `tests/heal_plan.rs`
(7) and `hunt_engine.rs` (2). The profile has no importer: eherbs keeps its settings in
Lich's per-character store, not a file.

A pure planner, `cena-behavior/src/heal/`: `next_herb_type` over the body, the health and the
two statuses, with eherbs' order (blood under half health, poison, disease, major wounds by
area, minor wounds, a severed limb, a missing eye, major scars, minor scars unless skipped,
blood again when seven short); the herb found in the hands, then the herb container
(yabathilium first for blood when asked, edible before drinkable unless potions are
preferred); `get`, `eat`/`drink`, and a kind with no herb skipped for the rest of the
run. Options as eherbs' switches: `blood` only, skip scars, prefer potions, yabathilium,
`--spellcast`, `--ranged`. The herb container is named in a profile file. A `;heal`
command runs it.

### Stage 3 — the survival kit and the distiller

**BUILT 2026-09-25.** `cena-model/src/state/kit.rs` (`Kits`, `GameState::kits`): what
`analyze` says of a container (a kit, its tier, the extractor, what it is distilling) and a
kit's DOSE and TINCTURE listing with counts and ids, read from the chunk by the kit's id. The
healer analyzes the container once a run, takes a kit's herbs from its listing (a TINCTURE
drunk), and after healing analyzes afresh and points the kit at a dose
(`heal/kit.rs`, `distill_target`: a solid with no liquid yet, else the scarcest liquid),
unless it is already distilling; `distiller = false` in the profile turns it off, `true`
tries a kit not known to have the extractor. Tested in `cena-model/tests/herbs.rs` (the
reader on wire) and `tests/heal_plan.rs` (2). The kit's tier scales the stock (Stage 4).

`analyze` names a Survivalist's Kit, its tier and whether it has the Liquid Extractor; `look
in` lists its DOSEs and TINCTUREs with counts; the planner reads both. The distiller points
the kit at the solid herb with no liquid counterpart, else at the liquid it has least of,
unless the extractor is already busy.

### Stage 4 — stocking

The herb container counted per kind against eherbs' minimum doses (scaled by the stock
percent, or the kit's tier); a shopping list priced from the herbalist's menu (`order`,
read once per herbalist and kept); silver withdrawn; `order N <item>`, `buy`, the herb
stored, like herbs bundled. `fill`, one of each kind the town sells, is the same planner
with a minimum of one.

### Not ported, named

`escort` and `deader` (healing another character), casting 650/1035/9713 before eating
(a spell a profile can list instead), F2P banking, the GTK window.
