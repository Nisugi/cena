"""Ask Jev one job's answer key, via OpenRouter's Decisions endpoint.

Every answer is cached under results/cache/ by a hash of exactly the request
body that produced it, so a re-run costs nothing and re-asks nothing. The key
is read from the environment and never written anywhere.

Usage:
    python ask.py results/job1_key.jsonl results/job1_answers.jsonl [limit]
"""

import hashlib
import json
import os
import sys
import time
from pathlib import Path

import requests

GROUND = set(json.load(open(
    Path(__file__).parent / "results" / "ground.json", encoding="utf-8")))

ENDPOINT = "https://openrouter.ai/api/alpha/decisions"
MODEL = "typesafe/jev-1.13"
CACHE = Path(__file__).parent / "results" / "cache"

CRITERIA = {
    "up": "The character ends up HIGHER than they started. Typical of climbing "
          "a tree, ladder, staircase, rope or cliff face; stepping up onto a "
          "porch, dais, platform, ledge, landing, balcony or wall walk; or "
          "entering an upper room of a tower or building.",
    "down": "The character ends up LOWER than they started. Typical of "
            "descending into a pit, shaft, chimney, crevasse, ravine, tunnel, "
            "cellar or cave mouth; going down a slope, bank or trail into a "
            "valley; or entering a lower room of a tower or building. A "
            "feature described as BELOW the character, or that they look or "
            "peer DOWN into, is reached by going down.",
    "same floor": "The character stays at the SAME height. The two rooms are "
                  "side by side: both on the street, both at water level, both "
                  "on one floor of a building. Choose this only when neither "
                  "room is plainly above the other.",
}

# Prepended to the instructions. These target the failure modes measured in
# run 1: 26 of 34 mistakes were polarity inversions (right that the exit is
# vertical, wrong about which end), and accuracy fell to 75% when both rooms
# share a title.
GUIDANCE = (
    "Work out which room is physically higher. Read the room the character "
    "LEAVES first: if it describes the feature as above them (overhead, rising, "
    "towering) they are going up; if it describes it as below them (a pit, a "
    "drop, a floor far below, something they peer down into) they are going "
    "down. The two rooms may share the same title and read similarly -- that "
    "does not mean they are on the same floor, so judge by what each "
    "description says about height. A room marked known_elevation 'ground "
    "level' sits on the street or the base floor: you cannot go further down "
    "to reach the street from it, and a room above it is reached by going up. "
    "Note that going DOWN from a ground level room is still possible, into a "
    "cellar, crypt or tunnel."
)


def state_for(item):
    """What Jev sees. Both ends of the exit, so it can judge from either."""

    def side(r):
        # The exits line is DELIBERATELY omitted. In 245 of 300 key items it
        # names exactly one vertical direction, and it is the inverse of the
        # answer -- because the key selects exits whose return leg is up/down
        # and the exits line advertises that same exit. Sending it hands over
        # the answer. See README section 8.
        out = {
            "title": r["title"],
            "description": r["description"],
            "area": r["location"],
        }
        if r["id"] in GROUND:
            out["known_elevation"] = "ground level"
        return out

    return {
        "command_typed": item["command"],
        "room_departed": side(item["from"]),
        "room_arrived": side(item["to"]),
    }


def body_for(item):
    return {
        "model": MODEL,
        "state": state_for(item),
        "questions": {
            "direction": {
                "type": "choice",
                "instructions": (
                    "A character in room_departed types command_typed and "
                    "arrives in room_arrived. Relative to where they started, "
                    "did they go up, go down, or stay on the same floor? "
                    + GUIDANCE
                ),
                "criteria": CRITERIA,
            }
        },
    }


def cache_path(body):
    h = hashlib.sha256(
        json.dumps(body, sort_keys=True, ensure_ascii=False).encode("utf-8")
    ).hexdigest()
    return CACHE / f"{h}.json"


def ask(session, body, key):
    """POST once, with a short backoff on rate limits and 5xx."""
    for attempt in range(5):
        resp = session.post(
            ENDPOINT,
            headers={
                "Authorization": f"Bearer {key}",
                "Content-Type": "application/json",
            },
            json=body,
            timeout=60,
        )
        if resp.status_code == 200:
            return resp.json()
        if resp.status_code in (429, 500, 502, 503, 504):
            time.sleep(2 ** attempt)
            continue
        raise SystemExit(f"HTTP {resp.status_code}: {resp.text[:500]}")
    raise SystemExit("gave up after 5 attempts")


def main():
    key = os.environ.get("OPENROUTER_API_KEY")
    if not key:
        raise SystemExit("set OPENROUTER_API_KEY in the environment first")

    key_file, out_file = sys.argv[1], sys.argv[2]
    limit = int(sys.argv[3]) if len(sys.argv) > 3 else None

    items = [json.loads(l) for l in open(key_file, encoding="utf-8")]
    if limit:
        items = items[:limit]

    CACHE.mkdir(parents=True, exist_ok=True)
    session = requests.Session()
    asked = cached = 0
    cost = 0.0
    tokens_in = tokens_out = 0

    with open(out_file, "w", encoding="utf-8") as fh:
        for n, item in enumerate(items, 1):
            body = body_for(item)
            cp = cache_path(body)
            if cp.exists():
                resp = json.loads(cp.read_text(encoding="utf-8"))
                cached += 1
            else:
                resp = ask(session, body, key)
                cp.write_text(
                    json.dumps(resp, ensure_ascii=False), encoding="utf-8"
                )
                asked += 1
            u = resp.get("usage") or {}
            cost += u.get("cost") or 0.0
            tokens_in += u.get("input_tokens") or 0
            tokens_out += u.get("output_tokens") or 0
            a = (resp.get("answers") or {}).get("direction") or {}
            fh.write(json.dumps({
                "from": item["from"]["id"],
                "to": item["to"]["id"],
                "command": item["command"],
                "expected": item["answer"],
                "choice": a.get("choice"),
                "confidence": a.get("confidence"),
                "probabilities": a.get("probabilities"),
            }, ensure_ascii=False) + "\n")
            if n % 25 == 0:
                print(f"{n}/{len(items)} asked={asked} cached={cached}",
                      flush=True)

    print(f"done: {len(items)} items, {asked} asked, {cached} cached")
    print(f"tokens in={tokens_in} out={tokens_out} cost=${cost:.6f}")


if __name__ == "__main__":
    main()
