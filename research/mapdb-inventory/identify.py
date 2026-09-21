"""How identifiable is a room from what the game shows? plan/21 step 4.

usage: python identify.py <map.json>
"""
import json
import sys
from collections import defaultdict

rooms = json.load(open(sys.argv[1], encoding="utf-8"))
by_id = {r["id"]: r for r in rooms}


def texts(r):
    """Every (title, description, paths) the room has been seen with."""
    return {
        (t, d.strip(), p.strip())
        for t in r.get("title") or [""]
        for d in r.get("description") or [""]
        for p in r.get("paths") or [""]
    }


# 1. uids shared by several rooms: does text tell them apart?
owners = defaultdict(list)
for r in rooms:
    for u in r.get("uid") or []:
        owners[u].append(r["id"])
shared = {u: ids for u, ids in owners.items() if len(ids) > 1}
told_apart = 0
for u, ids in shared.items():
    sets = [texts(by_id[i]) for i in ids]
    if all(not (a & b) for n, a in enumerate(sets) for b in sets[n + 1:]):
        told_apart += 1
print(f"uids shared by several rooms      {len(shared)}")
print(f"  told apart by text alone        {told_apart}")

# 2. rooms the map has no uid for: the game still sends one, the map cannot
# look it up, so text is all there is.
no_uid = [r for r in rooms if not r.get("uid")]
multi = [r for r in rooms if "meta:map:multi-uid" in (r.get("tags") or [])]
print(f"rooms with no uid                 {len(no_uid)}")
print(f"rooms tagged multi-uid            {len(multi)}")

pool = no_uid + [r for r in multi if r.get("uid")]
seen = defaultdict(set)
for r in pool:
    for t in texts(r):
        seen[t].add(r["id"])


def verdict(r, key):
    rivals = set()
    for t in texts(r):
        rivals |= key(t)
    return len(rivals)


by_title = defaultdict(set)
for r in pool:
    for t in r.get("title") or [""]:
        by_title[t].add(r["id"])

unique_full = sum(1 for r in no_uid if verdict(r, lambda t: seen[t]) == 1)
unique_title = sum(1 for r in no_uid if verdict(r, lambda t: by_title[t[0]]) == 1)
print(f"  unique by title alone           {unique_title}")
print(f"  unique by title+desc+paths      {unique_full}")

# 3. of the rest, how many does one neighbour settle? A twin is settled when no
# single room has an exit to both it and a rival.
ambiguous = [r for r in no_uid if verdict(r, lambda t: seen[t]) > 1]
entrances = defaultdict(set)
for r in rooms:
    for to in (r.get("wayto") or {}):
        entrances[int(to)].add(r["id"])
settled = 0
for r in ambiguous:
    rivals = set()
    for t in texts(r):
        rivals |= seen[t]
    rivals.discard(r["id"])
    mine = entrances[r["id"]]
    if mine and all(not (mine & entrances[x]) for x in rivals):
        settled += 1
print(f"  ambiguous by text               {len(ambiguous)}")
print(f"    settled by the room come from {settled}")
print(f"    flagged check_location        {sum(1 for r in ambiguous if r.get('check_location'))}")
print(f"    carrying a peer tag           {sum(1 for r in ambiguous if any(t.startswith('peer ') or 'peer ' in t for t in r.get('tags') or []))}")
