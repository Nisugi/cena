"""Split the exits below the cut into those worth settling and those set aside.

Author's decision, 2026-09-21: an exit touching a room that cannot be reached
from ANY town centre is dropped from the work list with a note, and so is an
exit touching a room Lich tags `closed`, `gone` or `missing`.

Reachability follows every exit in the map, scripted ones included, from all
rooms tagged `town`. It cannot see places entered by ticket, boat or one-way
portal, so "unreachable" means "not connected in this map file".

Usage:
    python set_aside.py     # reads results/job1_remaining.jsonl
"""

import collections
import json

R = "results/"
MAP = "E:/Cena/reference/mapdb/map-1789942730.json"
FLAG = {"closed", "gone", "missing"}


def main():
    rooms = {str(r["id"]): r for r in json.load(open(MAP, encoding="utf-8"))}
    towns = [k for k, r in rooms.items() if "town" in (r.get("tags") or [])]
    seen, q = set(towns), collections.deque(towns)
    while q:
        x = q.popleft()
        for y in rooms[x]["wayto"]:
            y = str(y)
            if y in rooms and y not in seen:
                seen.add(y)
                q.append(y)

    def reason(row):
        for rid in (row["from"], row["to"]):
            if rid not in seen:
                return f"room {rid} cannot be reached from any town centre in this map"
        for rid in (row["from"], row["to"]):
            tags = FLAG & set(rooms[rid].get("tags") or [])
            if tags:
                return f"room {rid} is tagged {', '.join(sorted(tags))} in the Lich map"
        return None

    rows = [json.loads(l) for l in open(R + "job1_remaining.jsonl", encoding="utf-8")]
    settle, aside = [], []
    for row in rows:
        why = reason(row)
        if why:
            aside.append({**row, "set_aside": why})
        else:
            settle.append(row)
    for name, part in (("job1_to_settle.jsonl", settle), ("job1_set_aside.jsonl", aside)):
        with open(R + name, "w", encoding="utf-8") as fh:
            for row in part:
                fh.write(json.dumps(row, ensure_ascii=False) + "\n")
    kinds = collections.Counter("unreachable" if "cannot be reached" in a["set_aside"]
                                else "tagged closed/gone/missing" for a in aside)
    print(f"below the cut: {len(rows)} | to settle: {len(settle)} | set aside: {len(aside)} {dict(kinds)}")


if __name__ == "__main__":
    main()
