# go2.lic + Lich map library — part 2 (sections 2-8)

Digest written 2026-09-20 by the main session from a research subagent's report; part 1
(`go2-and-lich-map-semantics.md`) holds room identification in full and stops at the start
of section 2. Evidence, not instructions.

Citations: `base` = `reference/lich-5/lib/common/map/map_base.rb`, `gs` = `map_gs.rb`,
`mv` = `lib/common/move.rb`, `go2` = `reference/mapdb/go2.lic`. NOT READ: go2 lines 501-843
(GTK setup), `map_dr.rb`, the mapdb procs themselves.

**The structural finding: go2 contains almost none of the special-case travel logic.** For
urchins, portmasters, seeking, day passes, the FWI trinket, ice mode, caravans, the rogue
password and portals, go2 only sets a variable; the mapdb procs do the work. go2 itself
implements: silver, Hinterwilds gigas travel, the confluence search, the Vaalor shortcut
patch, the urchin-expiry query, and playershop escape.

## 2. Pathfinding — `Room#dijkstra` (base:777-846)

- Binary min-heap. Weight = `timeto[adj]`; a StringProc is `.call`ed once per relaxation,
  no memoisation (base:823).
- **`next unless edge_weight`** (base:829): nil, false, or a missing timeto = impassable.
  **Dijkstra has no default cost**; the 0.2 default exists only in `estimate_time`
  (base:250-254).
- Destination forms (base:788-803): Integer stops when popped. **Array stops at the first
  listed room popped with distance < 20**; otherwise the whole graph is explored. nil =
  full graph.
- `static_only: true` uses only plain commands with numeric costs (base:817); go2 never
  uses it.
- `path_to` excludes the source, includes the destination. No result caching.
- YAML overrides exist: `apply_wayto_overrides` (base:657-688), entries
  `{start_room, end_room, str_proc, travel_time}`. Caller NOT READ.

## 3. go2's trip loop (go2:2143-2517)

Outer loop = one plan + one execution. `$room_count` (a parser counter bumped per room
arrival) is the arrival signal throughout.

- On `$go2_restart`: poll 10 x 0.05 s for "already there"; if `error_count > 1` run a
  **lag probe** — send `help lag-check`, wait 5 s for
  `/^No help files matching that entry were found\./` — a round-trip barrier; then
  re-identify the room and re-run Dijkstra.
- No path => `error: failed to find a path between...`.
- Unless `--preserve-scripts`: stop `roomnumbers` if the route has a peer-tag room; stop
  `textsubs` if ETA > 5 s.
- Confirmation pause unless disabled, restarting, or `confirm` is false.

Per step: exit if dead; wait out `muckled?` (webbed/dead/stunned/bound/sleeping); stand
unless standing, the wayto mentions swim/pedal, or mounted.

- Stand regex union (go2:2328-2341): "You cannot do that while mounted." · "You stand
  (back )?up." · "your \w+ back and stand up." · "You struggle, but fail to stand." · "You
  are already standing." · "You don't seem to be able to move to do that." · "There's not
  enough room to do that!" · "There is not enough room to stand up in here." · "You'd tip
  the boat over!" · "...wait N seconds." · "You are overburdened and cannot manage to
  stand." · "You attempt to stand, but slip and fall flat in the slippery green gook!"
- On "mounted": `Go2.mounted = true`; if urchins are on, turn them off, restart.
- **Proc wayto:** wait for outstanding typeahead to land, `.call` it, `sleep delay`,
  `break if $go2_restart`. **A proc step gets NO arrival verification from go2** (the check
  at go2:2403/2407 is commented out). A proc may set `$go2_restart` to force a replan.
- **String wayto, typeahead mode:** fire-and-forget `put`, bounded by
  `$room_count + typeahead`; on "Sorry, you may only type ahead" reduce typeahead, restart.
- **String wayto, confirmed mode** (always the first move): `result = move cmd`. Falsy =>
  `error_count += 1`; **blacklist** the edge (`timeto = nil` for this run, reverted at
  exit) only if `idx == 0 && error_count > 2 && still in that room`; then restart.
- After each step: `waitrt?`; re-cast 402 under 515 if `$go2_cast`.
- **No explicit wrong-room check.** Recovery is only: `move` returning falsy, a typeahead
  count mismatch, or a proc setting `$go2_restart`. Restarts are unbounded.
- Final: poll 1 s for the destination; failure is silent.

### The `move` primitive (mv:167-447) — port verbatim

`move(dir, giveup_seconds = 10, giveup_lines = 30)` -> true / false (exit is wrong) / nil
(blocked). Arrival = `XMLData.room_count` increased. `MAX_REMEDIES 3`, `MAX_ROLLS 20`.

- **false**: the long regex at mv:284 (`^You can't go there|^You can't (?:go|swim) in that
  direction\.|^Where are you trying to go\?|^What were you referring to\?|^I could not find
  what you were referring to\.|^How do you plan to do that here\?|^You take a few steps
  towards|^You cannot do that\.|…`) — copy the whole line from the file.
- **nil**: mv:290 (`^An unseen force prevents you\.$`, `^Sorry, you aren't allowed to enter
  here\.`, ticket/guard/locker/"Abandoned" lines).
- Engaged (mv:267): `retreat` twice. Hidden (mv:273): `unhide`.
- "rusty doorknob" with a door dir (mv:275-283): iterate ordinals first..twelfth.
- Climb/swim failures: sleep 1, `waitrt?`, stand if needed, resend, up to 20.
- `^You(?:'re going to| will) have to climb that\.` => go->climb;
  `^You can't climb that\.` => climb->go (mv:331-334).
- Hands full (mv:350): `empty_hands`, `fill_hands` on exit.
- Closed (mv:355): `(?:appears|seems) to be closed\.$|^You cannot quite manage to squeeze
  between the stone doors\.$` => `open` once; second time false/:closed.
- RT (mv:363): `/^(\.\.\.w|W)ait ([0-9]+) sec(onds)?\.$/` => sleep n-0.2 (0.3 if n<=1).
- `^You're still recovering from your recent` => sleep 2. Type-ahead (mv:392) => sleep 1.
- `^(You notice .* at your feet, and do not wish to leave it behind|As you prepare to move
  away, you remember)` => `stow feet` (mv:408).
- "You don't seem to be able to move to do that." => wait <=3 s for "You regain control of
  your senses!".
- **`^It's pitch dark and you can't see a thing!` => SUCCESS** (mv:431). Sailor's Grief
  swim lines => success (mv:308).
- Webbed has no branch; go2's `muckled?` wait covers it.

## 4. Target resolution (go2:1443-2106), in order

0. `goback` -> `UserVars.go2_start_room`.
1. uid: `/u(?<uid>\d+)/` (unanchored) -> first id from `ids_from_uid`.
2. Room id `^[0-9]+$`; `confirm = false`.
3. `confluence` / `confluence-hot` / `confluence-cold` / `instability`.
4. Custom targets (`GameSettings['custom targets']`, name -> id or list): exact then prefix
   match; a list resolves to the nearest. `save` / `delete` / `list`.
5. `guild` / `guild shop` -> tag `"<profession> guild"`.
6. `locker` -> `public locker` or `meta:che:<che>:locker` + entrance tags, nearest.
7. **Any tag, exact and case-sensitive -> nearest.** This is how bank, gemshop, town,
   portmaster all work. Town-relative targets are not a separate concept.
8. Text search over title/description; several hits => a paged menu.

**"Nearest" is measured in timeto SECONDS (proc costs included), not rooms.** Confirm when
distance >= `confirm_distance` (default and minimum 20). `;go2 targets` tag list is at
go2:1013.

## 5. Variables (go2 side). The mapdb side is measured in `vars.tsv`.

| Name | Meaning | Default |
|---|---|---|
| `UserVars.mapdb_use_urchins` | procs may use urchin guides; forced false when mounted | false |
| `UserVars.mapdb_urchins_expire` | epoch seconds; 0 = none | 0 |
| `UserVars.mapdb_use_portmasters` | | false |
| `$go2_use_seeking` | Voln symbol of seeking | false |
| `$go2_get_silvers` | may withdraw from the bank | false |
| `CharSettings['get return trip silvers']` | budget the return too | nil |
| `UserVars.mapdb_ice_mode` | `auto` / `wait` / `run` | `auto` |
| `UserVars.mapdb_fwi_trinket` | trinket noun; nil = off (go2:1398 has a bug: reads `mapdb_trinket`) | nil |
| `UserVars.mapdb_use_day_pass`, `mapdb_buy_day_pass`, `day_pass_sack` | pairs validated against `wl,imt|imt,wl|wl,sol|sol,wl|imt,sol|ill,val|val,ill|ill,cys|cys,ill|val,cys|cys,val` (`sol,imt` is missing upstream) | false / nil |
| `UserVars.mapdb_car_to_sos` / `_from_sos` | Sanctum of Scales caravan | false |
| `$go2_use_vaalor_shortcut` | patches `16745<->16746` timeto to 15 or nil | false |
| `UserVars.mapdb_use_portals`, `_use_old_portals`, `_have_portal_pass` | Plat/GSF only | unset |
| `UserVars.mapdb_hinterwilds_location` | `"EN"` / `"IM"` | nil |
| `CharSettings['use_gigas_hwtravel']`, `['gigas_min_number']` (4..20) | | unset |
| `$mapdb_last_instability`, `$mapdb_instability_timeto` | room id; town id -> seconds for `[228, 2300, 1438, 1005, 188, 1932, 3519, 10855, 3668]` | unset |
| `UserVars.rogue_password` | read by rogue guild procs | '' |
| `$go2_restart` | go2 AND procs; forces a replan | false |
| `CharSettings`: `typeahead` 0, `delay` 0, `stop for dead` false, `disable_confirm` false, `confirm_distance` 20 | | |

`mapdb_fwi_return_room`, `mapdb_premium`, and every per-event `*_origin` variable do NOT
appear in go2.lic; they are mapdb-internal.

## 6. Special cases go2 implements

**Urchins** (go2:975-996). The query runs in a `before_dying` hook — AFTER the trip — and
only when urchins are on and the expiry is nil or past. Send `urchin status`, 3 s, await
`/You will have access to the urchin guides|You currently have no access to the urchin
guides.|permanent access to the urchin guides./`. Expiry:
`/You will have access to the urchin guides until (?<expires>.*?)\./`, date rewritten by
`gsub(/(\d+)\/(\d+)\/(\d+) (\d+:\d+:\d+) (.*?)$/, '\3-\1-\2 \4 \5')` (M/D/Y -> Y-M-D).
Permanent => Jan 1 of next year. Otherwise 0.

**Silver** (go2:958-973, 2217-2300). Route cost from room tags
`/^silver-cost:<next_room_id>:(.*)$/`; digits are used directly, **anything else is Ruby**
evaluated via StringProc. On hand: `info` squelched, parsed by
`/^\s*Mana\:\s+\-?[0-9]+\s+Silver\:\s+([0-9,]+)/`; fallback `wealth quiet`,
`/^You have (no|[,\d]+|but one) silver with you/`. Short + permitted: nearest bank whose own
path is affordable (else "You're too poor to go to the bank."), child `go2 <bank>`, `unhide`,
`withdraw <deficit> silvers` (Pinefar Depository: `ask banker for <max(deficit,20)>
silvers`), re-check. Short + not permitted: warn, `sleep 10`, continue. **No deposit logic.**

**Hinterwilds** (go2:337-371, 2192-2200). Trigger: path includes room 29860 or 22154,
gigas travel on, and `wealth gigas` ->
`/You are carrying (\d+) gigas artifact fragments\./` meets the minimum. Inbound: go to
`u13205202` (if a path title includes 'Seethe Naedal'; var "EN") else `u4132054` ("IM"),
`go sliver` twice. Outbound: go to `u7503253`, `order 3`, `order confirm`. Then replan.

**Confluence / instability** (go2:1617-1854). `attune` reads the element
(`/You are attuned to the Element of (.+)\./`); revisit sightings <10 s away (6 h TTL);
`attune sense` up to 2x accepting `/^You sense nothing unusual\.|^You sense an unusual
fluctuation emanating from .+\.|^You feel your senses being pulled towards a strong
fluctuation\.\.\.|^You feel a strong sense of instability surround you!/`; walk candidates
looking for loot noun `instability`; `look instability` emission -> element (gust of wind =
air, burst of sparks = lightning, waft of heat = fire, puff of rock dust = earth, puff of
mist = water); enter with `push instability with my soulstone`.

**Playershop escape** (go2:1434-1443). Room unknown and exits present, and title matches
`Outfitting|Magic Shoppe|Weaponry|General Store|Armory|Combat Gear|Lockpicks` or
`room_id / 1000` in 631..646: move `out` (or the first exit).

**Stop-for-dead**: pause when a dead PC is present.

**Absent from go2 entirely:** hiding/sneaking while travelling, group handling (only DR
`--drag`), hand stowing (that is in `move`).

## 7. Pre-flight and post-arrival

Pre: seed defaults -> parse options (one-trip overrides restored at exit) -> Vaalor patch ->
playershop escape + room check -> resolve target -> dead-watcher -> `flag description off` /
`flag roomnames off` (restored with `look`) -> per plan: HW detour, script hygiene, silver,
confirmation. Post: wait 1 s for the destination, echo travel time, restore everything,
revert blacklisted timetos, run the urchin expiry query. **No arrival-failure error.**

## 8. API surface the mapdb procs may use

From go2: `$go2_restart` (writable), `$go2_use_seeking`, `$go2_get_silvers`, `$go2_cast`,
`Go2.mounted`, `Go2.go2_check_silver`, `Go2.wealth_quiet`, `Go2.silent_command`,
`Go2.min_fragment_count`, `Go2.hinterwilds_travel`. From the map library: `Map[]`/`Room[]`
(id, `uNNN`, text), `Map.current`, `Map.previous`, `Map.list`, `Map.ids_from_uid`,
`Map.dijkstra`, `Map.findpath`, `Map.get_location`, `Room#find_nearest`,
`#find_nearest_by_tag`, `#find_all_nearest_by_tag`, `#path_to`, `#wayto`, `#timeto`, `#tags`.
