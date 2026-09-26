# 4-loot-trade: lich_repo_mirror survey

Scope: 380 scripts, 214 substantive (capture + data >= 5); 40 read deeply (header, every
capture line, every data literal), 174 at capture-line depth, 166 in the tail by name and a
one-line capture summary; Cena HEAD 3566f8d (`git -C E:/Cena rev-parse --short HEAD`),
2026-09-25. Pile total 66,423 lines.

**Absence searches** were run with `tools/4-loot-trade/cg.sh "<phrase>"`, a fixed-string
`grep -rlF --include=*.rs --include=*.tsv` over `E:/Cena/crates/*/src` and
`E:/Cena/crates/*/data` (tests added with `CG_TESTS=1`). Every GAP below names its phrase.
Item-name tables were run through `tools/4-loot-trade/classify.py`, a Python re-implementation of
`cena-model/src/state/gameobj.rs:95-107` over `data/gameobj-data.tsv`.

## Top findings

1. **Shop appraisals with no `silver` word are lost, and the selling round then keeps the item.**
   eloot's own parser takes `N for it if you want to sell` and `N for this if you'd like`
   (`reference/scripts/scripts/eloot.lic:5910`). This pile shows the same wording:
   `gemseller.lic:30` and `gemvalue.lic:22` (`matchfind "you ? for it if you want to sell"`),
   and `stupid-deed.lic:87` (`/I'll give you \d+ for/`). Every Cena shop-appraisal pattern
   needs `silver` (`cena-model/src/state/ledger/town.rs:72` `shop_offer`, `:73-75`
   `jeweler_offer`, `:76` `shop_worth`). The planner reads the value only from an `Appraised`
   fact (`cena-behavior/src/town/plan.rs:481-484`), and with no value it stows the item instead
   of selling it (`:504-507`). `./cg.sh "if you want to sell" "for this if you'd like"` ->
   NONE. **PARTIAL, HIGH, M6 Stage 4a.** How often it happens is INFERRED. The full-line wording
   is UNVERIFIED: check it against the corpus, with the author's permission.
2. **Bank replies: Cena reads 3 shapes, and the pile has 11.** `bank.lic:36` (Dantax, also
   `tbank.lic`) matches these: *Very well, a withdrawal of N*; *That's N silver(s) to your
   account*; *scrip for N silvers, with a N silver fee for the scrip* (Teras);
   *I have a bill of N silvers presented by your creditors* (debt); *You deposit your note worth
   N into your account*; *They add up to N silvers*; *teller scribbles the transaction into a
   book and hands you N*; *teller carefully records the transaction, (and then )?hands you N*.
   Cena has deposit, withdraw and note-deposit only (`ledger/town.rs:95-97`). Tested with
   Python `re`: Cena's withdraw pattern does **not** match *records the transaction, and then
   hands you 500 silvers* (`town.rs:96` has `(?:and )?`. Three sources spell `(?:and then )?`:
   `bank.lic:36`, `colmaster.lic:1280-1281`, and Lich's `lich-5/lib/gemstone/bank.rb:37`). The same alternatives sit in
   `lich-5/lib/gemstone/bank.rb:22-46` and `loottracker.lic:1255-1293`, so the fix is a port.
   **PARTIAL, HIGH, M6 Stage 4b and the ledger.**
3. **A note with a surcharge (the Elven Nations) is not read as a sale.**
   `duskrunner_support.lic:23` quotes a real line: *He scribbles out a City-States promissory
   note for 35000 (minus a small 72 silver surcharge) and hands it to you.* The gem-shop form is
   in `duskrunner.lic:69`: *She then hands you a City-States promissory note for N silvers,
   minus a small N silver surcharge.* Both fail Cena's `pawn_note` (`town.rs:68`) and
   `gemshop_note` (`town.rs:78-80`), which need `for N silvers?` followed by ` and hands` or `.`
   (tested with Python `re`). `./cg.sh "surcharge"` -> NONE. **CONFLICT, HIGH, M6** (the ledger
   loses the sale, and the planner's `sold` flag, `town/plan.rs:466`, stays false).
4. **Skins that Cena's own bestiary names do not classify as `skin`.** Prompted by
   `Autobundle.lic:29` (Mindl, hides list updated 2024-06, 286 names): 13 of the 286 get no
   `skin` type from `classify.py`. Then all 291 `creatures.tsv` skin values (column 39) were
   cross-checked. *rolton eye* (mountain rolton), *glossy kiramon chitin* (chitinous kiramon
   myrmidon), *chipped troll tusk* (mongrel troll), *tailspike* (three-toed tegu), *ki-lin horn*
   and others get no `skin` type and no `sellable furrier` (`gameobj-data.tsv:59-70`). The
   town routes a lot by `sellable` (`town/goods.rs:1-7`), so these go to the pawnshop.
   **CONFLICT inside Cena, MEDIUM, M6.** The list is in `tools/4-loot-trade/bestiary_skins_cls.txt`.
5. **The bounty facts this pile needs are missing (M8).** Lifetime bounty points,
   *accumulated a total of N lifetime bounty points* from `bounty` (`badge2.lic:33`,
   `badge.lic:13`). The heirloom find, *You spy X, which looks like the heirloom that you are
   searching for!* (`mugreport.lic:358`). The skin-quality ladder *crude < poor < fair < fine <
   exceptional < outstanding < superb < magnificent* (`skintracker.lic:27`): Cena's skin
   appraisal drops the grade (`town.rs:59`), and the bounty keeps `quality` as a bare string
   (`bounty.rs:408`), so *of at least fine quality* cannot be tested. The gem dealer's *You can
   SELL them to me as you find them.* (`sparklies.lic:145`). All four GAP, MEDIUM, M8.
6. **Jar hoarding messages** (`plan/31` §4d, Hoard not ported): *Inside X you see N
   portion(s)* (`ezmove.lic:449`, `gemscounter.lic:129`, `shake.lic:58`, `doit.lic:396`),
   *The X is empty.* (`doit.lic:403`), *You add ... filling it* / *is full* / *does not appear
   to be a suitable container for* (`ezmove.lic:304-321`, sample in `jarcatch.lic:9`), and
   `rummage in X ingredient Y` (`ezmove.lic:266`). GAP, MEDIUM. Cena already keeps a jar's
   *containing ...* after-name (`state/room.rs:88-96`); only the count is missing.
7. **Merchant `order`/`buy` replies.** *Sold for N silver* (`chargeitem.lic:161-162`,
   `backroom-auto.lic:65`), *But you do not have enough silver!* (`chargeitem.lic:161`,
   `sammo.lic:685`), *accept only local notes* (`gforgeb.lic:610`), *There is no merchant here to
   order anything from.* (`backroom-auto.lic:82`), *You may order a QUANTITY* / *no such item*
   (`:85`), and the catalog's `<d cmd='order N'>` links (`chargeitem.lic:131`). Cena's buy
   errands check only for *hands you* (`travel/routines/shopping.rs:314-324`, `giant.rs:64`).
   `./cg.sh "Sold for" "But you do not have enough" "no such item"` -> NONE. GAP, MEDIUM
   (M6d herbs, M8).
8. **Loot that arrives outside a search.** Mug results: *You rifle X pockets and discover N
   silvers!*, *X had Y tucked away!*, *X didn't hide Y well enough!*, *You find Y on X!*
   (`mugreport.lic:216-391`). *Your hasty search scatters the X's riches to the floor!*
   (`gemstone-tracker.lic:1469`). Silver passed between players: *X just gave you N coins which
   you quickly pocket.* (`fun-with-dreaven-video-1.lic:65`), and the ACCEPT/offer flow
   (`saccept.lic:137`, `offercheck.lic:22`). Cena's ledger reads item lines only under
   `You search the` (`ledger/hunt.rs:37-40`, `:133-140`). GAP, MEDIUM: the ledger, and
   multi-session silver moved between one's own characters. (The ledger's item reader is
   `hunt.rs:39-41`; `search_result` at `:135` reads a line only while a search is open.)
9. **Items that must never be sold.** `inventory full` marks items `(registered)` and
   `(marked)` (`dont-lose-it.lic:97-123`), and `mark` answers *has been (permanently) marked as
   unsellable* / *is not marked as unsellable* (`colmaster.lic:1070-1078`). The selling round
   skips `sell_exclude`, ready-list and bound items (`plan/31` §4a), but it cannot see a mark.
   `./cg.sh "(marked)" "marked as unsellable"` -> NONE. GAP, MEDIUM, M6.
10. **Pet types, so a hunt does not attack a companion or familiar.** `xmlpatch.lic:125-209`
    (LostRanger 0.3.2) adds `companion` (ranger companion nouns plus an exclude list) and
    `familiar` (wizard familiar names). Neither is in `gameobj-data.tsv`, nor in the live
    `C:/Gemstone/lich-5/data/gameobj-data.xml` (`grep -c "familiar\|companion"` -> 0). GAP,
    MEDIUM, M6. Whether Hydra's `dDBTarget` target list already leaves pets out
    (`state/targeting.rs`) is UNVERIFIED.
11. **Refusals, confirmations and trash warnings.** The pawnshop's
    *To accept the offered price, attempt to resell it again within the next 30 seconds.*
    (`duskrunner.lic:69`, `dusk_sell.lic:41`). Icemule's pawnbroker saying *Not my line, really.*
    (`unload.lic:31`), a `WrongShop` form missing from `town/reply.rs:64-71`. The trash
    confirmations *If you wish to continue, throw the item away again within fifteen seconds.*
    (`CIDig.lic:79`) and *There appears to be an item or items attached to the container you
    are throwing away that is either scripted or of significant value.* (`dr_dig.lic:184`,
    `riftboxes.lic:19`). The pool treats an unrecognised trash answer as a failure and drops the
    box next (`town/pool.rs:175-186`). GAP, MEDIUM, M6 Stage 4c.
12. **Furrier and gem-dealer appraisal variants.** *...I'll pay you N silvers for it.*
    (`appskin.lic:74-75`, Dalz 2023): `town.rs:72` has only `give|offer`. *The gemcutter takes
    the X and inspects it carefully...give you N silvers* (`gemvaluestracker.lic:30`) fits
    `jeweler_offer` only when the quote follows *before saying, "I'll give you*. The Duskruin
    appraiser says *How's N silvers sound*, *I already quoted N*, *that's worth at least N* and
    *I've no use* (`dr.lic:84-105`). PARTIAL, MEDIUM (furrier, M6) and LOW (Duskruin).

**The five named scripts, and whether any holds data Cena lacks.** None is a loot or trade
engine.
- `colmaster.lic` (Dreaven v46) masters the Council of Light in Solhaven. It holds three
  tables: 336 Inquisitor Q&A rows, 25 skinning tasks (skin, creature, level, rooms) and 11
  gift items with shop order numbers. Cena already has the skin-to-creature mapping in
  `creatures.tsv`. The rest is society-quest data with no milestone (LOW). It also shows the
  `mark`/unsellable replies (Top 9) and a third source for the *and then hands you* withdraw
  (Top 2).
- `coltv.lic` (Fib 0.2, 2015) is an older, smaller CoL task helper, with 127 Q&A rows (`grep -cE '^\s*"[^"]+"\s*=>\s*"' coltv.lic`). LOW.
- `cobble.lic` (Dreaven v54) masters cobbling. It holds 37 workshop rooms and the buy and rent
  replies. Crafting, N/A.
- `slich.lic` (SpiffyJr) overrides Lich's `empty_hand`, `fill_hand` and `move`. Its move-failure
  union is already in `cena-model/src/movement.rs`, so nothing new.
- `gforgeb.lic` (Gwrawr 1.2 beta) does artisan forging. It has the `order`/`buy` replies
  (Top 7) and the note noun per bank (`chit` in Icemule, `scrip` in Kharam-Dzu), which Cena
  already has (`state/bank.rs:54`).

**Authors and licences for the HIGH items.** `gemseller.lic` and `gemvalue.lic`: Caithris.
`stupid-deed.lic`: no author. `bank.lic`: Dantax. `tbank.lic`: elanthia-online.
`duskrunner.lic` and `duskrunner_support.lic`: no author. None of them has a licence line
(`grep -i licen`). Per the brief, what is recommended for porting is the game's message text
and its tables, not the code. Lich's own `bank.rb` already holds the bank shapes.

## Data tables Cena lacks or differs on

| script:line | table | rows | fields | Cena closest (file:line) | verdict | value |
|---|---|---|---|---|---|---|
| Autobundle.lic:29 (Mindl, 2024-06) | bundleable hides | 286 names | name | `gameobj-data.tsv:68-70` `type skin`; `creatures.tsv` col 39 `skin` | PARTIAL: 273 of 286 classify as `skin`; 13 do not (`tools/4-loot-trade/autobundle_cls.txt`). The same check across all 291 bestiary skin values finds 36 with no `skin` type, among them real ones such as *rolton eye*, *glossy kiramon chitin*, *chipped troll tusk*, *ki-lin horn*, *tailspike*, *caribou antlers* (the rest are bestiary spelling defects such as *cobra's*, *ear*, *tawny brindlecat hide.*) | MEDIUM, M6 |
| collectcheck.lic:17, collectible.lic:8-33 | collectibles | 93 expanded names | name | `gameobj-data.tsv:15` `type collectible` | HAVE: all 93 classify `collectible` (`classify.py collect_names.txt`) | - |
| xmlpatch.lic:125-209 (LostRanger 0.3.2, 2019); GameObjAdd.lic:94-107 | extra GameObj types | 9 types | name / noun / exclude regex | `gameobj-data.tsv` (68 type and sellable categories) | HAVE for `moonshard` (as `quest` + `event:duskruin`), `reim` gems (`realm:reim`), `duskruin`, `noncorporeal`, `breakable`, the Sanctum gems, *scepter*/*plate*/*pitcher* jewelry, *ur-barath*/*glimaerstone* excludes (each found with grep). GAP: `companion` (ranger companion nouns and an exclude of wild creatures), `familiar` (wizard familiar names), `pennant chase` (gem name pattern), `phased` (`/^shifting /`) | MEDIUM (pets, M6), LOW (the rest) |
| gems_2jars.lic:7-102, cantrips.lic:313-420, pure.lic:9 family (`gemdb`) | gem name fragments and nouns | ~95 / ~60 / 70 | name or noun | `gameobj-data.tsv:23-25` `type gem` | HAVE: 19 of the 23 nouns tested classify `gem` by noun alone. `caederine` and `galena` are classified by full name (*some waxy grey caederine*, *silvery galena*); `rhimar` classifies `uncommon` (a metal); `egg` only through full names | - |
| skintracker.lic:27 | skin quality ladder | 8 | ordered grade | `ledger/town.rs:59` (`is of \w+ quality`, grade not kept); `bounty.rs:408` (`quality` as a string) | GAP | MEDIUM, M8 |
| payroll.lic:74-92 (Bhuryn 0.28) | bank names and aliases | 17 banks | code, listing names, room names | `state/bank.rs:207-214` (`local`, Lich's `local_bank`, by the room's location), `bank.rs:333` (`town_of`) | PARTIAL: no alias table. The Hinterwilds (*Brindlestoat's, Moneylender*), Evermore Hollow (*Otherworldly Wealth*), Seareach (*Imperial Counting House*), Flotilla (*The Drifting Depository*), Nielira Harbor and Sailor's Grief (*The Contempt*) spell it their own way, and matching by location is UNVERIFIED for them | MEDIUM, M6 bank |
| payroll.lic:93-111 | room title to bank | 17 | code, title regexes | as above | PARTIAL | LOW |
| reverb.lic:138-162 (LostRanger) | worn-location slot limits | 23 | location header, [functional max, total max] (e.g. pin 8/20, neck 3/6, belt 3/5, fingers 2/6) | none: `./cg.sh "(functional)" "Hung around your neck"` -> NONE | GAP | MEDIUM, M10 GUI inventory |
| enhcontainer.lic:30-55 | inspect wear phrase to location label | 24 | phrase, label | none | GAP | LOW |
| inspect.lic:37-66 (LostRanger 0.7); inspectweight.lic:33-56 (2014, "GM Vanah's info"); capacity.lic:17-49; capacityall.lic:61-77; sizematters.lic:16-32 | container capacity words to pounds; item-count words to counts | 17 + 6 | word, range | exact numbers from `<inventoryManager>` `in_max` (`cena-protocol/src/frame/payload.rs:321-324`, decoded `Capacity`) | GAP for the prose, LOW because the snapshot gives numbers. The sources disagree: inspect.lic, inspectweight.lic and capacityall.lic give *small 5-7, fairly small 8-11, somewhat small 12-15*; capacity.lic and sizematters.lic give *small 5-9, fairly small 10-14, somewhat small 15*. The item count *a number* is 7-9 (inspect.lic) or 7+ (inspectweight.lic) | LOW |
| inspect.lic:73-116 | armor groups, ASG names, ASG base weights | 5 / 20 / 20 | group, ASG, name, lbs | `data/armor.tsv` (20 rows) | CONFLICT: full plate is **70** lbs in `inspect.lic:115` (`ASG_WEIGHTS[20]`, table at `:109-116`) and **75** in `armor.tsv:21`; the other 16 stated weights (ASG 2 and 5-19) agree. inspect prose names the groups *soft leather* and *rigid leather*, while `armor.tsv` names them `leather` and `scale`; no map between the two exists (`./cg.sh "armor that covers"` -> NONE) | LOW |
| lmapp.lic:7-37 (Drafix) | lockpick strength and precision words | 10 + 18 | word, rank or modifier, materials | none (`./cg.sh "unsurpassed" "highly accurate"` -> NONE) | GAP | LOW (no picking behavior; the pool is used) |
| weightcalc.lic:31-57 | race body weight, weight factor; capacity formula; *160 silver is 1 pound* | 13 races | race, base lbs, factor | none (`./cg.sh "Burghal Gnome"` finds only `cena-map/src/cond.rs`) | GAP | LOW-MEDIUM (M6: coin weight when the round banks at 80% encumbrance) |
| stupid-deed.lic:73-88 | shop price modifiers | 3 | trading bonus + INF bonus over 12, capped by ranks; Dark Elf and Forest Gnome -5, Half-Krolvin -25 | none | GAP, UNVERIFIED (the script says "assume Wehnimer's only") | LOW |
| badge2.lic:49-107 (Ponclast 1.4) | Adventurer's badge tiers | 5 parts x 11 tiers x 3 words | material word, tier | none | GAP | LOW |
| colmaster.lic:559-711 (Dreaven v46) | CoL skinning tasks | 25 | skin, creature, level, room ids, boundary rooms | `creatures.tsv` col 39 `skin` (e.g. `cave_troll` -> *a troll skin*, `dark_orc` -> *an orc ear*) | HAVE for skin to creature; rooms GAP | LOW |
| colmaster.lic:220-557; coltv.lic:24-154; inquisition.lic:8-192 | CoL Inquisitor Q&A | 336 / 127 / 183 (`grep -c` of `=>` rows) | question, answer | none | GAP | LOW |
| colmaster.lic:713-725 | CoL gift items | 11 | item, cost, order number, shop room | none | GAP | LOW |
| MoonshardBundle.lic:20-71 | Duskruin moon piece pairs | 9 pairs, 3 moons | piece, piece, moon | pieces classify (`event:duskruin`) | GAP (combinations) | LOW |
| update-playershops.lic:632-1087 | player-shop map areas | ~375 | map id, xygon id per town | none | GAP | LOW |
| cobble.lic:29-65 | cobbling workshop rooms | 37 | door adjective, room id | none | N/A (crafting) | - |

## Captures Cena lacks or differs on

| script:line | fact | sample text or regex | Cena counterpart (file:line) | verdict | value |
|---|---|---|---|---|---|
| gemseller.lic:30, gemvalue.lic:22, stupid-deed.lic:87 | shop appraisal with no `silver` word | `you ? for it if you want to sell`; `/I'll give you \d+ for/`; eloot.lic:5910 also `for this if you'd like` | `ledger/town.rs:72-76` all need `silver` | PARTIAL (Top 1) | HIGH, M6 |
| bank.lic:36, tbank.lic (Dantax) | withdrawal | `Very well, a withdrawal of ([\d,]+) silver`; `teller scribbles the transaction into a book and hands you ([\d,]+)` | `town.rs:96` has only the *carefully records* form | PARTIAL | HIGH, M6 |
| bank.lic:36; colmaster.lic:1280 | withdrawal, *and then* form | `teller carefully records the transaction, (?:and then )?hands you`; colmaster waits for `/and then hands you .* silvers?\./` alone, at the Solhaven bank (its message at :1284) | `town.rs:96` `,? (?:and )?hands you` does not match *, and then hands you* (Python `re` test) | CONFLICT | HIGH, M6 |
| bank.lic:36 | deposit forms | `That's ([\d,]+) silvers? to your account`; `You deposit your note worth ([\d,]+) into your account`; `They add up to ([\d,]+) silvers?` | `town.rs:95` (`You deposit N silvers into your account`), `:97` (`That's a total of N silvers, bringing your balance to`) | PARTIAL | HIGH, M6 |
| bank.lic:36 | note withdrawn at a fee (Teras) | `scrip for ([\d,]+) silvers, with a ([\d,]+) silver fee for the scrip` | none (`./cg.sh "silver fee"`: NONE) | GAP | MEDIUM, M6 |
| bank.lic:36; vaalor_document.lic:53 | debt collected before a withdrawal | `I have a bill of ([\d,]+) silvers presented by your creditors`; `The local debt collector` | none (`./cg.sh "I have a bill of" "debt collector"`: NONE); Lich `bank.rb:46` `DEBT` | GAP | MEDIUM, M6 |
| payroll.lic:1080; duskrunner.lic:92; withdrawall.lic:55 | teller's `check balance` | `Your balance is currently at ([\d,]+) silvers.` | none (`./cg.sh "Your balance is currently"`: NONE) | GAP | MEDIUM, M6 bank |
| payroll.lic:1085 | balance after a transaction | `This brings your total to ([\d,]+) silvers.` | only as text in a test fixture (`cena-model/tests/loot_facts.rs:135`); the figure is not read | GAP | MEDIUM |
| payroll.lic:1011-1061 | bankbook read | `You study your current bank balances...`; `(.+?): ([\d,]+) silvers`; `Your total silvers: N` | `state/bank.rs:342-345` opens only on `bank account` | GAP | LOW |
| duskrunner_support.lic:23; duskrunner.lic:69 | note with a surcharge | `He scribbles out a City-States promissory note for (.*) \(minus a small (.*) silver surcharge\) and hands it to you.`; `She then hands you a City-States promissory note for ([\d,]+) silvers, minus a small (.*) silver surcharge.` | `town.rs:68` `pawn_note`, `:78-80` `gemshop_note` (neither matches; Python `re` test) | CONFLICT | HIGH, M6 |
| duskrunner.lic:69; dusk_sell.lic:41 | pawnshop resell confirm | `To accept the offered price, attempt to resell it again within the next 30 seconds.` | none (`./cg.sh "accept the offered"`: NONE). The offer before it is read (`town.rs:72`, *I'll offer you N silver and no more!*) | GAP | MEDIUM, M6 |
| unload.lic:31 | pawnshop refusal (Icemule, Cendadric) | `Not my line, really.` | `town/reply.rs:64-71` `WrongShop` (6 phrases, not this one) | PARTIAL | MEDIUM, M6 |
| appskin.lic:74-75 (Dalz 2023) | furrier appraisal | `.+I'll pay you (?<amount>[\d+,]+) silvers for it.` | `town.rs:72` `(?:give\|offer)` only | PARTIAL | MEDIUM, M6 |
| dr.lic:84-105 | Duskruin appraiser | `How's \d+ silvers sound`; `I already quoted \d+`; `that's worth at least (\d+)`; `I've no use`; `but I'll offer you 35,000` | `town.rs:72` covers *I'll give you N silvers* | PARTIAL | LOW (event) |
| skintracker.lic:67 | skin appraisal grade | `(crude\|poor\|...\|magnificent) quality and worth .* (\d+) silver` | `town.rs:59` matches but keeps only the value | PARTIAL | MEDIUM, M8 |
| gibs_purify.lic:112; PurifyL.lic:176-246; purifyjars.lic:57-63 | loresong purify outcomes | `turn as the very essence` (orb); `gem becomes more perfect`; `appearing smoother and more pure`; `improves somewhat`; `crack loudly and strain`; `cannot be purified any further`; `shatter into thousands of fragments`; `you feel it trying to draw power from you` (orbsort.lic:65) | shatter HAVE (`town.rs:64`), the loresong value HAVE (`:62-63`); the rest none (`./cg.sh "very essence" "improves somewhat"`: NONE) | GAP | LOW (bard) |
| chargeitem.lic:161-162; backroom-auto.lic:65-99; gforgeb.lic:610-622 | merchant `order`/`buy` | `Sold for ([0-9]+) silver`; `But you do not have enough silver!`; `accept only local notes`; `There is no merchant here to order anything from.`; `You may order a QUANTITY of this item\|no such item`; `There is nobody here to buy anything from.` | `travel/routines/shopping.rs:314-324` checks *hands you* only; *Looks like you might buckle* HAVE (`giant.rs`) | GAP | MEDIUM, M6d/M8 |
| chargeitem.lic:131; toxicologist.lic:355 | shop catalog | `<d.*?cmd=["']order ([0-9]+).*?>(.*?)<\/d>` | none | GAP | MEDIUM, M6d |
| gshop.lic:123; update-playershops.lic:424 | player-shop price | `The.*will cost ([0-9,]*) coins\.`; `Looking .* will cost N coins` | none | GAP | LOW |
| collect_buy.lic:68 | consignment price | `The selling price is (\d+) silvers` | none | GAP | LOW (`plan/31` §4d consignment) |
| mugreport.lic:216-391 (Demandred 2024.10.22) | rogue MUG loot | `You rifle X pockets and discover ([,\d]+) silvers?!`; `X had Y tucked away!`; `X didn't hide Y well enough!`; `You find Y on X!`; `X left Y behind.` | the attack line HAVE (`combat_attacks.tsv`: *larcenous intent*, *down for hidden valuables*); the loot none (`./cg.sh "pockets and discover" "tucked away"`: NONE) | GAP | MEDIUM, ledger |
| mugreport.lic:358 | heirloom found | `You spy (a\|an\|some) Y, which looks like the heirloom that you are searching for!` | the heirloom *task* HAVE (`bounty.rs:390`); the find none | GAP | MEDIUM, M8 |
| gemstone-tracker.lic:1469 | search scatters loot | `Your hasty search scatters the ([\w\s]+?)'s riches to the floor!` | `ledger/hunt.rs:37` (`You search the`) | GAP; what it means is UNVERIFIED | MEDIUM, M6 ledger |
| gemstone-tracker.lic:1441-1445 | Ascension jewel find | `** A glint of light catches your eye, and you notice .* at your feet! **`; `... at ([a-zA-Z]+)'s feet!` | the own-feet form HAVE (`hunt.rs:49`); the other-player form none | PARTIAL | LOW |
| gemstone-tracker.lic:1479-1503; gems.lic:40-76; gem_loadouts.lic:75-91 | Ascension gemstone properties | `Property:    (.*)`; `Rarity:      (.*)`; `You note the telltale filaments of a lesser binding`; `Gemstone (\d+): ... (equipped)`; `will apply the lesser binding to it` | none (`./cg.sh "Rarity:" "telltale filaments"`: NONE) | GAP | LOW-MEDIUM |
| badge2.lic:33; badge.lic:13 | lifetime bounty points | `accumulated a total of ([\d,]+) lifetime bounty points` | `bounty.rs:230` reads the task line of `bounty`, not this | GAP | MEDIUM, M8 |
| sparklies.lic:122-253 | gem-dealer bounty, bounty boost | `You can SELL them to me as you find them.`; `You already have an active Bounty Boost\|You do not have any Bounty Boosts to redeem\|You have activated a Bounty Boost`; `Hmm, I've got a task here from the town\|got a special mission for you` | none (`./cg.sh "Bounty Boost"`: NONE) | GAP | MEDIUM, M8 |
| dont-lose-it.lic:97-139 | `inventory full` and `glance` | `You are currently wearing and carrying:`; item lines with `(registered)` / `(marked)`; `(\d+ items? displayed.)`; `You glance down to see X in your right hand and Y in your left hand.` | hands HAVE from `<right>`/`<left>` frames (`state/hands.rs`); the listing and its flags none | GAP | MEDIUM, M6 |
| colmaster.lic:1070-1078 | `mark` result | `Your .* has been (permanently )?marked as unsellable`; `Your .* is not marked as unsellable` | none | GAP | MEDIUM, M6 |
| reverb.lic:136-137 | `inventory location` | `You are currently wearing:`, group headers, `(functional)` | none | GAP | MEDIUM, M10 |
| update-playershops.lic:591-594; htsorter.lic:153-164; sorter.lic:102-120 | `look in` prose | `Peering into the X, you see ...`; `The X bezel setting is empty.`; `There is nothing affixed`; `shut too tightly to see its contents`; `It is thoroughly colonized with mold`; `The X appears to be empty.` | containers come from `<container>`, `<inv>` and `<clearContainer>` frames (`state/inventory.rs:1-37`) and the snapshot; `shopping.rs:156-206` reads *That is closed*, *There is nothing* | PARTIAL; the structure is better than the prose | LOW |
| capacity.lic:13; inspect.lic:155; pawnstar.lic:319 | `inspect` capacity | `You estimate that (.*) can store (.*) with (.*)` (old); `It is estimated that .* can store (a\|an)` | numbers from the snapshot (`payload.rs:321-324`) | PARTIAL | LOW |
| pawnstar.lic:118-364 (Maze) | `inspect` properties | ~30 shapes: `It has (\d+) charges remaining`, `It imparts a bonus`, `It protects against magical attacks`, `It has been infused with the power of a`, `It appears to weigh`, `mainly crafted out`, `weighted to inflict more critical wounds`, `padded against`, `It provides a boost of`, `can be worn`, `requires skill in` | kept as raw prose in `ItemDetail` (`payload.rs:471-480`), not classified (`./cg.sh "It imparts" "charges remaining"`: NONE) | GAP | MEDIUM (no milestone; M10 inventory) |
| enhcontainer.lic:83-95 | enhancive `recall` | `a boost of (\d+) to (.*) (Bonus\|Base\|Rank)` | `character/enhancive.rs` reads the totals, not one item's recall | PARTIAL | LOW |
| 704monitor.lic:167-190; phaseorb.lic:60; rephase.lic:60 | Phase (704) on containers and gems | `The (.+) becomes (momentarily\|somewhat) insubstantial and appears lighter.`; `Your (.+) feels somewhat heavier.`; `resists the effects of your magic` | none | GAP | LOW |
| CIDig.lic:79; dr_dig.lic:184; riftboxes.lic:19 | trash confirmations | `If you wish to continue, throw the item away again within fifteen seconds.`; `There appears to be an item or items attached to the container you are throwing away that is either scripted or of significant value.`; success `you feel pleased with yourself at having cleaned up the surrounding area` | `town/pool.rs:175-186` (trash, then drop, then back to a bag, whatever the reply) | GAP | MEDIUM, M6 4c |
| tdig2021.lic:264; DRDIG.lic:218-221; digdug.lic:356 | `empty X into Y` | `but nothing will fit.`; `but nothing comes out.`; `and everything falls in quite nicely.`; `but you can't quite get several items to come out.` | none | GAP | LOW |
| coinpouch.lic:36-37; silvers.lic:7 | coin containers | `There is only room for (.*?) more coins`; `is already full`; `Inside the coin bag you see approximately (.*) silver coins.` | `character/currency.rs:114` (`silver stored within your`) | PARTIAL | LOW |
| fun-with-dreaven-video-1.lic:57-138; saccept.lic:137; offercheck.lic:22; giveall.lic:11 | player offers | `X offers you Y.  Click ACCEPT to accept the offer or DECLINE to decline it.  The offer will expire in 30 seconds.`; `X just gave you (\d+) coins which you quickly pocket.`; `X has accepted your offer and is now holding Y.`; `X has declined the offer.`; `Your offer to X has expired.`; `You offer your (.*?) to (.*?), who has 30 seconds to accept the offer.` | none (`./cg.sh "has accepted your offer" "Click ACCEPT to accept"`: NONE) | GAP | MEDIUM, M5/M6 (moving loot between one's own characters) |
| reserve.lic:247, :507, :736 | herb `reserve` | `You reserve your .* for use in combat.`; `You remove .* from your reserves and hold it in your hand.`; `contains DOSEs`, `contains TINCTUREs` | none (`./cg.sh "reserve your" "from your reserves" "contains DOSE"`: NONE) | GAP | MEDIUM, M6d |
| flaskbundle.lic:42 | potion doses | `You have \d+ doses left.\|You only have one dose left.\|That was the last drop.\|You can't pour any more` | none | GAP | LOW-MEDIUM, M6d |
| mail.lic:28-119 | mail and COD | `has been marked for payment upon delivery at a cost of (.*) silvers? by its sender`; `They disappear for a little while before returning with .*, which they hand to you.` | none | GAP | LOW |
| deliver.lic:19-40 | Simucoin delivery | `CONFIRM that you wish to have this item delivered`; `you glance down and see that you now have` | none | GAP | LOW |
| countpaidentries.lic:47-68; countbooklets.lic:43 | event entries | `(\d+) of \d+ stamped vouchers remaining`; `(\d+) entries left` | the event currencies HAVE (`character/currency.rs:155-163`); these counts none | GAP | LOW |
| star-charges.lic:91 | resource line | `Covert Arts Charges: N/N` | HAVE (`character/standing.rs`) | HAVE | - |
| backroom-auto.lic:141-161; collect_buy.lic:178; flag-simulator.lic:185-190; unloadlinmur.lic:213 | old `wealth` forms | `You have no silver coins with you.`, `You have but one coin with you.`, `You have (\d+) coins with you.` | Cena reads the current `You have N silver with you.` (`currency.rs:103-110`, as Lich `infomon/parser.rb:74`); backroom-auto.lic v8 notes the change of wording | HAVE (current wording); the old forms are obsolete, INFERRED | - |
| sammo.lic:559; chargeitem.lic:59-72; sparklies.lic:85-87; giveaway.lic:18-20; depocoins.lic:10; passcoinsto.lic:10; reverb.lic:339 | silver from `info` | `^\s*Mana:\s+-?[0-9]+\s+Silver:\s+([0-9,]+)` (7 scripts read silver this way, silently) | `character/stats.rs` reads `info` stats; the silver field is not read (`./cg.sh "Silver:"` -> NONE). `wealth` HAVE (`currency.rs:103-110`) | GAP | LOW |
| toxicologist.lic:241-254 | `wealth` notes | `(?:bond )?notes valued at a total of ([\d,]+) silver`; `carrying (?:a total of )?([\d,]+) silver between notes and coins` | Cena reads `Total note value: N` and `You are carrying a total of N silver.` (`currency.rs:130-137`, Lich `parser.rb:76-77`) | CONFLICT, UNVERIFIED (toxicologist's patterns look defensive, not quoted from the wire) | LOW |

## Script by script

| script | lines | purpose | captures / data / uses | verdict summary |
|---|---|---|---|---|
| colmaster.lic (Dreaven v46) | 1440 | Council of Light join and master, Solhaven | Inquisitor Q&A hash `answers` (:220-557, 336 rows, `grep -cE "=> *\{ *:a"`); `critter_info` skin tasks (:559-711, 25 rows: skin, critter, level, rooms, boundary rooms); `item_info` gift items (:713-725, 11 rows: cost, order number, shop room); COL task lines (:1100-1227); `mark` replies *has been (permanently) marked as unsellable* / *is not marked as unsellable* (:1070-1078); *and then hands you N silvers* withdraw (:1281) | Skin->creature HAVE (`creatures.tsv` col 39 `skin`, e.g. cave troll -> *a troll skin*); COL task lines PARTIAL (`societies/membership.rs:119` has the High Taskmaster line only); Q&A, gift table GAP LOW; unsellable mark GAP MEDIUM |
| gforgeb.lic (Gwrawr 1.2b) | 1319 | artisan forging (perfects) | forge/grinder/trough lines; `buy` replies *Sold for / But you do not have / accept only local notes* (:610-622); workshop rent *the clerk collects / have enough silver* (:641); note noun per bank: `chit` Icemule, `scrip` Kharam-Dzu (:688-690) | Crafting N/A for M6-M8. Note nouns HAVE (`state/bank.rs:54` `NOTE_NOUNS`). Merchant `order`/`buy` replies GAP (see Captures) |
| slich.lic (SpiffyJr 1.0) | 597 | Lich overrides: `empty_hand`, `fill_hand`, `move` | get/put/wear replies (:77, :117, :26 *won't fit*, *It's closed!*); move failure union (:446-566) | Move failures HAVE (`cena-model/src/movement.rs`: *rusty doorknob*, *silvery thread*, *wobbles briefly*, *fast-moving river*, *may only type ahead* all found); *won't fit*/*closed* HAVE (`loot/outcome.rs:89-93`); *Why don't you leave some for others?* GAP LOW |
| cobble.lic (Dreaven v54) | 991 | artisan cobbling | workshop rooms by door adjective (:29-65, 37 rows); rent/buy/withdraw replies (:271-354) | Crafting N/A; `buy` *you do not have enough silver* GAP (merchant) LOW |
| coltv.lic (Fib 0.2) | 429 | CoL task helper | `qna` hash (:24-154, 127 rows); *present the Master of Services with (\d+) more (.*)* (:366); *present the Master of Gifts with ...* (:391); `order` catalog *(\d+)\. (a\|an\|some) item* (:225) | COL tasks GAP LOW (society quest, no milestone) |
| witchhunt.lic | 644 | Ebon Gate witch hut tasks | seven task names and completion lines (:120-580) | Event N/A |
| ezmove.lic (Steworaeus 0.5) | 608 | move gems/reagents to jars on alts | `look in` jar *Inside .*? you see N portion* (:449-450); `_drag` to jar *You add ... filling it / is full / does not appear to be a suitable container* (:304-321); `rummage ... ingredient` (:266) | Jar hoarding GAP MEDIUM (`plan/31` §4d: Hoard not ported). Searched: `./cg.sh "portion" "filling it" "suitable container"`: *portion* hits only unrelated text (`loot/outcome.rs`, three data TSVs), *filling it* only `character/enhancive.rs` (unrelated), *suitable container* NONE |
| xmlpatch.lic (LostRanger 0.3.2, 2019) | 262 | patches Lich's `GameObj` type data | adds types `moonshard`, `reim`, `duskruin`, `pennant chase`, `noncorporeal`, `companion`, `familiar`, `breakable`, `phased` (:131-240) | Mostly HAVE: current `gameobj-data.tsv` carries moonshards (`quest`,`event:duskruin`), Reim gems (`realm:reim`), `noncorporeal`, `breakable`, the Sanctum gems. GAP: `companion` (ranger companions, noun list + exclude), `familiar` (wizard familiars), `pennant chase` gems. See Data tables |
| gems_2jars.lic | 452 | gems to jars in a house | `gem_patterns` (:7-102, ~95 gem name fragments); waitfor put/get replies | Gem nouns HAVE (gameobj `gem`); jar flow GAP (Hoard) LOW |
| payroll.lic (Bhuryn 0.28) | 1344 | per-character bank balances | `bank account` (:1020-1074) and bankbook (*You study your current bank balances...*, *X: N silvers*, *Your total silvers: N*, :1011-1061) readers; *Your balance is currently at N silvers.* (:1080); *This brings your total to N silvers.* (:1085); Lumnis schedule lines (:1029-1119); `PAYROLL_BANK_COLUMNS` 17 banks x aliases (:74-92); `PAYROLL_ROOM_BANK_PATTERNS` (:93-111) | `bank account` HAVE (`state/bank.rs`, split on last `": "`); bankbook, teller balance, post-transaction balance GAP MEDIUM; bank alias table PARTIAL (see Data tables) |
| cantrips.lic (Jymamon 2015) | 533 | utility library | `gems` jarrable-gem patterns (:313-~420); `forage` replies (:196-211); `silver-cost:` map tag reader (:160) | Gem nouns HAVE; forage LOW (M6d herbs) |
| sammo.lic (SpiffyJr) | 807 | ammo fletching/refill | fletching steps; crossbow load counts (:476-491); `order`/`buy` *But you do not have enough / hands you* (:685) | Crafting N/A |
| update-playershops.lic / update-playershops_commas.lic (Tillmen/Xanlin 0.23/0.28) | 1294/1327 | scrape player shops to a website | `look in` reply union (:621-624: *bezel setting is empty*, *There is nothing affixed*, *shut too tightly to see its contents*, *thoroughly colonized with mold*, *appears to be empty*); *Looking .* will cost N coins* (:440); *It is estimated that X can store* (:514) | Player shops LOW (no milestone). `look in` prose variants GAP LOW (Cena reads containers from `<container>/<inv>` frames, `state/inventory.rs:1-37`) |
| inquisition.lic (Gibreficul) | 192 | CoL Inquisitor Q&A | 183 Q&A rows (`grep -cE` of `"..." =>`) | GAP LOW |
| GameObjAdd.lic / GameObjAddMore.lic | 108/66 | earlier xmlpatch halves | same patches as xmlpatch; GameObjAddMore adds box adjectives/materials (`fel`, `haon`, `maoral`, `modwir`, `monir`, `tanik`, `thanot`, `wooden`) and `phased` = `/^shifting /` | See xmlpatch. Box adjectives and materials HAVE: `gameobj-data.tsv:12` `type box name` covers all 14 materials and more (cherrywood, mahogany, rolaren reliquary); `phased` GAP LOW (the box regex already admits `shifting`) |
| reserve.lic | 1135 | herb RESERVE command manager | *You reserve your X for use in combat* / *You remove X from your reserves* (:247, :736); herb container *contains DOSEs / TINCTUREs* (:507) | GAP MEDIUM (M6d heal); `./cg.sh "reserve your" "from your reserves" "contains DOSE"` -> NONE |
| toxicologist.lic | 850 | rogue poisoncraft vials | `wealth` variants *notes valued at a total of N silver*, *carrying (a total of) N silver between notes and coins* (:241-254); `order`/`buy` replies (:404-422) | wealth: Cena reads Lich's three forms (`character/currency.rs:103-137`, Lich `infomon/parser.rb:74-77`); these two variants UNVERIFIED (loose regexes). Rogue craft N/A |
| mission-gemstone.lic (Dreaven v8) | 714 | Mission Gemstone quest runner | quest step table `@mission_steps` | Quest N/A |
| vaalor_document.lic | 281 | buy Ta'Vaalor papers | withdraw replies *have that much in the account / here ye go / The local debt collector* (:53) | Bank debt GAP (see Captures) |
| widgetsearch.lic (Tysong 1.0.1, 2025) | 210 | Duskruin brass gear themes | *The theme for this is "(\d+) -* (:143); gear upgrade lines | Event LOW |
| finvale_crystal.lic | 352 | tarot crystal readings | 78 card texts (:12-322) | N/A |
| dr.lic | 391 | Duskruin arena loop | arena merchant appraise *I've no use / How's N silvers sound / I'll give you N / but I'll offer you 35,000 / Can't say / I already quoted N / that's worth at least N* (:84-105) | *I'll give you N* HAVE (`ledger/town.rs:72`); the other five GAP LOW (Duskruin) |
| Autobundle.lic (Mindl v2, 2024-06-07) | 573 | bundle hides by tens, arrows by 24 | `hides` list (:29, 286 names); bundle lines *You glance through your bundle and count a total of*, *You carefully add your*, *You carefully arrange your*, *You bundle your* (:47-79) | Bundle create/add/arrange HAVE (`ledger/hunt.rs:52-56`); the bundle count and the arrow forms GAP LOW; hides list see Data tables (Top 4) |
| badge.lic / badge2.lic (Ponclast 1.4) | 180/199 | Adventurer's badge upgrade calculator | lifetime bounty points (:33); badge tier words (:49-107); treasure master `ask ... about change` | Lifetime points GAP MEDIUM (M8); tiers GAP LOW. badge2 fixes badge's typos (*Your have been tasked*, *moneybag*) |
| tdig2021.lic / tdig2020.lic (Docktoer, after Tysong) | 572/508 | Ebon Gate digging | dig and shovel lines; `empty X in Y` replies (:264); seashell bucket `rummage` (:231-233); `look seashell` *It is worth N seashells.* (:243); *You cannot hold any more silvers.* (:315) | Coins-full HAVE (`loot/outcome.rs:145`); the rest event N/A |
| chargeitem.lic (Althias, Doug, Aethor 1.0.8) | 306 | charge magic items with orbs | `info` silver (:57-72); `order` catalog links (:131); *Sold for N silver* / *But you do not have enough silver!* (:161-164); orb rub outcomes (:265-277) | Buy replies GAP MEDIUM (Top 7); orb charging LOW |
| qskin.lic (Daedeus 2015, from sloot) | 385 | skinning | skin replies (:330-333) incl. *You break through the crust of the X and withdraw Y!*; put failure *find there is no space for the* (:198) | Skin outcomes HAVE (`loot/outcome.rs:111-131`); *no space for the* GAP LOW |
| sparklies.lic | 264 | gem-bounty helper | gem-name normaliser patterns (:31-53); `rummage ... ingredient`; *You can SELL them to me as you find them.* (:145); `boost bounty` replies (:122) | Bounty lines GAP MEDIUM (M8) |
| dont-lose-it.lic (Tgo01 v8) | 292 | guard tracked items against put/drop | `inventory full` listing and flags (:97-123); `glance` hands (:129-139) | Listing GAP MEDIUM (Top 9); hands HAVE from frames |
| potato.lic (Tsalinx 0.6) | 264 | care for a potato pet item | potato status words; bury terrain list | N/A |
| gemstone-tracker.lic (Dreaven) | 1517 | Ascension gemstone find odds | search and mug lines (:1425-1469); jewel find (:1441-1445); `Property:`/`Rarity:` (:1479-1503) | Jewel find HAVE (`ledger/hunt.rs:49`); *hasty search* and properties GAP (see Captures) |
| dr_dig, digdug, sdig, beldig, duskdig, duskdigbin, digdusk | 103-479 | Duskruin digging | pickaxe, coffin, skeleton lines; archaeologist `give` *I can buy that / I don't want anything* (sdig.lic:115); piece lists to sell or trash | Event N/A; trash confirm GAP (Captures) |
| egdig, egdig2, xegdig, egdigger, digger, EgDigForOldLich, ddig, ddigmac, vdig, CIDig, DRDIG, DSD, dsdprime | 32-365 | Ebon Gate / Caligos digging and diving | `match` lists of dig outcomes; *throw the item away again within fifteen seconds* (CIDig.lic:79) | Event N/A |
| reverb.lic (LostRanger) | 531 | verb conveniences | `inventory location` slot table (:138-162); locker lines (:442) | Slot table GAP MEDIUM (Data tables) |
| gshop.lic (Gwrawr) | 429 | buy from player shops | `shop browse`/`shop purchase` flow (:70-301): *will cost N coins*, *to cover the cost of the purchase*, *The urchin returns after a while*, *You currently have no contract* | Player shops GAP LOW |
| pawnstar.lic (Maze) | 574 | inspect every item on a pawnshop table | ~30 `inspect` shapes (:118-364) | Inspect vocabulary GAP MEDIUM (Captures) |
| mtrawl.lic (Mara, co-author Claude) / trawl.lic (Alastir) | 413/185 | Nyessem mana trawling | `waggle pool` outcomes; aevarch entries | Event N/A; aevit currency HAVE (`currency.rs:163`) |
| ebon_arena.lic (Lucullan 2024) | 622 | EG arena loop | `Field Exp: N/N` via quiet `exp` (:255); arena announcer lines | Field exp HAVE (`character/experience_report.rs`, `./cg.sh "Field Exp"`); event N/A |
| gritapply.lic (Rinkidinkicus 1.3.1, 2026) | 563 | warrior grit on weapons | `Suffused Grit: N`, `N/N (Total)`, *need at least N suffused energy* (:219-265) | GAP LOW (no warrior behavior) |
| ph_heist.lic | 310 | Poisoned Heretic heist event | hide/watch/sneak lines | N/A |
| gemscounter.lic (Nisch 1.0.0) | 370 | count gems in jars in a locker | locker open, `look in` jars (:84-129) | Jar count GAP (Top 6) |
| stupid-deed.lic / gems2deeds.lic | 640/169 | Lorminstra deeds from cheap gems | price modifier (:73-88); *I'll give you N for* (:87); `Level: N Deeds: N` (:39); *thy deed has been recorded* (:631); *Thy offering pleases the Goddess* (gems2deeds.lic:103) | Deeds count HAVE (`experience_report.rs`, `./cg.sh "Deeds:"`); deed ritual GAP LOW; appraisal form see Top 1 |
| gearinfo.lic / record.lic / itemnote.lic | 407/156/141 | notes on held items | `inv hands` and `glance` parsing | N/A (player notes) |
| collect_buy.lic (Tysong 1.0) | 225 | buy collectibles from consignment | *The selling price is N silvers* (:68); town names (:93-109) | GAP LOW |
| fun-with-dreaven-video-1.lic / fun-with-dreaven-video-2.lic (Tgo01) | 247/237 | demo scripts | player offer/accept lines; scroll infusion lines | Offers GAP MEDIUM (Captures) |
| unloadlinmur.lic / linmurunload.lic | 246/245 | personal unload script | old `wealth` form (:213) | N/A |
| inspect.lic (LostRanger 0.7), inspectweight.lic (Oweodry), capacity.lic (Tgo01 v6), capacityall.lic (Soliere), sizematters.lic | 57-217 | add numbers to `inspect` | capacity words, armor ASG (Data tables) | CONFLICT on full plate; the rest LOW |
| lmapp.lic (Drafix) | 71 | `lm appraise` of lockpicks | *You examine the X very closely... an? (.+) level of precision and (?:has\|is) (.+?) strength* (:42) | GAP LOW |
| orchear.lic (Hazado 0.1) | 104 | orcspeak translator | word table | N/A |
| enhcontainer.lic | 152 | list enhancives in a container | wear phrases (:30-55); recall boosts (:83-95) | GAP LOW |
| puzzlebox.lic (Demandred) | 158 | solve the mithril puzzlebox | dial lines | N/A |
| jailboxthatworks.lic / jailbox.lic | 91/44 | recover the possessions box after arrest | box by name | N/A |
| purify.lic (Jymamon 2015), pure/pure2/pure3/pure4/pppure.lic (Gibreficul after Shaelun), purifyjars.lic | 131-287 | bard purify | same `matchwait` set; `pure*.lic` differ only in the loresong verse and gem list (`diff pure.lic pure2.lic` -> one line) | See gibs_purify row |
| 704monitor.lic / rephase.lic / phaseorb.lic / phaser.lic / dephase.lic | 18-296 | Phase (704) on containers and gems | phase lines | GAP LOW |
| backroom-auto.lic (Dreaven v8) | 271 | buy until backroom access | `order`/`buy` replies (:65-99); old and new `wealth` forms (:141-161) | Buy GAP MEDIUM (Top 7) |
| gemstones.lic (Alastir 2025) | 304 | Mission Gemstone status | `mission gemstone` regex | N/A |
| murderverses.lic (Luxelle 1.2) | 154 | bard loresong kill points | `recall` status words (:52-137) | GAP LOW |
| weightcalc.lic | 103 | exact carried weight by coins | race tables (Data tables) | GAP LOW-MEDIUM |
| htsorter.lic (2026-08, Saga), sorter.lic (Tillmen 0.9), windowsorter.lic, autosort.lic, auto_sort.lic (Azanoth) | 99-334 | sort `look in` output by type | `[IO]n the X you see`, `Peering into the X, you see` with container tags (htsorter.lic:153-164) | Cena uses frames; PARTIAL LOW |
| lootpackage.lic | 172 | Duskruin prize package | pawn prompt | N/A |
| doit.lic, gem-jars.lic (Nisch), shake.lic, rum.lic, jars.lic, appgem.lic, jarserve2.lic (0.2.3), jarserve_set_jarsacks.lic, jarcatch.lic (Mara 1.3.0) | 13-2306 | jar hoarding | portions, *is empty*, `shake`, `rummage`, *not a suitable container* | GAP MEDIUM (Top 6) |
| iron_maker.lic (Triex 2.04), ironslab.lic, ironslab_m.lic | 221-279 | Voln iron slab task | smelting lines | N/A |
| bloot.lic (Akono), lootbox2.lic, dunbox.lic, dropbox.lic | 62-86 | empty or drop boxes | `open` *It appears to be locked*; `get coins in #box` *You gather the*; trash nouns (lootbox2.lic:45); *Your disk arrives, following you dutifully.* (dunbox.lic:21) | Box open/gather HAVE (`loot/outcome.rs:139-143`, `ledger/boxes.rs:27`); disk arrival GAP LOW (`./cg.sh "disk arrives"` NONE) |
| foxhunt.lic / pixiehunt.lic / mandrake.lic (Alastir) | 108-201 | Rumor Woods events | `observe` and `forage` lines | N/A |
| sewers_shat.lic (Tysong 1.8), sewer.lic, sewer2.lic, duskruin_sewer.lic, duskruin_sewer2.lic | 178-221 | Duskruin sewer search | *You search around and find*, *wave of sewage* | N/A |
| spiritguard.lic (Mara 1.3.2) | 493 | rub small statues for 1712 | rub outcomes (:323-328) | N/A (magic items, pile 5/6) |
| enhance.lic / enhance2.lic (Tillmen, Tsalinx) / wearstuff.lic | 53-222 | wear and remove enhancives | put/open replies | N/A |
| gibs_purify.lic / purify2.lic / PurifyL.lic (Leafiara, 2025-03-29) | 178-298 | bard loresong purify gems | purify outcomes *turn as the very essence / gem becomes more perfect / shatter / crack / appearing smoother and more pure / improves somewhat / cannot be* (gibs_purify.lic:112); value *worth about N silvers* (:132) | Loresong value and shatter HAVE (`ledger/town.rs:62-64`); purify outcomes GAP LOW (bard only). Family: gibs_purify, purify, purify2, PurifyL, pure, pure2, pure3, pure4, pppure, purifyjars |
| shopgo.lic (Mara 1.5, 2026), go2shop.lic (LostRanger), findshop.lic, shoppin.lic (Vailan 2023), shopsearch.lic, whatsold.lic (Licel), shopnotes.lic | 60-500 | find, browse and track player shops | `shop browse` directory parsing; *bank account is currently* (shopnotes.lic:444) | Player shops GAP LOW |
| crimson_salt.lic | 438 | make crimson salt for Animate Dead | alchemy steps; *Total items:* `look in` | N/A (crafting) |
| gembuyer.lic | 298 | buy gems a gem bounty wants from jars | gem-name normaliser (:134-145); shop line `containing X for N silver` (:209); `XMLData.bounty_task` gem pattern (:171) | Gem bounty HAVE (`bounty.rs`, gem task); normaliser GAP LOW |
| mugreport.lic (Demandred 2024.10.22), exportmugstoyaml.lic, alltimemugging.lic | 28-411 | record MUG results and heirlooms | mug lines (:216-391) | GAP MEDIUM (Top 8) |
| containers.lic / auto_containers.lic (Drafix) | 117/62 | Psinet-style container aliases | typed bag names | N/A |
| duskrunner.lic / duskrunner_pawn.lic / duskrunner_support.lic | 174/155/33 | Duskruin loot sold in Ta'Illistim | sale union (:69) with the surcharge notes, resell confirm, *Maybe you can find a buyer*; `check balance` (:92) | Surcharge CONFLICT HIGH (Top 3); resell GAP; *Maybe you can find a buyer* HAVE (the tail of the worthless line, `town.rs:69`, per `loottracker.lic:673`) |
| zToT.lic (Ziled 0.2) | 106 | EG Trick or Treat | door knock lines | N/A |
| 125.lic (Dantax) | 221 | pop boxes with 125 after a 704 check | *becomes momentarily insubstantial*, *resists the effects of your magic*; justice-zone lines | N/A (boxes, pile 5) |
| appskin.lic (Dalz 0.0.1, 2023) / skintracker.lic (Greminty) | 226/205 | appraise skins for bounty value | *I'll pay you N silvers for it.* (appskin.lic:75); quality ladder (skintracker.lic:27); sell reply *would like to buy* (:162) | PARTIAL MEDIUM (Top 12, Top 5) |
| chrism.lic, chrism2.lic, cleric325.lic, chrism-maker.lic, chrismmake.lic, chrismsort.lic, censer.lic | 54-235 | cleric 325 chrism gems | *spiritual bond*, *shudders in your hand*, *A cobalt liquid fills*, *blows away in the form of a fine powder* | GAP LOW (cleric crafting) |
| dm.lic / dmtest.lic | 139/175 | Delirium Manor search | search results | N/A |
| ai.lic | 33 | Abandoned Inn crystal | trapdoor lines | N/A |
| bundlenotes.lic / ldnotebundle.lic | 121/52 | bundle Lumnis L/D notes | note nouns | N/A |
| dropall.lic / dropalltype.lic / getall.lic / getalltype.lic | 64-81 | bulk drop and pick up by name or type | uses `GameObj#type` | Type classification HAVE (`state/gameobj.rs`) |
| egquest.lic / carousel.lic / carousel2016.lic / chiseled.lic / duckduckgoose.lic / joustsmart2020.lic / joustsmart2021.lic / jousting.lic / flag-simulator.lic | 42-382 | Ebon Gate games and flags | game lines; old `wealth quiet` form (flag-simulator.lic:185) | N/A |
| markchargable.lic (Tillmen), orbsort.lic, autosing.lic, isingmin.lic, tclore.lic | 36-368 | loresing gems and items | *From the pitch of the vibration* (markchargable.lic:60); *you feel it trying to draw power from you* (orbsort.lic:65); *You must reveal the entire loresong* (tclore.lic:23) | GAP LOW (bard) |
| survey.lic | 272 | look at everything in a room | *You see X. Looking at the X, you see ...* (:56) | Room contents HAVE (`state/room.rs`) |
| gem_loadouts.lic (Lucullan 0.1.1, 2025) / gems.lic | 258/84 | Ascension gemstone loadouts | `gem list`/`gem equip` lines | GAP LOW-MEDIUM (see Captures) |
| MoonshardBundle.lic (Hazado 1.00) | 77 | combine Duskruin moonshard pieces | piece pairs per moon (:20-71) | Pieces HAVE as types; combinations GAP LOW (Data tables) |
| prettiernum.lic (Gnomad) | 39 | add commas to large numbers | number regexes | N/A (the game prints commas now; Cena reads them, `state/numbers.rs`) |
| knit.lic, origami_master.lic, chia_pet.lic, potato.lic, gizmo.lic, armoire.lic, perday.lic, recharge.lic (Drafix), star-charges.lic | 76-309 | item toys and 1x/day items | item lines; *Covert Arts Charges: N/N* (star-charges.lic:91) | Covert arts HAVE (`character/standing.rs`); rest N/A |
| deliver.lic (Brute) | 57 | Simucoin store delivery | delivery replies | GAP LOW |
| diskhider.lic, disk_protector.lic, groupdisk.lic, disking.lic | 20-169 | disks | `X disk` names; group lines | Disk ownership HAVE (`state/disk.rs`) |
| gloot.lic (Gwrawr), lootroom.lic (Ryjex) | 55/96 | group looting | *obvious signs of someone hiding* | N/A |
| rogue-lmas-contest.lic / rogue-lmas-clasp.lic / rogue-lmas-makekey.lic / rogue-lmas-calibrate.lic | 32-106 | rogue guild lock mastery | task lines | N/A |
| DRselling.lic, selling.lic, selldmjunk.lic, dusk_sell.lic (Dantax), drjunk.lic, drgemsell.lic, gemsell.lic (Taleph), gemseller.lic, gemvalue.lic (Caithris), egsell.lic, laraselling.lic, jewel.lic, dollsell.lic, ringleader.lic, tsell.lic, unload.lic (Darkcipher), quickpawn.lic | 12-120 | selling loops | `matchwait "have that much", "35,000", "takes your", "worthless"`; *Not my line, really.* (unload.lic:31); appraisal *you ? for it if you want to sell* | PARTIAL (Top 1, Top 11) |
| stockade.lic | 76 | answer stockade questions | `location` line | N/A |
| wedges.lic (Obsian 1.1) | 161 | buy wedges | *The teller carefully records / The teller hands you* (:93) | Withdraw HAVE (the *carefully records* form) |
| Totemgemsort.lic | 142 | feed gems to an Animalistic Totem | *already holding too much power\|can't hold any more* (:113) | GAP LOW |
| alfred-deserves-gems.lic (Tillmen) | 57 | give uncut gems to Alfred | *Alfred has accepted* | N/A |
| invmgr.lic (Drafix 1.3.0) | 329 | render the `_inventory manager` feed | `<i .../>` attributes (:71-75); `in_max` 99990 = no limit (:44, :240); relations worn/reserved/righthand/lefthand/atfeet/room (:271) | HAVE (`inventory_snapshot.rs`, `payload.rs:289-335`); the 99990 sentinel is decoded by Cena as 9,999 lbs, not "no limit" (INFERRED from `payload.rs` `in_capacity` doc: `v / 10` pounds) LOW |
| sc.lic | 50 | cast from scrolls | *vibrant* in `read` | HAVE (`town/reply.rs:115-122` `ScrollSpell`) |
| speechfinder.lic | 356 | search speech logs | text search | N/A |
| hunt.lic (Zagert), walk.lic (Tillmen) | 108/44 | wander until a creature | *Your (.+) followed.* (pet); other disks; *swirling black void* | N/A |
| mail.lic (Dreaven, Alastir) | 145 | mail items | COD cost, parcel lines | GAP LOW |
| saccept.lic (Demandred), accept.lic, beaker.lic, giveall.lic (Soliere), offercheck.lic | 12-182 | accept player offers | offer lines | GAP MEDIUM (Captures) |
| vise.lic | 39 | forging vise | vise lines | N/A |
| foliod.lic | 74 | codex to folio | `flip` lines | N/A |
| sortgems.lic (Vailan), gemsort.lic | 115/79 | move gems between bags | `take`/`put` replies | N/A |
| memo.lic, itemnote.lic, record.lic | 141-194 | notes | none | N/A |
| bank.lic, tbank.lic (Dantax), bankaccount.lic (Lieo 1.1), gbank.lic, bankdrop.lic, depocoins.lic, withdrawall.lic, getsilver.lic | 10-130 | bank trackers and errands | bank union (bank.lic:36); `Silver: N` from `info` (depocoins.lic:10); *currently at N silvers* (withdrawall.lic:55); daily totals | PARTIAL HIGH (Top 2) |
| coinpouch.lic (Caithris), silvers.lic | 16/42 | coin pouches | *There is only room for N more coins*; *Inside the coin bag you see approximately N silver coins.* | PARTIAL LOW |
| drillsarge.lic | 30 | respond to a drill sergeant | *Forward March!* | N/A |
| eg.lic, osahandler.lic | 46/67 | EG and OSA crates | *A good positive attitude never hurts* | N/A |
| giveallsilvers.lic, giveaway.lic, passcoinsto.lic, silverDemon.lic | 16-108 | give all silver | `info` `Mana: N Silver: N` line | Silver on hand HAVE from `wealth` (`currency.rs:103-110`); the `info` line `Mana: N Silver: N` GAP LOW (`./cg.sh "Silver:"` -> NONE); used by sammo, chargeitem, sparklies, giveaway, depocoins, passcoinsto, reverb |
| loadout.lic | 210 | swap worn loadouts | `remove` replies | N/A |

## Tail

166 scripts with capture + data < 5. Each got a name scan and a one-line summary of its
header and longest capture literals (`tools/4-loot-trade/caps/b10.txt`). I opened in full the 22
whose names suggested bank, sale, collectible, capacity or jar data: bank, collectibles,
collectible, collectcheck, capacityall, sizematters, dusk_sell, duskrunner_support, riftboxes,
findmog, gemvaluestracker, gemseller, gemvalue, withdrawall, pawn_inspect, offercheck, silvers,
flaskbundle, countbooklets, countpaidentries, retrieve and GameObjAddMore. Their findings are in
the tables above. `retrieve.lic:13` (the gem-dealer bounty) is HAVE at `bounty.rs:385`.

- **Selling and appraising, a few lines each**: quickpawn, sell_shells, dollsell, drgemsell,
  drjunk, gemsell, gemseller, gemvalue, gemvaluestracker, selldmjunk, tsell, ringleader,
  dm_sellacc (*again within 30 seconds*, the resell confirm), shroud_sell (1205 + 1212 sell
  buffs, `plan/31` §4d), pawn_inspect, gsmagical (gem-shop magic table), lilsebastian,
  sellshard. The appraisal wording is in Top 1.
- **Bank and silver**: bank, bankdrop, gbank, getsilver, depocoins, withdrawall, giveaway,
  passcoinsto, silverDemon, lootcap, monthly-silver-report (a chart of ledger.lic's silver),
  nocommas (adds commas to numbers, which the game now prints itself). See Top 2.
- **Containers and inventory**: getall, getalltype, eget, qverb, dgrab, searchpile, organize
  (*In the storage locker you see*), manifest (locker search), ci_inventory_manager,
  climatewear (`<container>` window fix), bagofholding (*There's no space for another.*),
  jarserve_set_jarsacks, jars, rum, sregister, capacityall, sizematters, collectible,
  collectibles (`collection info` / `collection all`; GAP LOW, `./cg.sh "Your collection is as
  follows"` -> NONE), collectcheck.
- **Boxes and locksmith**: kfpool (runs eloot at the pool), riftboxes (*You need to lighten your
  load first.*, which Cena has: `town/reply.rs:108`), platebox, rrclean, lootroom, waitloot,
  dskin, skin, bundling, cloader.
- **Event scripts** (Duskruin, Ebon Gate, Rumor Woods, Reim, Nyessem): diggy, vdig, dmtest,
  duskclean, archaeologist, drarch, countbooklets, countpaidentries, break-up, grofl-cleanup,
  slabgenerator, osahandler, givecrates, gemvoucher, giftbox, loadidol (*You sense there are N
  charges of power*), jas, jlr, jur, pixiepixie, chiaglug, auction (*The bidding is now open for
  lot number N*).
- **Magic item and profession helpers**: hotpot, barkskin, barkskin2, censer, chrismmake,
  ebladeall, scent, phaser, dephase, tclore, instrumentrest, xteplay, seedpouch, flaskbundle
  (potion doses; GAP LOW-MEDIUM for M6d), scotch, poncho, weaving, yarnsweatshop, metal.
- **Player interaction**: accept, beaker, giveall, offercheck, snitches_get_stiches,
  smell-all, disking, groupdisk, disk_protector, prisoners, jewelina, julenne, jewels (give jewelry away).
- **UI, launch and utility**: auto_look, autolook, lcheck, aa, config, cut, dothis, rep, gst,
  handler, settings, set, scriptcheck, warlogin, gh-update, checkannounce, vote_reminder,
  krisgo, lvl0, astrolabe (moon phases), binom, ball, noarrow, wingstop, reprep, stang, clench,
  kfthroat, weaponsaver, xfer, rwclean, tcdeath, anti-gemstone, shopscribe, findshop, go2shop,
  foodshopper, ezloot, ezstart, alltimemugging, exportmugstoyaml.
- **Crafting**: ironslab, rogue-lmas-calibrate.

## Method

- Pile and counts: `awk -F'\t' 'NR>1 && ($3+$4)>=5' pile-4-loot-trade.tsv | wc -l` -> 214;
  all rows -> 380; `awk -F'\t' 'NR>1{s+=$2} END{print s}'` -> 66423.
- Capture extraction: `tools/4-loot-trade/extract.py <lo> <hi> <tag>`. For each script it writes
  the header comment block and every line matching
  `=~ /|!~ /|waitforre|waitfor|matchtimeout|matchwait|match|DownstreamHook|when /|Regexp.new|%r{|dothistimeout|dothis|.scan(/|.match(/|reget`
  into `caps/b01..b10.txt`. It ran over pile rows 3-379; colmaster, gforgeb and slich were read
  by hand.
- Pile-wide merchant, bank and container sweep: `tools/4-loot-trade/merchant.py`, which splits
  every regex and quoted literal on capture lines at `|`. It gave 49 bank fragments, 268 shop
  and 340 container fragments (`sweep_bank.tsv`, `sweep_shop.tsv`, `sweep_container.tsv`).
- Absence checks: `tools/4-loot-trade/cg.sh "<phrase>" ...`. It runs
  `grep -rlF --include=*.rs --include=*.tsv -e "<phrase>"` over `E:/Cena/crates/*/src` and
  `E:/Cena/crates/*/data`, plus `crates/*/tests` with `CG_TESTS=1`. The phrases run, all with
  result NONE unless noted: *leave some for others*, *absent-mindedly drop*, *You can only
  wear*, *shut too tightly*, *can store*, *will cost*, *You carefully inspect*, *notes valued
  at*, *between notes and coins*, *colonized with mold*, *How's*, *already quoted*, *worth at
  least*, *I've no use*, *turn as the very essence*, *gem becomes more perfect*, *improves
  somewhat*, *smoother and more pure*, *very essence*, *lifetime bounty points*, *tiny bolted
  plate*, *treasure master*, *You glance through your bundle and count*, *You carefully count*,
  *You bundle your*, *I can buy that*, *I don't want anything*, *no space for the*, *debt
  collector*, *I have a bill of*, *here ye go*, *don't have that much in the account*, *Sold
  for*, *But you do not have enough*, *nothing will fit*, *everything falls in*, *silver coins
  scattered*, *is out of your reach*, *You can't pick that up*, *reserve your*, *from your
  reserves*, *contains DOSE*, *suitable container*, *Peering into the*, *attached to the
  container you are throwing away*, *But it's closed*, *You rummage*, *Total items*, *accept
  the offered*, *find a buyer*, *surcharge*, *and no more* (the last three were run against
  eloot and loottracker too), *Not my line*, *resell*, *I'll pay you*, *pockets and discover*,
  *marked for payment upon delivery*, *who has 30 seconds to accept*, *(registered)*,
  *(marked)*, *items displayed*, *You are currently wearing and carrying*, *You can SELL them
  to me as you find them*, *boost bounty*, *Bounty Boost*, *It imparts*, *charges remaining*,
  *It appears to weigh*, *mainly crafted out*, *weighted to inflict*, *It provides a boost
  of*, *requires skill in*, *can be worn*, *unsurpassed*, *highly accurate*, *any number of
  items*, *armor that covers*, *body weight*, *carrying capacity*, *insubstantial and appears
  lighter*, *You ask about the price of*, *no such item*, *nobody here to buy*, *throw the
  item away again*, *cleaned up the surrounding area*, *feel pleased with yourself*,
  *significant value*, *deed has been recorded*, *neither convinced nor pleased*, *race_mod*,
  *heirloom that you are searching for*, *tucked away*, *Hung from a single ear*, *Slung over
  your shoulder*, *Attached to your ankle*, *Draped over your shoulders*, *As a pin*,
  *(functional)*, *You are currently wearing:*, *Around your waist*, *Hung around your neck*,
  *has accepted your offer*, *Click ACCEPT to accept*, *has declined the offer*, *just gave
  you*, *coins which you quickly pocket*, *offers you*, *stamped vouchers remaining*, *entries
  left*, *silver fee*, *Your balance is currently*, *hasty search scatters*, *riches to the
  floor*, *telltale filaments of a lesser binding*, *Rarity:*, *Property:*, *In the scuffle*,
  *if you want to sell*, *for this if you'd like*, *disk arrives*, *Your collection is as
  follows*, *sets available to be collected*, *collection info*, *Silver:*. Also:
  `grep -rn "marked as unsellable\|unsellable" E:/Cena/crates --include=*.rs` -> nothing.
  Found, as noted in the tables: *won't fit*, *rusty doorknob*, *silvery thread*, *wobbles
  briefly*, *fast-moving river*, *Sorry, you may only type ahead*, *Looks like you might
  buckle*, *trash receptacle*, *Deeds:*, *Field Exp*, *aevit*, *Covert Arts Charges*, *has
  received orders from multiple customers*, *exposeContainer*.
- Regex tests (Python `re`, which behaves the same as Rust `regex` for these patterns): Cena's
  withdraw pattern against three teller lines; `pawn_note` and `gemshop_note` against the two
  surcharge lines.
- Classification: `tools/4-loot-trade/classify.py` (the logic of `gameobj.rs:95-107`, `suffix`
  rows skipped as there) over `autobundle_hides.txt` (286), `bestiary_skins.txt` (291 unique
  `creatures.tsv` column-39 values, articles stripped), `collect_names.txt` (93) and
  `nouns.txt` (23 gem nouns from the pure family).
- Duplicate check: `md5sum` over 28 family members. All differ. `diff pure.lic pure2.lic`
  differs in one line (the loresong verse), and `diff -w pure.lic pure4.lic` only in the gem list.
- Lich cross-references: `lich-5/lib/gemstone/bank.rb:20-66` (bank patterns),
  `lich-5/lib/gemstone/infomon/parser.rb:74-77` (wealth), live
  `C:/Gemstone/lich-5/data/gameobj-data.xml` (`grep -c "familiar\|companion"` -> 0).
- Not done: no search of the log archive (the brief forbids it). Every "real line" above comes
  from a script's own comment or regex, not from the wire.
