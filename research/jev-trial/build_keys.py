"""Job 1 answer key: which way does a non-compass exit go?

An exit A->B whose command is not a compass direction is "vertical-looking".
It has a KNOWN answer when the way back, B->A, is an explicit compass-free
vertical: the command "up" or "down". If B->A is "up", then A->B went down,
and vice versa.

Usage:
    python build_keys.py <map.json> <out.jsonl>
"""

import json
import sys

# Movement commands that carry their own answer, so the exit is not a puzzle.
COMPASS = {
    "north", "south", "east", "west",
    "northeast", "northwest", "southeast", "southwest",
    "n", "s", "e", "w", "ne", "nw", "se", "sw",
    "out", "go out",
}
VERTICAL = {"up": "up", "down": "down", "u": "up", "d": "down"}


def is_script(cmd):
    """Lich stores Ruby for some exits; those are not movement commands."""
    return cmd.strip().startswith(";e")


def load(path):
    with open(path, encoding="utf-8") as fh:
        rooms = json.load(fh)
    return {str(r["id"]): r for r in rooms}


def room_text(room):
    return {
        "id": str(room["id"]),
        "title": room.get("title") or [],
        "description": room.get("description") or [],
        "paths": room.get("paths") or [],
        "location": room.get("location"),
    }


def build(rooms):
    items = []
    seen = set()
    for rid, room in rooms.items():
        for dest, cmd in (room.get("wayto") or {}).items():
            if is_script(cmd):
                continue
            c = cmd.strip().lower()
            if c in COMPASS or c in VERTICAL:
                continue  # not vertical-looking; the command already says
            dest_room = rooms.get(str(dest))
            if dest_room is None:
                continue
            back = (dest_room.get("wayto") or {}).get(rid)
            if back is None or is_script(back):
                continue
            b = back.strip().lower()
            if b not in VERTICAL:
                continue
            # Going back is "up" => the forward exit went down.
            answer = "down" if VERTICAL[b] == "up" else "up"
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
                "answer": answer,
            })
    return items


def main():
    rooms = load(sys.argv[1])
    items = build(rooms)
    with open(sys.argv[2], "w", encoding="utf-8") as fh:
        for it in items:
            fh.write(json.dumps(it, ensure_ascii=False) + "\n")
    up = sum(1 for i in items if i["answer"] == "up")
    print(f"rooms={len(rooms)} key={len(items)} up={up} down={len(items) - up}")
    if items:
        print(f"baseline(always up)={up / len(items):.1%}")


if __name__ == "__main__":
    main()
