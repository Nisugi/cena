"""Which scripted shapes hold the map together? plan/21 step 7's order.

Plain exits are 90% of the map, but the rooms they reach from a town are a
fraction of it: a few scripted exits are the bridges. This ranks shapes by
rooms gained, greedily: port the shape that opens the most new rooms from the
start room, then the next, until nothing gains.

An exit is blocked by its scripted `wayto` shape, its scripted `timeto` shape,
or both; it opens only when every blocker is ported.

usage: python chokepoints.py <map.json> [start-room-id] [rounds]
"""
import json
import re
import sys

BS = chr(92)
Q = re.compile('"(?:[^"' + BS * 2 + ']|' + BS * 2 + '.)*"|' + "'(?:[^'" + BS * 2 + "]|" + BS * 2 + ".)*'")
RX = re.compile('/(?:[^/ ' + BS * 2 + ']|' + BS * 2 + '.)(?:[^/' + BS * 2 + ']|' + BS * 2 + '.)*/[a-z]*')
NUM = re.compile(BS + 'd+(?:' + BS + '.' + BS + 'd+)?')
ARR = re.compile(BS + '[(?:' + BS + 's*(?:N|nil|S)' + BS + 's*,?)+' + BS + 's*' + BS + ']')
WS = re.compile(BS + 's+')


def norm(s):
    s = ARR.sub('[..]', NUM.sub('N', RX.sub('R', Q.sub('S', s))))
    return WS.sub(' ', s).strip()


rooms = json.load(open(sys.argv[1], encoding="utf-8"))
start = int(sys.argv[2]) if len(sys.argv) > 2 else 228
rounds = int(sys.argv[3]) if len(sys.argv) > 3 else 15
ids = {r["id"] for r in rooms}

graph = {}
for r in rooms:
    out = []
    for to, cmd in (r.get("wayto") or {}).items():
        cost = (r.get("timeto") or {}).get(to)
        if cost is None or cost is False or int(to) not in ids:
            continue
        blockers = set()
        if cmd.startswith(";e"):
            blockers.add("go: " + norm(cmd))
        if isinstance(cost, str):
            blockers.add("cost: " + norm(cost))
        out.append((int(to), frozenset(blockers)))
    graph[r["id"]] = out


def reach(ported):
    seen = {start}
    stack = [start]
    border = set()
    while stack:
        at = stack.pop()
        for to, blockers in graph[at]:
            if to in seen:
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
ceiling = len(reach(everything)[0])
ported = set()
seen, border = reach(ported)
print(f"start {start}; every exit open reaches {ceiling} rooms")
print(f"{len(seen):>6}  plain exits only")
for _ in range(rounds):
    best = max(((len(reach(ported | {b})[0]), b) for b in border), default=None)
    if best is None or best[0] == len(seen):
        break
    ported.add(best[1])
    gained = best[0] - len(seen)
    seen, border = reach(ported)
    print(f"{len(seen):>6}  +{gained:<6} {best[1][:110]}")
