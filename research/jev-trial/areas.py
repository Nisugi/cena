"""The area list: every walkable room assigned to an area, with its floor.

The unit is Simutronics' own layout where one exists (123 areas, from
map-data/prime/layouts.json, matched to Lich rooms by uid), and the mapdb
`location` field everywhere else. Neither alone is right: 66 official areas
span several locations and 53 locations span several official areas.

Writes:
  results/areas.tsv          one row per AREA -- rooms, floors, confidence
  results/area_rooms.tsv     one row per ROOM -- area, room id, name, floor
  results/areas.json         the same, machine-readable, area -> room ids

Usage:
    python areas.py
"""

import collections
import csv
import json

import floors2 as F


def main():
    rooms = {str(r["id"]): r for r in json.load(open(F.MAP, encoding="utf-8"))}
    fl = json.load(open(F.R + "floors2.json"))
    cl = F.clusters(rooms)
    T = lambda k: (rooms[k].get("title") or ["?"])[0]
    walk = [k for k in rooms if fl[k]["walkable"]]

    area, source = {}, {}
    for k in walk:
        if k in cl:
            area[k], source[k] = cl[k][0], "official layout"
        else:
            area[k], source[k] = (rooms[k].get("location") or "(no location)"), "mapdb location"

    members = collections.defaultdict(list)
    for k in walk:
        members[area[k]].append(k)

    ORDER = ["certain", "strong", "moderate", "weak", "guess"]
    rows = []
    for a, ks in members.items():
        f = collections.Counter(fl[k]["floor"] for k in ks)
        c = collections.Counter(fl[k]["confidence"] for k in ks)
        rows.append({
            "area": a,
            "source": source[ks[0]],
            "rooms": len(ks),
            "floors": " ".join(f"{n:+d}:{f[n]}" for n in sorted(f)),
            "floor_span": max(f) - min(f),
            "worst_confidence": next(g for g in reversed(ORDER) if c[g]),
            **{g: c[g] for g in ORDER},
            "room_ids": " ".join(sorted(ks, key=int)),
        })
    rows.sort(key=lambda r: (-r["floor_span"], -r["rooms"]))
    with open(F.R + "areas.tsv", "w", encoding="utf-8", newline="") as fh:
        w = csv.DictWriter(fh, fieldnames=list(rows[0]), delimiter="\t")
        w.writeheader()
        w.writerows(rows)

    with open(F.R + "area_rooms.tsv", "w", encoding="utf-8", newline="") as fh:
        w = csv.writer(fh, delimiter="\t")
        w.writerow(["area", "source", "room_id", "room_name", "floor", "confidence"])
        for a in sorted(members, key=lambda a: (-len(members[a]), a)):
            for k in sorted(members[a], key=int):
                w.writerow([a, source[k], k, T(k), fl[k]["floor"], fl[k]["confidence"]])

    json.dump({a: sorted(ks, key=int) for a, ks in members.items()},
              open(F.R + "areas.json", "w"), indent=0)

    n = collections.Counter(source[k] for k in walk)
    print(f"{len(members)} areas over {len(walk)} walkable rooms "
          f"({n['official layout']} from the official layouts, {n['mapdb location']} from location)")
    print(f"  areas on ONE floor: {sum(1 for r in rows if r['floor_span'] == 0)}"
          f" | spanning 2-3: {sum(1 for r in rows if 1 <= r['floor_span'] <= 2)}"
          f" | 4+: {sum(1 for r in rows if r['floor_span'] >= 3)}")
    print("  areas whose worst room is:", dict(collections.Counter(r["worst_confidence"] for r in rows)))
    print("\n  widest floor spans (these are where the elevation is least believable):")
    for r in rows[:10]:
        print(f"    {r['area'][:42]:42} {r['rooms']:5} rooms  span {r['floor_span']:3}  {r['floors'][:46]}")


if __name__ == "__main__":
    main()
