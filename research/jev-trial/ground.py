"""Derive ground level by flood fill from a known ground-level room.

The author's insight: if room 228 (Town Square Central) is ground level, then
any room reachable from it by an exit that cannot change elevation is also
ground level. A door, arch, gate or curtain keeps you on the floor you are on.
A staircase, ladder or climb does not.

The fill is deliberately conservative. An exit is crossed ONLY if its command
is recognised as flat. Anything unrecognised stops the fill, so an unknown
command can never silently paint a whole tower as ground level.

Usage:
    python ground.py <map.json> [seed_room_id ...] > results/ground.json
"""

import collections
import json
import re
import sys

COMPASS = {
    "north", "south", "east", "west",
    "northeast", "northwest", "southeast", "southwest",
    "n", "s", "e", "w", "ne", "nw", "se", "sw",
}

# Words that mean the exit may change FLOOR. If any appears, do not cross.
#
# Spans -- bridge, plank, pier, dock, walkway, catwalk, causeway -- are NOT
# here, and that is deliberate. The author's definition (2026-09-21): "going
# on to a bridge isn't really changing elevations ... we're not changing
# floors, we still end up on the same plane." A span is level with the ground
# at either end. See README section 11.
VERTICAL = re.compile(
    r"\b("
    r"up|down|upward|downward|upstairs|downstairs|above|below|over|under"
    r"|stair|stairs|staircase|stairway|stairwell|steps|step|stile"
    r"|ladder|rung|rope|cord|cords|vine|rungs"
    r"|climb|clamber|scale|descend|ascend|jump|leap|dive|crawl"
    r"|ramp|slope|incline|bank|hill|rise|ridge|dune"
    r"|tree|oak|branch|limb|trunk|canopy|platform|treehouse"
    r"|tower|turret|balcony|landing|loft|attic|roof|rooftop|parapet|battlement"
    r"|cellar|basement|crypt|undercroft|dungeon|vault"
    r"|pit|shaft|chimney|crevasse|crevice|fissure|chasm|ravine|gorge|gully"
    r"|cave|cavern|tunnel|burrow|hole|opening|mouth|well|mine"
    r"|cliff|ledge|ledges|outcrop|boulder|rock|face|wall|scaffolding"
    r"|dais|pedestal|altar|stage|porch|stoop|deck|mast|rigging|crow"
    r"|hatch|trapdoor|manhole|grate"
    r"|slide|chute|drop|fall|depression|hollow|basin|valley"
    r"|water|river|stream|pool|lake|surface|underwater|swim|wade"
    r")\b"
)


def is_script(cmd):
    return str(cmd).strip().startswith(";e")


def is_flat(cmd, strict=True):
    """True only when the command is recognised as elevation-preserving.

    strict=True crosses only compass moves and `out`. MEASURED: this produces
    zero contradictions against the 300-item answer key, because a compass
    move never changes floor.

    strict=False additionally crosses `go <thing>` where the thing is not in
    VERTICAL. It reaches 4x as many rooms and is WRONG about 14% of the key
    items it touches -- see README §8. `go door` into a cellar is flat by
    vocabulary and descends in fact. Use it only where an error is cheap.
    """
    s = str(cmd).strip().lower()
    if not s or is_script(s):
        return False
    if s in COMPASS or s == "out":
        return True
    # "pedal north" and friends: a vehicle on the level.
    if s.startswith("pedal ") and s.split()[-1] in COMPASS:
        return True
    if strict:
        return False
    if not (s.startswith("go ") or s.startswith("enter ")):
        return False
    return not VERTICAL.search(s)


def fill(rooms, seeds, strict=True):
    ground = set()
    q = collections.deque()
    for s in seeds:
        if s in rooms:
            ground.add(s)
            q.append(s)
    while q:
        rid = q.popleft()
        for dest, cmd in (rooms[rid].get("wayto") or {}).items():
            dest = str(dest)
            if dest in ground or dest not in rooms:
                continue
            if is_flat(cmd, strict):
                ground.add(dest)
                q.append(dest)
    return ground


STREET_WORDS = ("town square", "street", "road", "plaza", "courtyard",
                "crossing", "gate")


def street_seeds(rooms, min_compass=3):
    """Rooms that read as street level: a street-ish title and a real grid of
    compass exits. MEASURED: 328 rooms."""
    out = []
    for rid, r in rooms.items():
        title = " ".join(r.get("title") or []).lower()
        if not any(w in title for w in STREET_WORDS):
            continue
        n = sum(1 for v in (r.get("wayto") or {}).values()
                if str(v).strip().lower() in COMPASS)
        if n >= min_compass:
            out.append(rid)
    return out


def main():
    path = sys.argv[1]
    args = sys.argv[2:]
    loose = "--loose" in args
    seeds = [a for a in args if not a.startswith("--")]
    with open(path, encoding="utf-8") as fh:
        rooms = {str(r["id"]): r for r in json.load(fh)}
    if not seeds:
        seeds = street_seeds(rooms)
    ground = fill(rooms, seeds, strict=not loose)
    sys.stderr.write(
        f"seeds={len(seeds)} strict={not loose} "
        f"ground={len(ground)} of {len(rooms)} "
        f"({len(ground) / len(rooms):.1%})\n"
    )
    print(json.dumps(sorted(ground, key=int)))


if __name__ == "__main__":
    main()
