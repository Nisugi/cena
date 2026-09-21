"""Job 3, services only: what kind of room is this?

The answer key is Lich's own service tags (`bank`, `gemshop`, ...), the ones
go2 navigates by. Rooms carrying two different kinds are dropped. A "none of
these" class is sampled from indoor rooms with no service tag, so the test is
not all shops; that class is slightly dirty, because untagged does not prove a
room is not a shop.

Usage:
    python job3.py build <map.json> results/job3_key.jsonl
    python job3.py ask   results/job3_key.jsonl results/job3_answers.jsonl [limit]
"""

import hashlib
import json
import os
import random
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor
from pathlib import Path

import requests

from ask import ENDPOINT, MODEL, ask

CACHE = Path(__file__).parent / "results" / "cache"
NONE = "none of these"
N_NONE = 300

# Lich tag -> (label, when it applies). `mail` is folded into `postoffice`.
KINDS = {
    "bank": ("bank", "A bank: tellers, deposits, withdrawals, notes."),
    "inn": ("inn", "An inn: a front desk or innkeeper where a traveller checks in or rents a room."),
    "gemshop": ("gemshop", "A gem shop or jeweler that buys and appraises gems."),
    "furrier": ("furrier", "A furrier who buys skins, pelts and hides."),
    "pawnshop": ("pawnshop", "A pawnshop that buys and sells used goods of all kinds."),
    "herbalist": ("herbalist", "A herbalist or healer's shop selling healing herbs and potions."),
    "alchemist": ("alchemist", "An alchemist's shop selling alchemical ingredients, flasks and potions."),
    "reagent shop": ("reagent shop", "A shop selling alchemy reagents specifically."),
    "general store": ("general store", "A general store selling everyday adventuring supplies."),
    "weaponshop": ("weaponshop", "A weapon shop."),
    "armorshop": ("armorshop", "An armor shop."),
    "fletcher": ("fletcher", "A fletcher or bowyer selling bows, arrows and fletching supplies."),
    "locksmith": ("locksmith", "A locksmith selling lockpicks or opening boxes."),
    "forge": ("forge", "A forge or smithy workshop where a character crafts weapons themselves."),
    "cobbling": ("cobbling", "A cobbler's workshop where a character crafts footwear themselves."),
    "dyers": ("dyers", "A dyer's shop or tent that dyes clothing and items."),
    "collectibles": ("collectibles", "A collectibles shop that stores and trades collectible items."),
    "consignment": ("consignment", "A consignment shop selling goods on behalf of players."),
    "movers": ("movers", "A moving service that ships a character's locker to another town."),
    "public locker": ("locker", "A locker room or locker annex where characters store belongings."),
    "postoffice": ("postoffice", "A post office or mail room."),
    "mail": ("postoffice", "A post office or mail room."),
    "exchange": ("exchange", "A currency exchange converting one realm's coins to another's."),
    "advguild": ("adventurer guild", "The Adventurer's Guild, where bounty tasks are given."),
    "npchealer": ("healer", "An NPC healer who heals wounds for a fee."),
    "npccleric": ("cleric", "An NPC cleric who raises the dead."),
    "chronomage": ("chronomage", "A Chronomage office selling travel between towns."),
    "stable": ("stable", "A stable for mounts and animals."),
}
CRITERIA = {label: desc for label, desc in KINDS.values()}
CRITERIA[NONE] = ("An ordinary room that is none of the services listed: a "
                  "hallway, home, guild room, street, tavern floor, or any "
                  "shop of a kind not listed.")


def setting(room):
    words = {p.lower()[:13] for p in room.get("paths") or []}
    if words == {"obvious exits"}:
        return "indoors"
    if words == {"obvious paths"}:
        return "outdoors"
    return None


def build(map_path, out_path):
    with open(map_path, encoding="utf-8") as fh:
        rooms = json.load(fh)
    items, untagged, conflicts = [], [], 0
    for r in rooms:
        tags = set(r.get("tags") or [])
        labels = {KINDS[t][0] for t in tags if t in KINDS}
        if len(labels) > 1:
            conflicts += 1
            continue
        if labels:
            items.append((r, labels.pop()))
        elif setting(r) == "indoors" and r.get("description") and \
                not any("urchin guide" in t for t in tags):
            untagged.append(r)
    random.seed(22)
    items += [(r, NONE) for r in random.sample(untagged, N_NONE)]
    with open(out_path, "w", encoding="utf-8") as fh:
        for r, label in items:
            fh.write(json.dumps({
                "id": str(r["id"]), "title": r.get("title") or [],
                "description": r.get("description") or [],
                "location": r.get("location"), "setting": setting(r),
                "answer": label,
            }, ensure_ascii=False) + "\n")
    import collections
    c = collections.Counter(l for _, l in items)
    print(f"key={len(items)} kinds={len(c)} dropped_conflicts={conflicts}")
    print(", ".join(f"{k} {v}" for k, v in c.most_common()))


def body_for(item):
    state = {"title": item["title"], "description": item["description"],
             "area": item["location"]}
    if item["setting"]:
        state["setting"] = item["setting"]
    return {
        "model": MODEL,
        "state": state,
        "questions": {"kind": {
            "type": "choice",
            "instructions": (
                "This is one room of a fantasy game world. Which service, if "
                "any, does this room provide? Pick a service only when the "
                "room itself is where that service is done; a corridor or "
                "entry hall of the same building is 'none of these'."),
            "criteria": CRITERIA,
        }},
    }


def ask_all(key_path, out_path, limit):
    key = os.environ.get("OPENROUTER_API_KEY")
    if not key:
        raise SystemExit("set OPENROUTER_API_KEY in the environment first")
    items = [json.loads(l) for l in open(key_path, encoding="utf-8")]
    if limit:
        items = items[:limit]
    CACHE.mkdir(parents=True, exist_ok=True)
    stats = {"asked": 0, "cost": 0.0, "in": 0, "out": 0}
    lock = threading.Lock()
    local = threading.local()

    def work(item):
        if not hasattr(local, "s"):
            local.s = requests.Session()
        body = body_for(item)
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
                except SystemExit as e:  # ask() does not retry 529
                    if "529" not in str(e) or attempt == 5:
                        raise
                    time.sleep(3 * (attempt + 1))
            cp.write_text(json.dumps(resp, ensure_ascii=False), encoding="utf-8")
            u = resp.get("usage") or {}
            with lock:
                stats["asked"] += 1
                stats["cost"] += u.get("cost") or 0.0
                stats["in"] += u.get("input_tokens") or 0
                stats["out"] += u.get("output_tokens") or 0
        a = (resp.get("answers") or {}).get("kind") or {}
        # Shaped for score.py: from/to/command identify the row.
        return {"from": item["id"], "to": "-",
                "command": (item["title"] or ["?"])[0],
                "expected": item["answer"], "choice": a.get("choice"),
                "confidence": a.get("confidence"),
                "probabilities": a.get("probabilities")}

    with open(out_path, "w", encoding="utf-8") as fh, \
            ThreadPoolExecutor(max_workers=4) as pool:
        for n, row in enumerate(pool.map(work, items), 1):
            fh.write(json.dumps(row, ensure_ascii=False) + "\n")
            if n % 100 == 0:
                print(f"{n}/{len(items)} asked={stats['asked']}", flush=True)
    print(f"done: {len(items)} items, {stats['asked']} asked, tokens "
          f"in={stats['in']} out={stats['out']} cost=${stats['cost']:.6f}")


if __name__ == "__main__":
    if sys.argv[1] == "build":
        build(sys.argv[2], sys.argv[3])
    else:
        ask_all(sys.argv[2], sys.argv[3],
                int(sys.argv[4]) if len(sys.argv) > 4 else None)
