"""Job 1, key 2: exits that are known to be FLAT.

Key 1 (build_keys.py) finds exits known to be vertical, because the way back
is an explicit `up` or `down`. This finds the opposite: exits whose way back
is an explicit COMPASS direction, which means the two rooms sit on one plane.

The definition of flat is the author's, and it is about FLOORS, not physical
elevation (2026-09-21):

    "going on to a bridge isn't really changing elevations, even though in the
     reality sense you are, but we're not changing floors, we still end up on
     the same plane"

So a bridge, walkway, catwalk, pier or gangplank is FLAT. You walk out onto it
and you are on the same level you left. Only things that move you between
floors -- stairs, ladders, climbs, shafts, cellars -- are vertical.

Usage:
    python build_keys2.py <map.json> <out.jsonl>
"""

import json
import re
import sys

from build_keys import COMPASS, VERTICAL, is_script, room_text

# Compass directions proper -- the return legs that prove a flat exit.
# `out` is excluded: it is a container exit, not a direction, and says
# nothing about which plane you end on.
COMPASS_PROPER = {
    "north", "south", "east", "west",
    "northeast", "northwest", "southeast", "southwest",
    "n", "s", "e", "w", "ne", "nw", "se", "sw",
}

# Things that move you between floors. A command naming one of these is not
# trusted as flat even when its return leg is a compass direction, because the
# map may simply be missing the vertical return.
FLOOR_CHANGING = re.compile(
    r"\b("
    r"up|down|upstairs|downstairs"
    r"|stair|stairs|staircase|stairway|stairwell|steps|step"
    r"|ladder|rung|rope|climb|ascend|descend"
    r"|tree|oak|branch|limb|canopy"
    r"|tower|turret|balcony|loft|attic|roof|parapet|battlement"
    r"|cellar|basement|crypt|undercroft|vault|dungeon"
    r"|pit|shaft|chimney|crevasse|fissure|chasm|ravine|gorge"
    r"|cave|cavern|tunnel|burrow|hole|well|mine"
    r"|cliff|ledge|outcrop|outcropping|boulder|boulders|scaffolding"
    r"|dais|pedestal|altar|stage"
    r"|mast|rigging|hatch|trapdoor"
    r"|slope|ramp|hill|bank|depression|hollow|reef"
    r")\b"
)


def build(rooms):
    items = []
    seen = set()
    for rid, room in rooms.items():
        for dest, cmd in (room.get("wayto") or {}).items():
            if is_script(cmd):
                continue
            c = cmd.strip().lower()
            if c in COMPASS or c in VERTICAL:
                continue
            dest_room = rooms.get(str(dest))
            if dest_room is None:
                continue
            back = (dest_room.get("wayto") or {}).get(rid)
            if back is None or is_script(back):
                continue
            if back.strip().lower() not in COMPASS_PROPER:
                continue
            if FLOOR_CHANGING.search(c):
                continue  # suspect: may be vertical with a missing return
            k = (rid, str(dest), cmd)
            if k in seen:
                continue
            seen.add(k)
            items.append({
                "job": 1,
                "from": room_text(room),
                "to": room_text(dest_room),
                "command": cmd,
                "back_command": back,
                "answer": "same floor",
            })
    return items


def main():
    with open(sys.argv[1], encoding="utf-8") as fh:
        rooms = {str(r["id"]): r for r in json.load(fh)}
    items = build(rooms)
    with open(sys.argv[2], "w", encoding="utf-8") as fh:
        for it in items:
            fh.write(json.dumps(it, ensure_ascii=False) + "\n")
    print(f"rooms={len(rooms)} flat_key={len(items)}")


if __name__ == "__main__":
    main()
