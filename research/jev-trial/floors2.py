"""Floors, second method: outdoor ground is floor 0 everywhere.

floors.py joins rooms over every same-floor move into levels, so ONE door that
really changes level merges two levels across the whole map (its biggest level
is 9,637 rooms and holds 100 conflicts). Here an error cannot leave a structure:

  1. OUTDOOR rooms are joined over same-floor moves and outdoor up/down (slopes)
     into outdoor levels. An outdoor level holding a known ground room, or of at
     least BIG rooms, is floor 0. A smaller one joined to a placed outdoor level
     by a climb Jev/the subagent read (climb tree, climb ratlines) is +1 / -1
     from it -- the Hanging Gardens, a crow's nest. Any other is floor 0.
  2. INDOOR rooms are joined over same-floor moves BETWEEN INDOOR ROOMS only.
     An indoor level with a same-floor move to outdoors takes that outdoor
     room's floor (majority if they differ). Then up/down is walked from the
     anchored indoor levels, the map's literal up/down first.
     A DEAD-END POCKET -- rooms whose only way in is one outdoor climb -- is
     never forced to 0; it takes its floor from the climb (author, 2026-09-21).
  3. An indoor stack never reached is anchored by default, as in floors.py.

Exit classification is floors.py's, except that a literal up/down way back is
checked BEFORE the plain-portal default (see exits()).

Usage:
    python floors2.py [cut] [big]      # defaults 0.93, 25
"""

import collections
import json
import sys

from build_keys2 import FLOOR_CHANGING
import re

from floors import COMPASS, MAP, R, STEP, VERT, outdoors

BUILT = re.compile(r"deck|mast|nest|roof|tower|wall|balcon|terrace|platform|rampart|"
                   r"battlement|castle|ship|galley|brig|sloop|carrack|galleon|frigate|"
                   r"parapet|scaffold|loft|stage", re.I)


def exits(rooms, cut):
    rows = [json.loads(l) for l in open(R + "job1_classified.jsonl", encoding="utf-8")]
    jev = {(r["from"], r["to"]): STEP[r["direction"]] for r in rows
           if r["estimated_accuracy"] >= cut}
    for l in open(R + "job1_kept_agent.jsonl", encoding="utf-8"):
        r = json.loads(l)
        jev[(r["from"], r["to"])] = STEP[r["direction"]]
    flat, vertical = [], []
    for a, r in rooms.items():
        for b, cmd in r["wayto"].items():
            b, c = str(b), str(cmd).strip().lower()
            if b not in rooms or c.startswith(";e"):
                continue
            if c in VERT:
                vertical.append((a, b, VERT[c], "rule"))
            elif (a, b) in jev or (b, a) in jev:
                s = jev[(a, b)] if (a, b) in jev else -jev[(b, a)]
                if s == 0:
                    flat.append((a, b))
                else:
                    vertical.append((a, b, s, "llm"))
            elif c in COMPASS:
                flat.append((a, b))
            else:
                # The way back decides first: `go gate` whose way back is `down`
                # goes up. floors.py called it flat one way and down the other.
                back = str(rooms[b]["wayto"].get(a, "")).strip().lower()
                if back in VERT:
                    vertical.append((a, b, -VERT[back], "rule"))
                elif c in ("out", "go out") or not FLOOR_CHANGING.search(c):
                    flat.append((a, b))
    return flat, vertical


def main():
    args = sys.argv[1:]
    cut = float(args[0]) if args else 0.93
    big = int(args[1]) if len(args) > 1 else 25
    rooms = {str(r["id"]): r for r in json.load(open(MAP, encoding="utf-8"))}
    out = {k: outdoors(r) for k, r in rooms.items()}
    flat, vertical = exits(rooms, cut)

    parent = {k: k for k in rooms}

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    doorways = []                                 # same-floor, indoor <-> outdoor
    for a, b in flat:
        if out[a] == out[b]:
            parent[find(a)] = find(b)
        else:
            doorways.append((a, b))
    # Dead-end pockets: an outdoor climb that is the ONLY way into a set of rooms
    # (a crow's nest, a tree house, a rooftop, a beach under a cliff). Nothing
    # else in the map can disagree, so the pocket takes its floor from the climb
    # and is never forced to 0 -- not even if the old flood fill reached it.
    adj = collections.defaultdict(set)
    for a, r in rooms.items():
        for b in r["wayto"]:
            if str(b) in rooms:
                adj[a].add(str(b))
                adj[str(b)].add(a)

    def pocket(s, t):
        seen, q = {s}, collections.deque([s])
        while q:
            x = q.popleft()
            for y in adj[x]:
                if {x, y} == {s, t}:
                    continue
                if y == t:
                    return None
                if y not in seen:
                    seen.add(y)
                    q.append(y)
        return seen

    # The map's literal up/down between two outdoor rooms is a slope on a
    # hillside -- UNLESS it is the only way into a pocket of at most SMALL rooms:
    # `down` out of a crow's nest or off a rooftop is a real change of level.
    # ... or it is on something BUILT. A ship is bigger than 8 rooms and its
    # crow's nest can be reached from two decks, so size and dead ends both miss
    # it (author, 2026-09-21). 39 of the 448 outdoor literal `up` exits match.
    SMALL = 8
    POCKET = 40       # a bigger one-entrance region is a region, not a pocket
    dead_end = {}
    for a, b, s, src in vertical:
        if out[a] and out[b] and (b, a) not in dead_end:
            sides = [x for x in (pocket(b, a), pocket(a, b)) if x is not None]
            dead_end[(a, b)] = dead_end[(b, a)] = min(sides, key=len) if sides else None
    climbs = []
    for a, b, s, src in vertical:
        built = BUILT.search(rooms[a]["title"][0] + " " + rooms[b]["title"][0])
        if (out[a] and out[b] and src == "rule" and not built
                and not (dead_end[(a, b)] and len(dead_end[(a, b)]) <= SMALL)):
            parent[find(a)] = find(b)
        else:
            climbs.append((a, b, s, src))
    level = {k: find(k) for k in rooms}
    members = collections.defaultdict(list)
    for k, l in level.items():
        members[l].append(k)
    is_out = {l: out[l] for l in members}

    ledges = collections.defaultdict(list)
    for a, b, s, src in climbs:
        if level[a] != level[b]:
            ledges[level[a]].append((level[b], s, src))
            ledges[level[b]].append((level[a], -s, src))

    lfloor, lanchor = {}, {}

    def walk(starts, anchor, only=None):
        for allowed in (("rule",), ("rule", "llm")):
            q = collections.deque(starts)
            while q:
                x = q.popleft()
                for y, s, src in ledges[x]:
                    if src in allowed and y not in lfloor and (only is None or only(y)):
                        lfloor[y], lanchor[y] = lfloor[x] + s, anchor
                        q.append(y)
            starts = list(lfloor)

    in_pocket = set()
    for a, b, s, src in climbs:
        if out[a] and out[b] and dead_end[(a, b)] and len(dead_end[(a, b)]) <= POCKET:
            in_pocket |= {level[k] for k in dead_end[(a, b)]}
    print(f"levels inside a dead-end pocket: {len(in_pocket)}")

    door_levels = {level[a if out[a] else b] for a, b in doorways}
    climb_levels = {l for l in members if any(not is_out[y] for y, *_ in ledges[l])}

    # 1. outdoors
    ground = {level[g] for g in json.load(open(R + "ground.json")) if out[g]}
    # A ship's main deck is floor 0 (author, 2026-09-21), pocket or not. Left to
    # the cluster rule below, a bigger quarterdeck level took 0 and pushed the
    # main deck to -1 on 6 ships.
    main_deck = {level[k] for k, r in rooms.items()
                 if "main deck" in (r.get("title") or [""])[0].lower()}
    for l in main_deck:
        lfloor[l], lanchor[l] = 0, "main deck"
    for l in members:
        if l in in_pocket or l in lfloor:
            continue
        if is_out[l] and (l in ground or len(members[l]) >= big):
            lfloor[l], lanchor[l] = 0, "outdoor ground"
    walk(list(lfloor), "outdoor climb", only=lambda y: is_out[y])
    # Small outdoor clusters nothing placed (a ship: deck, shrouds, crow's nest).
    # The biggest level outside any pocket is the cluster's ground; climb from it.
    for l in sorted(members, key=lambda x: (x in in_pocket, -len(members[x]))):
        if is_out[l] and l not in lfloor:
            # A rooftop or balcony whose way out is the building under it waits
            # for the building: step 2 walks up to it, step 3 catches the rest.
            # So does a small "outdoor" room entered only through a building: a
            # courtyard, or a shop in a ship's hold that the map marks outdoors.
            # Five of those pulled the Spitfire's 129-room cargo hold up to 0.
            # ENCLOSED = every neighbour of every room in it, by any exit, is
            # indoors. A porch or a street stub has an outdoor neighbour and is
            # ground; deferring those too put a town centre at +2.
            if len(members[l]) < SMALL and l in door_levels | climb_levels and all(
                    not out[y] or level[y] == l for k in members[l] for y in adj[k]):
                continue
            lfloor[l], lanchor[l] = 0, "outdoor ground"
            walk([l], "outdoor climb", only=lambda y: is_out[y])

    # 2. indoors, from their doorways
    touch = collections.defaultdict(list)
    for a, b in doorways:
        i, o = (b, a) if out[a] else (a, b)
        if level[o] in lfloor:
            touch[level[i]].append(lfloor[level[o]])
    for l, fs in touch.items():
        lfloor[l], lanchor[l] = collections.Counter(fs).most_common(1)[0][0], "doorway"
    walk(list(lfloor), "stack")
    for a, b in doorways:                 # a waiting balcony level with its room
        i, o = (b, a) if out[a] else (a, b)
        if level[o] not in lfloor and level[i] in lfloor:
            lfloor[level[o]], lanchor[level[o]] = lfloor[level[i]], "doorway"
    walk(list(lfloor), "stack")

    # 3. what is left
    def stack_of(start):
        comp, q = {start}, collections.deque([start])
        while q:
            x = q.popleft()
            for y, *_ in ledges[x]:
                if y not in comp and y not in lfloor:
                    comp.add(y)
                    q.append(y)
        return comp

    for l in sorted(members, key=lambda x: -len(members[x])):
        if l not in lfloor:
            base = max(stack_of(l), key=lambda x: (len(members[x]), int(x)))   # int: no hash-order ties
            lfloor[base], lanchor[base] = 0, "default"
            walk([base], "default")

    # ---- conflicts and spans -------------------------------------------------
    floor = {k: lfloor[level[k]] for k in rooms}
    conflicts, spans, done = [], 0, set()
    for a, b, s, src in climbs:
        if (b, a) in done:
            continue
        done.add((a, b))
        diff = floor[b] - floor[a]
        if diff == s:
            continue
        if diff * s > 0:
            spans += 1
            continue
        where = ("both outdoors" if out[a] and out[b] else
                 "both indoors" if not out[a] and not out[b] else "indoor/outdoor")
        kind = "same level" if level[a] == level[b] else "same floor" if diff == 0 else "opposite"
        conflicts.append({"kind": kind, "where": where, "from": a, "to": b, "step": s,
                          "edge": src, "floor_from": floor[a], "floor_to": floor[b],
                          "level_size": len(members[level[a]])})
    for a, b in doorways:
        if floor[a] != floor[b] and (b, a) not in done:
            done.add((a, b))
            conflicts.append({"kind": "doorway", "where": "indoor/outdoor", "from": a, "to": b,
                              "step": 0, "edge": "flat", "floor_from": floor[a],
                              "floor_to": floor[b], "level_size": len(members[level[a]])})

    json.dump({k: {"floor": floor[k], "anchor": lanchor[level[k]]}
               for k in sorted(rooms, key=int)}, open(R + "floors2.json", "w"), indent=0)
    with open(R + "floor2_conflicts.jsonl", "w", encoding="utf-8") as fh:
        for c in conflicts:
            fh.write(json.dumps(c) + "\n")

    n = len(rooms)
    sizes = sorted((len(v) for v in members.values()), reverse=True)
    print(f"levels {len(members)} | largest {sizes[:5]}")
    print(f"largest INDOOR levels {sorted((len(v) for l, v in members.items() if not is_out[l]), reverse=True)[:5]}")
    for anc, v in collections.Counter(lanchor[level[k]] for k in rooms).most_common():
        print(f"   anchored by {anc:15}: {v:6} ({v / n:.1%})")
    dist = collections.Counter(floor.values())
    print("floors:", " ".join(f"{k:+d}:{dist[k]}" for k in sorted(dist)))
    print(f"exits that cross more than one floor: {spans}")
    print(f"conflicts: {len(conflicts)}")
    for k, v in sorted(collections.Counter((c["kind"], c["where"], c["edge"]) for c in conflicts).items()):
        print(f"   {k[0]:10} {k[1]:15} {k[2]:5} {v}")


if __name__ == "__main__":
    main()
