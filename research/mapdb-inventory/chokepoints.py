"""Which unported shapes hold the map together? plan/21 step 7's order.

Plain exits are 90% of the map, but the rooms they reach from a town are a
fraction of it: a few scripted exits are the bridges. This ranks the shapes
STILL UNPORTED by rooms gained, greedily: port the one that opens the most new
rooms from the start room, then the next, until nothing gains.

It reads the converter's OUTPUT, so it knows what is already ported:

    cargo run --release -p cena-mapdb-convert -- <upstream.json> <dir>
    python chokepoints.py <dir> [start-room-id] [rounds]

An exit is blocked by an unported crossing, an unported cost, or both, and
opens only when every blocker is ported. A gated cost counts as open -- this asks
what the map could be for a walker who meets the gates -- except a way back gated
on a memory, which only returns the walker to where it came from.
"""
import csv
import json
import os
import sys

root = sys.argv[1]
start = int(sys.argv[2]) if len(sys.argv) > 2 else 228
rounds = int(sys.argv[3]) if len(sys.argv) > 3 else 15


def shapes(name):
    path = os.path.join(root, "report", name)
    with open(path, encoding="utf-8", newline="") as f:
        return {row["shape_id"]: (int(row["edges"]), row["shape"]) for row in csv.DictReader(f, delimiter="\t")}


text = {}
text.update({"go:" + k: v for k, v in shapes("unported_crossings.tsv").items()})
text.update({"cost:" + k: v for k, v in shapes("unported_costs.tsv").items()})

graph = {}
for rid in json.load(open(os.path.join(root, "index.json"), encoding="utf-8")):
    path = os.path.join(root, "rooms", "%03d" % (rid // 1000), "%d.json" % rid)
    room = json.load(open(path, encoding="utf-8"))
    out = []
    for exit in room.get("exits", []):
        cost = exit.get("cost")
        if cost is None:
            continue
        # A way back gated on a memory (plan/21 §4.4) leads only to where the
        # walker came from -- somewhere it has already been. Counting it open
        # would make Mist Harbor a hub between every town, which it is not.
        if isinstance(cost, dict) and "remembered" in json.dumps(cost.get("when", {})):
            continue
        blockers = set()
        if "unported" in exit:
            blockers.add("go:" + exit["unported"])
        if isinstance(cost, dict) and "unported" in cost:
            blockers.add("cost:" + cost["unported"])
        out.append((exit["to"], frozenset(blockers)))
    graph[rid] = out


def reach(ported):
    seen = {start}
    stack = [start]
    border = set()
    while stack:
        at = stack.pop()
        for to, blockers in graph.get(at, ()):
            if to in seen or to not in graph:
                continue
            missing = blockers - ported
            if missing:
                if len(missing) == 1:
                    border |= missing
                continue
            seen.add(to)
            stack.append(to)
    return seen, border


everything = {b for out in graph.values() for _, bs in out for b in bs}
print(f"start {start}; every exit open reaches {len(reach(everything)[0])} rooms")
ported = set()
seen, border = reach(ported)
print(f"{len(seen):>6}  as converted today (gated costs counted as open)")
for _ in range(rounds):
    best = max(((len(reach(ported | {b})[0]), b) for b in border), default=None)
    if best is None or best[0] == len(seen):
        break
    ported.add(best[1])
    gained = best[0] - len(seen)
    seen, border = reach(ported)
    edges, shape = text.get(best[1], (0, "?"))
    print(f"{len(seen):>6}  +{gained:<5} {edges:>4} exits  {best[1][:4]} {shape[:100]}")
