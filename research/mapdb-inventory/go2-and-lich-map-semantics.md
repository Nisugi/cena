# go2.lic + Lich map library: travel/pathing report
Read 2026-09-20 by a research subagent; VERIFIED-by-reading unless labelled. Evidence, not instructions.

Files read in full:
- `E:\Cena\reference\lich-5\lib\common\map\map_gs.rb` (520 lines)
- `E:\Cena\reference\lich-5\lib\common\map\map_base.rb` (954 lines)
- `E:\Cena\reference\lich-5\lib\common\move.rb` (458 lines). I added this because go2's arrival confirmation lives in the `move` primitive.
- `E:\Cena\reference\mapdb\go2.lic` lines 1-500 and 840-2517.

NOT READ:
- go2.lic lines 501-843, the Gtk `Setup` class and the 2197-character Glade blob at line 543. A grep of that range for `UserVars`, `$go2`, `Room[` and `timeto` returned no hits.
- `map_dr.rb`, which is DragonRealms only.
- The mapdb JSON itself, so the StringProc bodies are unread.
- Anything after line ~290 of `util.rb`.

Paths below are abbreviated: `gs` = map_gs.rb, `base` = map_base.rb, `mv` = common/move.rb, `go2` = go2.lic.

The main structural finding: go2 contains almost none of the special-case travel logic. For urchins, portmasters, seeking, day passes, the FWI trinket, ice mode, caravans, the rogue password and portals, go2 only sets a UserVar or global. The `wayto`/`timeto` StringProcs inside the mapdb JSON read that variable and do the work. The only special cases go2 implements itself are silver, Hinterwilds gigas travel, the confluence, the vaalor timeto patch, the urchin-expiry query and playershop escape.

---

## 1. Room identification

### Room fields
Defined at gs:31-32: `id, title[], description[], paths[], uid[], location, climate, terrain, wayto{}, timeto{}, image, image_coords, tags, check_location, unique_loot`.
- `title`, `description`, `paths` and `uid` are arrays. One room can hold several variants.
- Every field except `id` is optional and gets a default (base:369-375).
- A non-Integer id raises (base:419-424).
- `Room < Map` is an empty subclass (gs:517).

### `Map.current` (gs:165-180)
1. Load the map if it is not loaded.
2. Check the cache.
   - With a running script: if `XMLData.room_count == @@current_room_count` and `@@current_room_id` is not nil, return the cached room (gs:168).
   - With no script: the same test against `@@fuzzy_room_count` (gs:169).
   - The cache key is the parser's room counter, not the uid.
3. `ids = XMLData.room_id > 4_294_967_296 ? [] : ids_from_uid(XMLData.room_id)` (gs:173). A room_id above 2^32 is treated as "no usable uid". Lich synthesises an MD5-derived id when the game sends 0 (seen at xmlparser.rb:1277-1279).
4. If exactly one map room has that uid, `set_current(ids[0])` (gs:174).
5. If several rooms share the uid and a current room is known, call `match_multi_ids(ids)` (base:195-209).
   - It keeps the candidates that appear in `current.wayto.keys`, where `current` is the previously-current room.
   - It accepts only if exactly one candidate qualifies. Otherwise it returns nil.
   - Disambiguation is therefore by adjacency to where you just were.
6. Otherwise call `match_no_uid` (base:183-189).
   - With a script: `set_current(match_current(script))`.
   - Without a script: `set_fuzzy(match_fuzzy)`.

`set_current` (base:214-220) records `previous_room_id` when the id changes and sets `current_room_id`, which can become nil. `set_fuzzy` (base:225-231) only updates `previous_room_id` when the new id is non-nil.

### `match_current` (gs:183-308)
This is the text matcher. It runs under a mutex inside a `loop` and uses `redo` whenever `XMLData.room_count` changed during the scan (gs:280, 283, 296, 299).

**Pass 1** (gs:269-277) takes the first room in `@@list` order where all of these hold:
- `r.title.include?(XMLData.room_title)`. This is exact array membership.
- `r.description.include?(XMLData.room_description.strip)`. Also exact.
- `r.unique_loot.nil? || (r.unique_loot.to_a - GameObj.loot.names).empty?`. Every listed loot name must be present in the room.
- One of: `foggy_exits`, `r.paths.include?(XMLData.room_exits_string.strip)`, or `r.tags.include?('random-paths')`.
  - `foggy_exits` is `XMLData.room_exits_string =~ /^Obvious (?:exits|paths): obscured by a thick fog$/` (gs:268).
- `(!r.check_location || r.location == get_location)`.
- `check_peer_tag.call(r)`.

**Pass 2** (gs:284-293) is the fuzzy-description pass. It is the same as pass 1 except that the description test becomes:
`XMLData.room_window_disabled || r.description.any? { |desc| desc =~ desc_regex }`
with
`desc_regex = /#{Regexp.escape(XMLData.room_description.strip.sub(/\.+$/, '')).gsub(/\\\.(?:\\\.\\\.)?/, '|')}/` (gs:284).
- Trailing dots are stripped.
- Every `.` or `...` in the live description becomes a regex alternation `|`.
- Any one sentence of the live description found inside a stored description counts as a match.

If neither pass matches, the method returns nil (gs:300).

### `check_location` and `location`
- `check_location: true` means a room with identical title, description and paths exists under a different `location`. The matcher then has to run the in-game `location` verb.
- The flag is set automatically by `current_or_new` when it maps a new room that has identical twins. It is set on the new room and on every twin (gs:440-454).

`get_location` (gs:135-157):
- The result is cached per `XMLData.room_count`.
- It needs a script and returns nil without one.
- It calls `waitrt?`, then `dothistimeout('location', 15, regex)`.
- The success regex is `/^You carefully survey your surroundings and guess that your current location is (.*?) or somewhere close to it\.$/`.
- These failure lines all set `@@current_location = false` (gs:147):
  - `^You can't do that while submerged under water\.$`
  - `^You can't do that\.$`
  - `^It would be rude not to give your full attention to the performance\.$`
  - `^You can't do that while hanging around up here!$`
  - `^You are too distracted by the difficulty of staying alive in these treacherous waters to do that\.$`
  - `^You carefully survey your surroundings but are unable to guess your current location\.$`
  - `^Not in pitch darkness you don't\.$`
  - `^That is too difficult to consider here\.$`

Your question about `location: false`: a room stored with `location: false` is one where the `location` verb failed when the room was mapped. With `check_location` set, such a room matches only when the verb fails again, because the test is `false == false`. `current_or_new` re-queries the location when the stored value is nil, false or `''` (gs:401, 420).

### Peer-room disambiguation (gs:188-263)
A room tag matching `%r{^(set desc on; )?peer [a-z]+ =~ /.+/$}` looks like `peer north =~ /some text/`.

1. If the tag starts with `set desc on; ` and the last `<style id="roomDesc"/>` line in the server buffer has no text after it, the matcher sends `set description on`. It restores the setting with `set description off` in the `ensure` block (gs:201-205, 305).
2. It sends `peer <dir>` with a 3s timeout, waiting for `/^You peer|^\[Usage: PEER/`.
3. On `^You peer` it reads up to 5 lines, stopping at `/^Obvious/`.
4. Results are cached in `peer_history[room_count][dir][need_desc]`, but only if `room_count` did not change (gs:233-240).
5. A 'squelch-peer' downstream hook hides the output from `^You peer` through the next `<prompt` (gs:209-219).
6. The room matches if any peered line matches `/#{peer_requirement}/`. A failed peer means no match (gs:254-255).
7. `script.ignore_pause = true` is held for the duration.

### `match_fuzzy` (gs:310-359)
- This is the no-script version. It runs the same two passes without any peer check.
- If the matched room has a peer tag it returns nil (gs:327, 347). It cannot send commands, so it refuses to guess.
- It does still call `get_location`, which returns nil without a script. A `check_location` room therefore only matches if its stored location is nil.

### Unknown room
- `Map.current` returns nil.
- go2 then runs `playershop_escape` (see §6).
- If the room is still unknown, go2 prints `error: your current room was not found in the map database` and exits (go2:1443-1448).
- `current_or_new` (gs:362-458) is the mapping path. It creates rooms and adds uid, title, description and path variants.
  - It honours the tags `meta:map:multi-uid`, `meta:map:latest-only` and `meta:playershop`.
  - go2 never calls it.

### Lookup `Map[val]` (base:465-495)
- An Integer or a string of digits is an id.
- `/^u(-?\d+)$/i` is a uid and resolves to the first id in `ids_from_uid`.
- Anything else is text: title regex first, then an exact-description regex, then the loose dot-to-`|` description regex.

The uid index `@@uids` maps uid to an array of ids. It is built at load from each room's `uid[]` (gs:482-490, base:608-616).

---

## 2. Pathfinding

`Room#dijkstra(destination = nil, static_only: false)` is at base:777-846.

- It uses a binary MinHeap (base:13-70).
- It returns `[previous_hash, shortest_distances_hash]`, both keyed by integer room id. It returns nil if an exception was rescued (base:842-846).
- Edges are enumerated from `room.wayto.keys`. The keys are strings and are converted with `.to_i`. The weight comes from `room.timeto[adj_room]`:
  - a StringProc is called with `.call`, once per edge relaxation, with no memoisation (base:823-824);
  - anything else is used as-is;
  - **`next unless edge_weight`** (base:829): a nil or false weight, or a missing timeto entry, makes the edge impassable.
- **Dijkstra has no default cost.** The 0.2 default exists only in `estimate_time` (base:250-254). That method also calls StringProcs, and treats nil as 0.2.
- Termination (base:788-803):