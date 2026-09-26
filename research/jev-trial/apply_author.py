"""Exits the author settled himself.

These outrank Jev and every subagent: the author plays the game, has the in-game
map, and reads the room text the same way. One row per exit, with his reason.

`floors2.py` reads results/job1_kept_author.jsonl, which this writes, and applies
it last -- after Jev, after both subagent rounds.

Usage:
    python apply_author.py            # write the file and show what depends on each
"""

import json

R = "results/"

# from, to, direction, the author's reason
ANSWERS = [
    ("9579", "9580", "same floor",
     "Czeroth Caverns: 9580 says 'the tunnel continues downward to the SOUTHWEST' -- "
     "the slope is the southwest passage, not this climb. `climb rocks` only gets you "
     "over the debris pile that blocks the tunnel where the walls crumbled. The author's "
     "in-game map draws both rooms on one level. A sloping tunnel is one level (his rule). "
     "And: 'there's not many other rooms that would change a level, so it would make the "
     "area 2 levels when it should be one.'"),
    ("5822", "5823", "same floor",
     "Zaerthu/Zul Logoth: the wall sealed with granite and mortar 'to prevent anyone from "
     "proceeding further down the tunnel' shows this is a tunnel entrance through a wall, "
     "not a hole in the ground. The crevasse is a crack ALONG the wall that something "
     "unknown opened. You go through the wall, so it is the same floor."),
    ("2647", "2658", "down",
     "Altar of the Elder -> Stairs of Ice: a real level change down, the author agreeing "
     "with Jev and both subagents. MEASURED on the 553-exit key: the verb is not the "
     "separator he wondered about -- `climb steps` (7) and `go steps` (33) both change "
     "level 100% of the time; it is the NOUN that decides. `climb` never means same floor "
     "(0 of 66), but `go <stair noun>` does not either."),
]


def main():
    rows = [{"from": a, "to": b, "direction": d, "reason": why, "source": "author"}
            for a, b, d, why in ANSWERS]
    with open(R + "job1_kept_author.jsonl", "w", encoding="utf-8") as fh:
        for r in rows:
            fh.write(json.dumps(r, ensure_ascii=False) + "\n")
    rooms = {str(r["id"]): r for r in
             json.load(open("E:/Cena/reference/mapdb/map-1789942730.json", encoding="utf-8"))}
    for r in rows:
        t = lambda k: (rooms[k].get("title") or ["?"])[0]
        print(f"{r['direction']:10} {r['from']} {t(r['from'])} -> {r['to']} {t(r['to'])}")
    print(f"{len(rows)} exits settled by the author")


if __name__ == "__main__":
    main()
