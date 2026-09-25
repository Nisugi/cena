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
import os
import sys

from build_keys2 import FLOOR_CHANGING
import re

R = "results/"
MAP = "E:/Cena/reference/mapdb/map-1789942730.json"
COMPASS = {"north", "south", "east", "west", "northeast", "northwest",
           "southeast", "southwest", "n", "s", "e", "w", "ne", "nw", "se", "sw"}
VERT = {"up": 1, "u": 1, "down": -1, "d": -1}
STEP = {"up": 1, "down": -1, "same floor": 0}


def outdoors(room):
    return {p.lower()[:13] for p in room.get("paths") or []} == {"obvious paths"}

BUILT = re.compile(r"deck|mast|nest|roof|tower|wall|balcon|terrace|platform|rampart|"
                   r"battlement|castle|ship|galley|brig|sloop|carrack|galleon|frigate|"
                   r"parapet|scaffold|loft|stage", re.I)
PORCH = re.compile(r"porch|veranda|stoop", re.I)
LAYOUTS = "E:/Cena/reference/mapdb/map-data/prime/layouts.json"


def clusters(rooms):
    """Simutronics' own prebaked render layouts: which rooms they draw on one
    plate. A cluster is a DRAWING artefact, not a floor -- but a plate cannot
    hold two rooms in one spot, so a vertical move usually forces a new cluster.
    MEASURED on the 553-exit key: 0.4% of compass moves cross a cluster boundary
    against 67.9% of literal up/down. And between two OUTDOOR rooms it is near
    perfect: same cluster -> same floor 99/99, different cluster -> a level
    change 64/67. INDOORS it is worthless (same cluster is same floor only
    18/49): a dense town gets packed onto one plate, storeys and all."""
    uid = {}
    for k, r in rooms.items():
        for u in (r.get("uid") or []):
            uid[int(u)] = k
    out = {}
    for key, lay in json.load(open(LAYOUTS, encoding="utf-8"))["layouts"].items():
        name = key.split("||")[0]
        for c in lay["clusters"]:
            for u in c["rooms"]:
                if u in uid:
                    out[uid[u]] = (name, c["id"])
    return out


def exits(rooms, cut):
    rows = [json.loads(l) for l in open(R + "job1_classified.jsonl", encoding="utf-8")]
    jev = {(r["from"], r["to"]): STEP[r["direction"]] for r in rows
           if r["estimated_accuracy"] >= cut}
    for l in open(R + "job1_kept_agent.jsonl", encoding="utf-8"):
        r = json.loads(l)
        jev[(r["from"], r["to"])] = STEP[r["direction"]]
    # Round two (agent_round2.py): uncrossed exits a second subagent settled at the
    # author's 93% gate, this time shown each room's computed floor and name verdict.
    if os.path.exists(R + "job1_kept_round2.jsonl") and "--no-round2" not in sys.argv:
        for l in open(R + "job1_kept_round2.jsonl", encoding="utf-8"):
            r = json.loads(l)
            jev[(r["from"], r["to"])] = STEP[r["direction"]]
    # The author's own answers outrank everything: he plays the game and has the
    # in-game map (apply_author.py).
    if os.path.exists(R + "job1_kept_author.jsonl") and "--no-author" not in sys.argv:
        for l in open(R + "job1_kept_author.jsonl", encoding="utf-8"):
            r = json.loads(l)
            jev[(r["from"], r["to"])] = STEP[r["direction"]]
    flat, vertical = [], []
    for a, r in rooms.items():
        for b, cmd in r["wayto"].items():
            b, c = str(b), str(cmd).strip().lower()
            if b not in rooms or c.startswith(";e"):
                continue
            # Not a walk: `urchin guide ...` runs from a town's Urchin Hideout to
            # every shop in it (8 hideouts, 35-46 exits each, 429 in all), and
            # `ask <npc> about ...` moves you the same way. Crossed as flat they
            # glued Solhaven's shops into ONE 155-room indoor level.
            if c.startswith(("urchin ", "ask ")) and "--teleports" not in sys.argv:
                continue
            porch = PORCH.search(rooms[a]["title"][0] + " " + rooms[b]["title"][0])
            if c in VERT and porch:                # the map's own `down` off a porch
                flat.append((a, b))
            elif c in VERT:
                vertical.append((a, b, VERT[c], "rule"))
            elif (a, b) in jev or (b, a) in jev:
                s = jev[(a, b)] if (a, b) in jev else -jev[(b, a)]
                # Steps up to a porch are level (author, 2026-09-21): a reader calls
                # `go steps` "up", the porch lands at +1, and its front door then
                # opens onto an interior at 0. --steps-flat widens this to every
                # `go steps` a reader judged; it is a measurement, not a decision.
                if porch or ("--steps-flat" in sys.argv and re.search(r"\bsteps?\b", c)):
                    s = 0
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
                if back in VERT and not porch:
                    vertical.append((a, b, -VERT[back], "rule"))
                elif c in ("out", "go out") or not FLOOR_CHANGING.search(c):
                    flat.append((a, b))
    return flat, vertical


SLOPE = re.compile(r"slop|declin|inclin|descen|ascen|downward|upward|steep|plunge|drops?\b|"
                   r"climbs?\b|rises?\b|rising|spiral|ramp|winds? (up|down)|"
                   r"lead(s|ing)? (up|down)|(turn|head|angl|cant|curv)\w* (up|down)", re.I)


def cut_slopes(rooms, out, flat, vertical):
    """The map's literal up/down between two INDOOR rooms is real, and a compass
    route between the same two rooms is the slope (author, 2026-09-21: Glaes Vein
    2240 says "a stark plunge down" one way and "a gentle decline" the other).
    So one compass move on each such route crosses the floor. Cut the move whose
    rooms have slope wording, nearest the far end; with none, the last move.
    A MAZE -- every room on the route has one description -- is left alone:
    its directions mean nothing. A cut move is not crossed at all."""
    adj = collections.defaultdict(set)
    for a, b in flat:
        if out[a] == out[b]:
            adj[a].add(b)
            adj[b].add(a)
    desc = lambda k: " ".join(rooms[k].get("description") or [])
    cuts, gone, done = [], set(), set()
    for a, b, s, src in vertical:
        if src != "rule" or out[a] or out[b] or (b, a) in done:
            continue
        done.add((a, b))
        for _ in range(12):
            prev, q = {a: None}, collections.deque([a])
            while q and b not in prev:
                x = q.popleft()
                for y in sorted(adj[x], key=int):
                    if y not in prev:
                        prev[y] = x
                        q.append(y)
            if b not in prev:
                break
            path = [b]
            while prev[path[-1]] is not None:
                path.append(prev[path[-1]])
            path.reverse()
            if len({desc(k) for k in path}) == 1:
                cuts.append({"kind": "maze", "from": a, "to": b, "step": s})
                break
            pairs = list(zip(path, path[1:]))
            worded = [p for p in pairs if SLOPE.search(desc(p[0])) or SLOPE.search(desc(p[1]))]
            x, y = (worded or pairs)[-1]
            adj[x].discard(y)
            adj[y].discard(x)
            gone |= {(x, y), (y, x)}
            cuts.append({"kind": "cut", "from": x, "to": y, "worded": bool(worded),
                         "command": rooms[x]["wayto"].get(y, rooms[x]["wayto"].get(int(y))),
                         "because": [a, b], "step": s})
    return [e for e in flat if e not in gone], cuts


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    cut = float(args[0]) if args else 0.93
    big = int(args[1]) if len(args) > 1 else 25
    rooms = {str(r["id"]): r for r in json.load(open(MAP, encoding="utf-8"))}
    out = {k: outdoors(r) for k, r in rooms.items()}
    flat, vertical = exits(rooms, cut)

    # A room you cannot walk into takes no part in anyone else's floor (author,
    # 2026-09-21: "urchin rooms and other rooms like that we can't actually access
    # should not be touching any of the level stuff"). WALKABLE = reachable from a
    # town centre without `;e true` placeholders, urchin guides or `ask` exits.
    # Exits joining a walkable room to a non-walkable one are dropped; the
    # non-walkable rooms still get floors among themselves, marked walkable=false.
    towns = [k for k, r in rooms.items() if "town" in (r.get("tags") or [])]
    walkable, q = set(towns), collections.deque(towns)
    while q:
        x = q.popleft()
        for y, c in rooms[x]["wayto"].items():
            y, c = str(y), str(c).strip().lower()
            if (y in rooms and y not in walkable and c not in (";e true", ";e false")
                    and not c.startswith(("urchin ", "ask "))):
                walkable.add(y)
                q.append(y)
    if "--teleports" not in sys.argv:
        n = len(flat) + len(vertical)
        flat = [e for e in flat if (e[0] in walkable) == (e[1] in walkable)]
        vertical = [e for e in vertical if (e[0] in walkable) == (e[1] in walkable)]
        print(f"walkable rooms {len(walkable)} of {len(rooms)} | exits dropped for joining "
              f"a walkable room to one that is not: {n - len(flat) - len(vertical)}")

    flat, cuts = cut_slopes(rooms, out, flat, vertical)
    with open(R + "floor2_slope_cuts.jsonl", "w", encoding="utf-8") as fh:
        for c in cuts:
            fh.write(json.dumps(c) + "\n")
    print(f"compass moves cut as slopes: {sum(c['kind'] == 'cut' for c in cuts)} | "
          f"maze loops left alone: {sum(c['kind'] == 'maze' for c in cuts)}")

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
    # SLOPE COMMANDS. The rule above covered only the map's literal up/down, so
    # Solhaven's Tumbledown Lane -- ONE cobbled street snaking down a cliffside,
    # "the uneven, downward sloping cobblestones", 8 rooms -- kept its literal
    # up/down flattened and was then split across floors -1, +1 and +2 by the
    # `go steps` between its segments (author, 2026-09-21: "I'm not sure
    # tumbledown lane is up/down"). Outdoors, steps cut into a hillside, a ramp or
    # a slope are the hill, exactly like `up`. Stairs, ladders and climbs onto a
    # structure are not, and BUILT still overrides. --no-outdoor-steps turns off.
    SLOPE_CMD = re.compile(r"\b(steps?|ramp|slope|incline|path|trail)\b", re.I)
    flat_cmd = lambda a, b: (
        str(rooms[a]["wayto"].get(b, rooms[a]["wayto"].get(int(b), ""))).strip().lower())
    # THE OUTDOOR CLUSTER RULE. Two outdoor rooms Simutronics draw on one plate
    # are on one floor (99/99 on the key), whatever a reader said -- this catches
    # Tumbledown Lane outright. It does NOT force a level change the other way:
    # "different cluster" is 64/67 but the 3 misses would be errors we cannot see,
    # and the up/down we already have is better evidence. --no-clusters turns off.
    cl = {} if "--no-clusters" in sys.argv else clusters(rooms)
    one_plate = lambda a, b: (a in cl and b in cl and cl[a] == cl[b])
    

    climbs = []
    plated = 0
    for a, b, s, src in vertical:
        built = BUILT.search(rooms[a]["title"][0] + " " + rooms[b]["title"][0])
        slope_cmd = (src == "llm" and "--no-outdoor-steps" not in sys.argv
                     and SLOPE_CMD.search(flat_cmd(a, b) + " " + flat_cmd(b, a)))
        if out[a] and out[b] and one_plate(a, b) and not built:
            parent[find(a)] = find(b)
            plated += 1
            continue
        if (out[a] and out[b] and (src == "rule" or slope_cmd) and not built
                and not (dead_end[(a, b)] and len(dead_end[(a, b)]) <= SMALL)):
            parent[find(a)] = find(b)
        else:
            climbs.append((a, b, s, src))
    print(f"outdoor up/down exits flattened by the official layout: {plated}")
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

    lfloor, lanchor, lsource = {}, {}, {}

    def walk(starts, anchor, only=None):
        for allowed in (("rule",), ("rule", "llm")):
            q = collections.deque(starts)
            while q:
                x = q.popleft()
                for y, s, src in ledges[x]:
                    if src in allowed and y not in lfloor and (only is None or only(y)):
                        lfloor[y], lanchor[y] = lfloor[x] + s, anchor
                        lsource[y] = "llm" if (src == "llm"
                                               or lsource.get(x) == "llm") else "rule"
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
        lfloor[l], lanchor[l], lsource[l] = 0, "main deck", "rule"
    for l in members:
        if l in in_pocket or l in lfloor:
            continue
        if is_out[l] and (l in ground or len(members[l]) >= big):
            lfloor[l], lanchor[l], lsource[l] = 0, "outdoor ground", "rule"
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
            lfloor[l], lanchor[l], lsource[l] = 0, "outdoor ground", "rule"
            walk([l], "outdoor climb", only=lambda y: is_out[y])

    # 2. indoors, from their doorways
    # Repeated until nothing moves. Run once, a balcony took its floor from one
    # building and the SECOND building opening onto it was never anchored from it:
    # 61 of the 104 indoor/outdoor mismatches were that, an indoor side left at a
    # default 0.
    #
    # THE NAME LIST BREAKS THE TIE between a plain door and an up/down (author's
    # room-name classification, results/name_levels.json). A level whose names
    # Jev called below or above ground -- a Cellar, the Catacombs, a Garret --
    # does not take floor 0 from a door: the Wayside Inn Garret has `go grate
    # under ashes` to the street, the Abbey Cellar a door to the kitchen garden,
    # and the 120-room Catacombs a one-way drop in from the stables. Such a level
    # waits for the up/down walk; only if that never reaches it does a door count.
    # name_levels_all.json is every title in the map (20,226), which is what the
    # author asked for in the first place; name_levels.json is the 2,744 I ran.
    nfile = R + ("name_levels_all.json" if os.path.exists(R + "name_levels_all.json")
                 and "--names-small" not in sys.argv else "name_levels.json")
    names = json.load(open(nfile, encoding="utf-8"))
    print(f"name list: {nfile} ({len(names)} names)")
    off_ground = set()
    if "--no-names" not in sys.argv:
        for l, ks in members.items():
            if not is_out[l]:
                v = collections.Counter(
                    n["choice"] for n in (names.get((rooms[k].get("title") or [""])[0])
                                          for k in ks) if n and n["p"] >= 0.9)
                # The author's rule: rooms on one level are on one level, so ONE
                # room called Cellar makes the level a cellar. It looked wrong at
                # first (shops at -1) only because the urchin guides had glued
                # whole towns into fake levels. --names-half is the cautious
                # version kept for comparison: half the level named, no ground names.
                top = max(v["below ground"], v["above ground"])
                both = min(v["below ground"], v["above ground"])
                if "--names-half" in sys.argv:
                    ok = top * 2 >= len(ks) and not v["ground level"] and not both
                else:
                    ok = top > v["ground level"] and not both
                if ok:
                    off_ground.add(l)
    print(f"indoor levels whose names say off the ground: {len(off_ground)}")
    waiting = set(off_ground)
    while True:
        before = len(lfloor)
        touch = collections.defaultdict(list)
        for a, b in doorways:
            i, o = (b, a) if out[a] else (a, b)
            if level[o] in lfloor and level[i] not in lfloor and level[i] not in waiting:
                touch[level[i]].append(lfloor[level[o]])
        for l, fs in sorted(touch.items(), key=lambda t: int(t[0])):
            lfloor[l], lanchor[l] = collections.Counter(fs).most_common(1)[0][0], "doorway"
            lsource[l] = "rule"
        walk(list(lfloor), "stack")
        for a, b in doorways:             # a waiting balcony level with its room
            i, o = (b, a) if out[a] else (a, b)
            if level[o] not in lfloor and level[i] in lfloor:
                lfloor[level[o]], lanchor[level[o]] = lfloor[level[i]], "doorway"
                lsource[level[o]] = lsource.get(level[i], "rule")
        walk(list(lfloor), "stack")
        if len(lfloor) == before:
            if not waiting:
                break
            waiting = set()               # nothing walked to them: a door counts

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
            lfloor[base], lanchor[base], lsource[base] = 0, "default", "rule"
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

    # CONFIDENCE, per room. "anchor=default" alone said only "we guessed", which is
    # not good enough for 1,886 rooms (author, 2026-09-21). A room's floor is only
    # as good as the weakest link on the way to it, so grade the whole chain:
    #   certain    outdoor ground or a ship's main deck -- 0 by definition
    #   strong     reached only over the map's own up/down and plain doors
    #   moderate   a reader (Jev/subagent at the 93% gate) is on the chain
    #   weak       the level was placed by a plain door but its NAME says otherwise,
    #              or a conflict touches it
    #   guess      nothing anchors this stack; the whole area floats (anchor=default)
    # A room can be `guess` and still be right RELATIVE to its area: see 11p.
    conflicted = {x["from"] for x in conflicts} | {x["to"] for x in conflicts}
    # a level whose NAMES say below/above ground but which a plain door put on a
    # floor anyway (rule 10 let it fall through): the door may be the Garret's grate
    name_says_otherwise = {l for l in off_ground
                           if lanchor.get(l) == "doorway" and lfloor.get(l) == 0}
    grade = {}
    for k in rooms:
        l = level[k]
        a, src = lanchor[l], lsource.get(l, "rule")
        if a == "default":
            g = "guess"
        elif a in ("outdoor ground", "main deck") and src == "rule":
            g = "certain"
        elif src == "llm":
            g = "moderate"
        else:
            g = "strong"
        if g in ("strong", "certain") and (k in conflicted or l in name_says_otherwise):
            g = "weak"
        grade[k] = g
    json.dump({k: {"floor": floor[k], "anchor": lanchor[level[k]],
                   "confidence": grade[k], "walkable": k in walkable}
               for k in sorted(rooms, key=int)}, open(R + "floors2.json", "w"), indent=0)
    gc = collections.Counter(grade[k] for k in rooms if k in walkable)
    print("walkable rooms by confidence: " + " | ".join(
        f"{g} {gc[g]}" for g in ("certain", "strong", "moderate", "weak", "guess") if gc[g]))
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
