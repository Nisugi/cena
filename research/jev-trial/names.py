"""Classify a room NAME alone: is a place called this typically below ground,
at ground level, or above it?

The author's idea (2026-09-21), replacing a hand-written word list: let the
model judge every name, so words the list missed (hold, bilge, aerie, ...) are
covered. One call per distinct name, cached.

Usage:
    python names.py ask <titles.json> results/name_levels.json
"""

import hashlib
import json
import os
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import requests

from ask import MODEL, ask

CACHE = Path(__file__).parent / "results" / "cache"

CRITERIA = {
    "below ground": "A place with this name is normally under the surface or "
                    "on a lower level: a cellar, basement, crypt, catacomb, "
                    "tunnel, sewer, mine, cave, pit, dungeon, vault, a ship's "
                    "hold or bilge, the floor of a ravine.",
    "ground level": "A place with this name is normally at the surface or on "
                    "the main floor: a street, square, road, field, forest "
                    "floor, shop floor, entry hall, courtyard, dock, ship's "
                    "main deck.",
    "above ground": "A place with this name is normally raised above the main "
                    "level: an upper floor, loft, attic, tower top, roof, "
                    "balcony, battlement, treetop, platform in a tree, crow's "
                    "nest, summit, clifftop.",
    "cannot tell": "The name gives no real indication of height: a hallway, "
                   "room, chamber, path or trail that could be on any level.",
}


def body_for(title):
    return {
        "model": MODEL,
        "state": {"room_name": title},
        "questions": {"level": {
            "type": "choice",
            "instructions": (
                "This is the name of one room in a fantasy game world, in the "
                "form [Area, Room]. Judging by the name alone, is a place "
                "called this typically below ground, at ground level, or "
                "above ground? Answer 'cannot tell' unless the name itself "
                "points to a height."),
            "criteria": CRITERIA,
        }},
    }


def main():
    key = os.environ.get("OPENROUTER_API_KEY")
    if not key:
        raise SystemExit("set OPENROUTER_API_KEY in the environment first")
    titles = json.load(open(sys.argv[2], encoding="utf-8"))
    CACHE.mkdir(parents=True, exist_ok=True)
    stats = {"asked": 0, "cost": 0.0, "in": 0, "out": 0}
    lock = threading.Lock()
    local = threading.local()

    def work(title):
        if not hasattr(local, "s"):
            local.s = requests.Session()
        body = body_for(title)
        h = hashlib.sha256(json.dumps(
            body, sort_keys=True, ensure_ascii=False).encode("utf-8")).hexdigest()
        cp = CACHE / f"{h}.json"
        if cp.exists():
            resp = json.loads(cp.read_text(encoding="utf-8"))
        else:
            for attempt in range(6):
                try:
                    resp = ask(local.s, body, key)
                    break
                except SystemExit as e:
                    if attempt == 5:
                        return title, {"error": str(e)[:160]}
                    time.sleep(3 * (attempt + 1))
            cp.write_text(json.dumps(resp, ensure_ascii=False), encoding="utf-8")
            u = resp.get("usage") or {}
            with lock:
                stats["asked"] += 1
                stats["cost"] += u.get("cost") or 0.0
                stats["in"] += u.get("input_tokens") or 0
                stats["out"] += u.get("output_tokens") or 0
        a = (resp.get("answers") or {}).get("level") or {}
        return title, {"choice": a.get("choice"),
                       "p": (a.get("probabilities") or {}).get(a.get("choice"), 0)}

    out = {}
    with ThreadPoolExecutor(max_workers=4) as pool:
        for n, (t, v) in enumerate(pool.map(work, titles), 1):
            out[t] = v
            if n % 500 == 0:
                print(f"{n}/{len(titles)} asked={stats['asked']}", flush=True)
    json.dump(out, open(sys.argv[3], "w", encoding="utf-8"),
              ensure_ascii=False, indent=0)
    print(f"done: {len(titles)} names, {stats['asked']} asked, tokens "
          f"in={stats['in']} out={stats['out']} cost=${stats['cost']:.6f}")


if __name__ == "__main__":
    main()
