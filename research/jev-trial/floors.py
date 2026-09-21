"""Give every room a floor number, by the author's rules (plan/21 section 3f).

  1. literal `up` / `down` commands, and the reverse leg of one      +1 / -1
  2. Jev's direction for an open exit at or above the cut            +1 / -1 / 0
  3. THE GROUND-FLOOR DEFAULT: everything else keeps you on your floor --
     compass moves, `out`, and plain portals (go door, go gate, go arch ...)
  not crossed: Ruby-script exits, and height-word exits (go stairs, climb
     ladder ...) that Jev was not sure enough about. Their direction is unknown.

Rooms joined by same-floor moves form a LEVEL. Levels joined by up/down form a
STACK. A stack holding a known ground room (ground.py's strict fill) is
anchored there at floor 0. A stack with no known ground room is anchored by
default: its level with the most outdoor rooms is floor 0 (largest level if it
has none outdoors), and every room in it is marked anchor=default.

A staircase counts as one floor. That is a convention; the numbers order rooms,
they do not measure height.

CONFLICTS are written out, never resolved here:
  same-level   an up/down exit whose two rooms the default puts on ONE level.
               Either a portal on the loop really changes level (go door into
               a cellar) or the up/down is wrong.
  stack        two routes between the same two levels that disagree.

Usage:
    python floors.py [cut]          # default cut 0.93
"""

import collections
import json
import sys

from build_keys2 import FLOOR_CHANGING

R = "results/"
MAP = "E:/Cena/reference/mapdb/map-1789942730.json"
COMPASS = {"north", "south", "east", "west", "northeast", "northwest",
           "southeast", "southwest", "n", "s", "e", "w", "ne", "nw", "se", "sw"}
VERT = {"up": 1, "u": 1, "down": -1, "d": -1}
STEP = {"up": 1, "down": -1, "same floor": 0}


def outdoors(room):
    return {p.lower()[:13] for p in room.get("paths") or []} == {"obvious paths"}


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    # --slopes-flat: an up/down with BOTH rooms outdoors is a slope on a
    # hillside, not a floor change, and is treated as a same-floor move.
    slopes_flat = "--slopes-flat" in sys.argv
    cut = float(args[0]) if args else 0.93
    rooms = {str(r["id"]): r for r in json.load(open(MAP, encoding="utf-8"))}
    rows = [json.loads(l) for l in open(R + "job1_classified.jsonl", encoding="utf-8")]
    kept = [r for r in rows if r["estimated_accuracy"] >= cut]
    rest = [r for r in rows if r["estimated_accuracy"] < cut]
    for name, part in (("job1_kept.jsonl", kept), ("job1_remaining.jsonl", rest)):
        with open(R + name, "w", encoding="utf-8") as fh:
            for r in part:
                fh.write(json.dumps(r, ensure_ascii=False) + "\n")
    jev = {(r["from"], r["to"]): STEP[r["direction"]] for r in kept}
    # Exits below the cut that a blind Opus subagent settled at the author's 93%
    # gate (README 11m): agrees with Jev at confidence >= 80, or confidence >= 90.
    try:
        extra = [json.loads(l) for l in open(R + "job1_kept_agent.jsonl", encoding="utf-8")]
    except FileNotFoundError:
        extra = []
    for r in extra:
        jev[(r["from"], r["to"])] = STEP[r["direction"]]
    print(f"exits settled by the subagent gate: {len(extra)}")

    # ---- classify every exit -------------------------------------------------
    flat, vertical = [], []          # (a, b) ; (a, b, step, source)
    skipped = collections.Counter()
    for a, r in rooms.items():
        for b, cmd in r["wayto"].items():
            b, c = str(b), str(cmd).strip().lower()
            if b not in rooms:
                continue
            if c.startswith(";e"):
                skipped["Ruby script exit"] += 1
            elif c in VERT:
                vertical.append((a, b, VERT[c], "rule"))
            elif (a, b) in jev or (b, a) in jev:
                s = jev[(a, b)] if (a, b) in jev else -jev[(b, a)]
                if s == 0:
                    flat.append((a, b))
                else:
                    vertical.append((a, b, s, "llm"))
            elif c in COMPASS or c in ("out", "go out") or not FLOOR_CHANGING.search(c):
                flat.append((a, b))
            else:
                back = str(rooms[b]["wayto"].get(a, "")).strip().lower()
                if back in VERT:
                    vertical.append((a, b, -VERT[back], "rule"))
                else:
                    skipped["height-word exit, direction unknown"] += 1

    if slopes_flat:
        # Only the map's literal up/down. A climb Jev read (climb tree, climb
        # ratlines, go ladder) is a real change of level even outdoors: the
        # blunt version flattened the Hanging Gardens and a crow's nest.
        is_slope = lambda v: (v[3] == "rule" and outdoors(rooms[v[0]])
                              and outdoors(rooms[v[1]]))
        slope = [(v[0], v[1]) for v in vertical if is_slope(v)]
        vertical = [v for v in vertical if not is_slope(v)]
        flat += slope
        print(f"--slopes-flat: {len(slope)} outdoor up/down exits treated as level")

    # ---- levels: union rooms joined by same-floor moves ----------------------
    parent = {k: k for k in rooms}

    def find(x):
        while parent[x] != x:
            parent[x] = parent[parent[x]]
            x = parent[x]
        return x

    for a, b in flat:
        parent[find(a)] = find(b)
    level = {k: find(k) for k in rooms}
    members = collections.defaultdict(list)
    for k, l in level.items():
        members[l].append(k)

    conflicts = []
    ledges = collections.defaultdict(list)       # level -> (level, step, source, a, b)
    for a, b, s, src in vertical:
        la, lb = level[a], level[b]
        if la == lb:
            conflicts.append({"kind": "same-level", "from": a, "to": b, "step": s,
                              "edge": src, "level_size": len(members[la])})
            continue
        ledges[la].append((lb, s, src, a, b))
        ledges[lb].append((la, -s, src, b, a))

    # ---- stacks: walk up/down between levels ---------------------------------
    ground = set(json.load(open(R + "ground.json")))
    seeded = {level[g] for g in ground}
    lfloor, lsource, lanchor = {}, {}, {}

    def walk(starts, anchor):
        for allowed in (("rule",), ("rule", "llm")):
            q = collections.deque(l for l in starts if l in lfloor)
            seen = set(q)
            while q:
                x = q.popleft()
                for y, s, src, a, b in ledges[x]:
                    if src not in allowed:
                        continue
                    if y in lfloor:
                        if lfloor[y] != lfloor[x] + s and y not in seen:
                            pass
                        continue
                    lfloor[y] = lfloor[x] + s
                    lsource[y] = "llm" if (src == "llm" or lsource[x] == "llm") else "rule"
                    lanchor[y] = anchor
                    seen.add(y)
                    q.append(y)

    for l in seeded:
        lfloor[l], lsource[l], lanchor[l] = 0, "rule", "ground"
    walk(seeded, "ground")

    # stacks with no known ground room: anchor on the most-outdoor level
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
        if l in lfloor:
            continue
        comp = stack_of(l)
        base = max(comp, key=lambda x: (sum(outdoors(rooms[k]) for k in members[x]),
                                        len(members[x])))
        lfloor[base], lsource[base], lanchor[base] = 0, "rule", "default"
        walk([base], "default")

    # stack conflicts: an up/down edge whose two levels disagree with its step
    # An exit whose direction is right but which crosses more than one floor --
    # a cliff, a rope from the ground to a +2 platform, a swing from a tree house
    # down to a lake -- is NOT a conflict (author, 2026-09-21). "One exit = one
    # floor" is only the default for placing a level nobody has reached yet.
    done = set()
    spans = []
    for x in ledges:
        for y, s, src, a, b in ledges[x]:
            if lfloor[y] == lfloor[x] + s or (b, a) in done:
                continue
            done.add((a, b))
            diff = lfloor[y] - lfloor[x]
            if diff * s > 0:
                spans.append({"from": a, "to": b, "direction": "up" if s > 0 else "down",
                              "floors_crossed": abs(diff), "edge": src,
                              "floor_from": lfloor[x], "floor_to": lfloor[y]})
            else:
                conflicts.append({"kind": "stack", "from": a, "to": b, "step": s,
                                  "edge": src, "floor_from": lfloor[x],
                                  "floor_to": lfloor[y]})
    with open(R + "floor_spans.jsonl", "w", encoding="utf-8") as fh:
        for sp in spans:
            fh.write(json.dumps(sp) + "\n")
    print(f"exits that cross more than one floor: {len(spans)}")

    out = {k: {"floor": lfloor[level[k]], "source": lsource[level[k]],
               "anchor": lanchor[level[k]]} for k in sorted(rooms, key=int)}
    json.dump(out, open(R + "floors.json", "w"), indent=0)
    with open(R + "floor_conflicts.jsonl", "w", encoding="utf-8") as fh:
        for c in conflicts:
            fh.write(json.dumps(c) + "\n")

    n = len(rooms)
    print(f"cut {cut:.2f}: Jev exits used {len(kept)}, left for later {len(rest)}")
    print(f"levels {len(members)} | up/down exits between levels "
          f"{sum(len(v) for v in ledges.values()) // 2}")
    print(f"rooms with a floor: {len(out)} of {n}")
    c = collections.Counter((v['anchor'], v['source']) for v in out.values())
    for (anc, src), v in sorted(c.items()):
        print(f"   anchored by {anc:8} reached by {src:5}: {v:6} ({v / n:.1%})")
    dist = collections.Counter(v["floor"] for v in out.values())
    print("floors:", " ".join(f"{k:+d}:{dist[k]}" for k in sorted(dist)))
    print("exits not crossed:", dict(skipped))
    ck = collections.Counter((x["kind"], x["edge"]) for x in conflicts)
    print("conflicts:", {f"{k} / {e} edge": v for (k, e), v in sorted(ck.items())})


if __name__ == "__main__":
    main()
