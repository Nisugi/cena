"""An answer key for the pathfinder, computed from the UPSTREAM file by an
independent implementation. plan/21 step 5.

It walks only exits whose `wayto` is a plain command and whose `timeto` is a
number -- the same exits `cena_map::route::as_converted` admits -- so the Rust
search over the converted binary must agree with it to the last digit's
tolerance. It does not reproduce Lich's routes: Lich also crosses scripted
exits, and those arrive with plan/21 step 7.

usage: python route_key.py <map.json> > route_key.tsv
"""
import heapq
import json
import sys

rooms = json.load(open(sys.argv[1], encoding="utf-8"))
graph = {}
for r in rooms:
    out = []
    for to, cmd in (r.get("wayto") or {}).items():
        cost = (r.get("timeto") or {}).get(to)
        if cmd.startswith(";e") or isinstance(cost, (str, bool)) or cost is None:
            continue
        out.append((int(to), float(cost)))
    graph[r["id"]] = out


def distances(src):
    dist = {src: 0.0}
    heap = [(0.0, src)]
    done = set()
    while heap:
        d, at = heapq.heappop(heap)
        if at in done:
            continue
        done.add(at)
        for to, cost in graph.get(at, ()):
            if to in graph and to not in done and d + cost < dist.get(to, float("inf")):
                dist[to] = d + cost
                heapq.heappush(heap, (d + cost, to))
    return dist


# Town centres and a few far corners; destinations every 97th room id.
SOURCES = [228, 3668, 1438, 10861, 1932, 3542, 13048, 2300, 28813, 0]
print("from\tto\tseconds")
for src in SOURCES:
    dist = distances(src)
    for r in rooms[::97]:
        d = dist.get(r["id"])
        print(f"{src}\t{r['id']}\t{'-' if d is None else repr(round(d, 6))}")
