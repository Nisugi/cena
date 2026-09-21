"""Symmetry test: score Jev on exits that have NO answer key.

Both keys in this trial are built from an explicit return leg, so they cover
only exits a rule already answers. This tests the rest. For a pair of rooms
joined by vertical-looking exits in both directions, A->B and B->A must be
opposite: (up, down), (down, up) or (same floor, same floor). Any other
combination means at least one of the two answers is wrong.

Uses the v7 prompt: v6 plus each room's other exits.

Usage:
    python sym.py build <map.json> results/sym_pairs.jsonl
    python sym.py ask   results/sym_pairs.jsonl results/sym_answers.jsonl [limit]
    python sym.py score results/sym_answers.jsonl
"""

import json
import os
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor

import requests

from ask_v7 import CACHE, ask, body_for, cache_path
from build_keys import COMPASS, VERTICAL, is_script, room_text
from build_keys2 import FLOOR_CHANGING

OPPOSITE = {"up": "down", "down": "up", "same floor": "same floor"}


def looks_vertical(cmd):
    c = str(cmd).strip().lower()
    if is_script(c) or c in COMPASS or c in VERTICAL:
        return False
    return bool(FLOOR_CHANGING.search(c))


def build(map_path, out_path):
    with open(map_path, encoding="utf-8") as fh:
        rooms = {str(r["id"]): r for r in json.load(fh)}
    n = 0
    with open(out_path, "w", encoding="utf-8") as fh:
        for a, room in rooms.items():
            for b, cmd in (room.get("wayto") or {}).items():
                b = str(b)
                if b not in rooms or int(a) >= int(b):
                    continue  # each unordered pair once
                back = (rooms[b].get("wayto") or {}).get(a)
                if back is None:
                    continue
                if not (looks_vertical(cmd) and looks_vertical(back)):
                    continue
                fh.write(json.dumps({
                    "a": room_text(room), "b": room_text(rooms[b]),
                    "a_to_b": cmd, "b_to_a": back,
                }, ensure_ascii=False) + "\n")
                n += 1
    print(f"pairs={n} calls={2 * n}")


def one(session, key, frm, to, cmd, stats, lock):
    body = body_for({"from": frm, "to": to, "command": cmd})
    cp = cache_path(body)
    if cp.exists():
        resp = json.loads(cp.read_text(encoding="utf-8"))
    else:
        resp = ask(session, body, key)
        cp.write_text(json.dumps(resp, ensure_ascii=False), encoding="utf-8")
        u = resp.get("usage") or {}
        with lock:
            stats["asked"] += 1
            stats["cost"] += u.get("cost") or 0.0
            stats["in"] += u.get("input_tokens") or 0
            stats["out"] += u.get("output_tokens") or 0
    a = (resp.get("answers") or {}).get("direction") or {}
    return {"choice": a.get("choice"),
            "p": (a.get("probabilities") or {}).get(a.get("choice"), 0)}


def ask_all(pairs_path, out_path, limit):
    key = os.environ.get("OPENROUTER_API_KEY")
    if not key:
        raise SystemExit("set OPENROUTER_API_KEY in the environment first")
    pairs = [json.loads(l) for l in open(pairs_path, encoding="utf-8")]
    if limit:
        pairs = pairs[:limit]
    CACHE.mkdir(parents=True, exist_ok=True)
    stats = {"asked": 0, "cost": 0.0, "in": 0, "out": 0}
    lock = threading.Lock()
    local = threading.local()

    def work(p):
        if not hasattr(local, "s"):
            local.s = requests.Session()
        # ask() does not retry 529 (provider overloaded); do it here.
        for attempt in range(6):
            try:
                f = one(local.s, key, p["a"], p["b"], p["a_to_b"], stats, lock)
                r = one(local.s, key, p["b"], p["a"], p["b_to_a"], stats, lock)
                break
            except SystemExit as e:
                if "529" not in str(e) or attempt == 5:
                    return {"a": p["a"]["id"], "b": p["b"]["id"],
                            "error": str(e)[:200]}
                time.sleep(3 * (attempt + 1))
        return {"a": p["a"]["id"], "b": p["b"]["id"],
                "a_to_b": p["a_to_b"], "b_to_a": p["b_to_a"],
                "fwd": f, "rev": r}

    done = 0
    with open(out_path, "w", encoding="utf-8") as fh, \
            ThreadPoolExecutor(max_workers=4) as pool:
        for row in pool.map(work, pairs):
            fh.write(json.dumps(row, ensure_ascii=False) + "\n")
            done += 1
            if done % 100 == 0:
                print(f"{done}/{len(pairs)} asked={stats['asked']} "
                      f"cost=${stats['cost']:.4f}", flush=True)
    print(f"done: {len(pairs)} pairs, {stats['asked']} calls asked, "
          f"tokens in={stats['in']} out={stats['out']} "
          f"cost=${stats['cost']:.6f}")


def score(path):
    rows = [json.loads(l) for l in open(path, encoding="utf-8")]
    errs = [r for r in rows if "error" in r]
    rows = [r for r in rows if "error" not in r]
    print(f"pairs={len(rows)} errors={len(errs)}")

    def consistent(r):
        return OPPOSITE.get(r["fwd"]["choice"]) == r["rev"]["choice"]

    ok = sum(1 for r in rows if consistent(r))
    print(f"consistent {ok}/{len(rows)} = {ok / len(rows):.1%}")
    print()
    print("by the LOWER of the two stated probabilities")
    print(f"  {'gate':<8}{'pairs':>7}{'consistent':>12}{'rate':>8}{'coverage':>10}")
    for g in (0.0, 0.5, 0.7, 0.8, 0.9, 0.95, 0.99):
        s = [r for r in rows if min(r["fwd"]["p"], r["rev"]["p"]) >= g]
        c = sum(1 for r in s if consistent(r))
        rate = f"{c / len(s):.1%}" if s else "-"
        print(f"  >={g:<6}{len(s):>7}{c:>12}{rate:>8}{len(s) / len(rows):>10.1%}")
    print()
    import collections
    combos = collections.Counter(
        (r["fwd"]["choice"], r["rev"]["choice"]) for r in rows)
    print("answer pairs (forward, reverse)")
    for (f, v), n in combos.most_common():
        mark = "" if OPPOSITE.get(f) == v else "  <-- inconsistent"
        print(f"  {str(f):<12}{str(v):<12}{n:>6}{mark}")
    print()
    bad = [r for r in rows if not consistent(r)
           and min(r["fwd"]["p"], r["rev"]["p"]) >= 0.99]
    print(f"inconsistent with BOTH answers at p>=0.99: {len(bad)}")
    for r in bad[:20]:
        print(f"  {r['a']}<->{r['b']}  '{r['a_to_b']}'={r['fwd']['choice']}  "
              f"'{r['b_to_a']}'={r['rev']['choice']}")


if __name__ == "__main__":
    mode = sys.argv[1]
    if mode == "build":
        build(sys.argv[2], sys.argv[3])
    elif mode == "ask":
        ask_all(sys.argv[2], sys.argv[3],
                int(sys.argv[4]) if len(sys.argv) > 4 else None)
    else:
        score(sys.argv[2])
