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

ENDPOINT = "https://openrouter.ai/api/alpha/decisions"
MODEL = "typesafe/jev-1.13"
CACHE = Path(__file__).parent / "results" / "cache"

CRITERIA = {
    "up": "Taking this exit moves the character to a higher floor, level or "
          "elevation than the room they started in.",
    "down": "Taking this exit moves the character to a lower floor, level or "
            "elevation than the room they started in.",
    "same floor": "Taking this exit keeps the character at the same elevation. "
                  "It leads sideways, not up or down.",
}


def state_for(item):
    """What Jev sees. Both ends of the exit, so it can judge from either."""

    def side(r):
        return {
            "title": r["title"],
            "description": r["description"],
            "exits_line": r["paths"],
            "area": r["location"],
        }

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
                    "did they go up, go down, or stay on the same floor?"
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
