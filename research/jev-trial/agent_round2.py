"""Second round for a Claude subagent: the exits the floor pass still cannot use.

Two kinds of item, both among WALKABLE rooms only (floors2.py rule 11):
  uncrossed   a height-word exit nobody has settled (Jev under the cut, the first
              subagent not confident enough), so floors2.py does not cross it
  conflict    an exit whose direction came from Jev or a subagent and which
              disagrees with the floors floors2.py gave its two ends

Asking the same question of the same text would get the same doubt. What is new
here is context the first round did not have: each room's COMPUTED FLOOR with how
it was arrived at, and Jev's verdict on each room NAME (all 20,226 now classified).
Both are labelled as fallible.

No room ids, no way-back command, no exits line, and the judged exit is left out
of both rooms' exit lists, as in agent_test.py. There is no answer key for these.

Usage:
    python agent_round2.py build [batch_size]
    python agent_round2.py merge          # answers -> results/job1_round2.jsonl
"""

import collections
import glob
import json
import os
import random
import sys

import agent_test as A
import classify as C
import floors2 as F

OUT = "results/agent_round2/"
HOW = {"outdoor ground": "it is outdoor ground, which is floor 0 by definition",
       "main deck": "it is on a level with outdoor ground or a ship's main deck (floor 0)",
       "doorway": "a plain door or compass move joins its level to an outdoor room on that floor",
       "stack": "counted up/down from a level placed another way",
       "outdoor climb": "counted up/down from outdoor ground by a climb",
       "default": "A GUESS - nothing anchors this part of the map, so treat the number as unknown"}


def items():
    rooms = C.rooms
    floors = json.load(open(C.R + "floors2.json"))
    flat, vert = F.exits(rooms, 0.93)
    known = {(a, d) for a, d in flat} | {(a, d) for a, d, _, _ in vert}
    walk = lambda a, d: floors[a]["walkable"] and floors[d]["walkable"]
    got = []
    for a, r in rooms.items():
        for d, c in r["wayto"].items():
            d, c = str(d), str(c).strip().lower()
            if (d in rooms and a < d and (a, d) not in known and (d, a) not in known
                    and not c.startswith((";e", "urchin ", "ask ")) and walk(a, d)):
                got.append(("uncrossed", a, d))
    for l in open(C.R + "floor2_conflicts.jsonl", encoding="utf-8"):
        x = json.loads(l)
        if x["edge"] == "llm" and walk(x["from"], x["to"]):
            got.append(("conflict", x["from"], x["to"]))
    return got, floors


def block(label, rid, partner, floors, names):
    lines = A.room_block(label, rid, partner)
    f = floors[rid]
    lines.insert(2, f"    computed floor (may be wrong): {f['floor']:+d} - {HOW[f['anchor']]}")
    n = names.get((C.rooms[rid].get("title") or [""])[0])
    if n and n.get("choice") and n["choice"] != "cannot tell":
        lines.insert(3, f"    what the room's NAME suggests (may be wrong): {n['choice']} "
                        f"(confidence {n['p']:.2f})")
    return lines


def build(size):
    os.makedirs(OUT, exist_ok=True)
    for old in glob.glob(OUT + "batch_*.txt"):
        os.remove(old)
    got, floors = items()
    names = json.load(open(C.R + "name_levels_all.json", encoding="utf-8"))
    random.Random(23).shuffle(got)
    key = {}
    for n in range(0, len(got), size):
        lines = []
        for i, (kind, a, d) in enumerate(got[n:n + size], n + 1):
            item = f"R{i:04d}"
            cmd = C.rooms[a]["wayto"].get(d, C.rooms[a]["wayto"].get(int(d)))
            key[item] = {"kind": kind, "from": a, "to": d, "command": cmd}
            lines += [f"=== {item} ===", f"  command typed: {cmd}"]
            lines += block("ROOM LEFT", a, d, floors, names)
            lines += block("ROOM ARRIVED IN", d, a, floors, names)
            lines.append("")
        with open(f"{OUT}batch_{n // size + 1}.txt", "w", encoding="utf-8") as fh:
            fh.write("\n".join(lines))
    json.dump(key, open(OUT + "key.json", "w"), indent=0)
    print(collections.Counter(k for k, *_ in got), "in",
          len(glob.glob(OUT + "batch_*.txt")), "batches of up to", size)


def merge():
    key = json.load(open(OUT + "key.json"))
    rows = []
    for f in glob.glob(OUT + "answers_*.jsonl"):
        for l in open(f, encoding="utf-8"):
            if l.strip():
                a = json.loads(l)
                if a.get("item") in key:
                    rows.append({**key[a["item"]], "direction": a["direction"],
                                 "agent_confidence": a.get("confidence", 0),
                                 "agent_reason": a.get("reason", "")})
    with open(C.R + "job1_round2.jsonl", "w", encoding="utf-8") as fh:
        for r in rows:
            fh.write(json.dumps(r, ensure_ascii=False) + "\n")
    print(f"answered {len(rows)} of {len(key)}")
    for kind in ("uncrossed", "conflict"):
        g = [r for r in rows if r["kind"] == kind]
        print(kind, len(g), collections.Counter(r["direction"] for r in g).most_common(),
              "| confidence >= 90:", sum(r["agent_confidence"] >= 90 for r in g),
              "| 80-89:", sum(80 <= r["agent_confidence"] < 90 for r in g))


if __name__ == "__main__":
    if sys.argv[1] == "build":
        build(int(sys.argv[2]) if len(sys.argv) > 2 else 40)
    else:
        merge()
