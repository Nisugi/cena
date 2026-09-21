"""Validate the pair-accept rules on exits whose answer IS known.

The open pairs of sym.py have no key, so a rule such as "accept any pair whose
two answers are opposite" can be measured there for coverage but not accuracy.
This makes labelled pairs out of the 553 keyed exits. The forward question is
asked as usual. The reverse is asked from the far room with the forward
command reused -- 1,039 of the 1,241 open pairs have the same command both
ways, so that is the realistic shape -- and with the literal up/down/compass
return leg never shown. v7 prompt.

Usage:
    python sym_keyed.py ask   results/job1_combined_key.jsonl results/symk_answers.jsonl
    python sym_keyed.py score results/symk_answers.jsonl
"""

import json
import os
import sys
import threading
import time
from concurrent.futures import ThreadPoolExecutor

import requests

from sym9 import OPPOSITE, one


def ask_all(key_path, out_path):
    key = os.environ.get("OPENROUTER_API_KEY")
    if not key:
        raise SystemExit("set OPENROUTER_API_KEY in the environment first")
    items = [json.loads(l) for l in open(key_path, encoding="utf-8")]
    stats = {"asked": 0, "cost": 0.0, "in": 0, "out": 0}
    lock = threading.Lock()
    local = threading.local()

    def work(it):
        if not hasattr(local, "s"):
            local.s = requests.Session()
        for attempt in range(6):
            try:
                f = one(local.s, key, it["from"], it["to"], it["command"], stats, lock)
                r = one(local.s, key, it["to"], it["from"], it["command"], stats, lock)
                break
            except SystemExit as e:
                if attempt == 5:
                    return {"error": str(e)[:200]}
                time.sleep(3 * (attempt + 1))
        return {"a": it["from"]["id"], "b": it["to"]["id"],
                "command": it["command"], "expected": it["answer"],
                "fwd": f, "rev": r}

    with open(out_path, "w", encoding="utf-8") as fh, \
            ThreadPoolExecutor(max_workers=4) as pool:
        for n, row in enumerate(pool.map(work, items), 1):
            fh.write(json.dumps(row, ensure_ascii=False) + "\n")
            if n % 100 == 0:
                print(f"{n}/{len(items)} asked={stats['asked']}", flush=True)
    print(f"done: {len(items)} pairs, {stats['asked']} calls asked, tokens "
          f"in={stats['in']} out={stats['out']} cost=${stats['cost']:.6f}")


def score(path):
    rows = [json.loads(l) for l in open(path, encoding="utf-8")]
    errs = sum(1 for r in rows if "error" in r)
    rows = [r for r in rows if "error" not in r]
    n = len(rows)
    print(f"pairs={n} errors={errs}")

    def cons(r):
        return OPPOSITE.get(r["fwd"]["choice"]) == r["rev"]["choice"]

    def right(r):
        return r["fwd"]["choice"] == r["expected"]

    lo = lambda r: min(r["fwd"]["p"], r["rev"]["p"])
    hi = lambda r: max(r["fwd"]["p"], r["rev"]["p"])
    rules = [
        ("forward answer alone, any confidence", lambda r: True),
        ("forward answer alone, p>=0.99", lambda r: r["fwd"]["p"] >= 0.99),
        ("consistent pair", cons),
        ("consistent, both p>=0.7", lambda r: cons(r) and lo(r) >= 0.7),
        ("consistent, at least one p>=0.99", lambda r: cons(r) and hi(r) >= 0.99),
        ("consistent, both p>=0.99", lambda r: cons(r) and lo(r) >= 0.99),
    ]
    print(f"  {'accept rule':<40}{'accepted':>9}{'coverage':>10}"
          f"{'correct':>9}{'accuracy':>10}")
    for name, rule in rules:
        s = [r for r in rows if rule(r)]
        c = sum(1 for r in s if right(r))
        acc = f"{c / len(s):.1%}" if s else "-"
        print(f"  {name:<40}{len(s):>9}{len(s) / n:>10.1%}{c:>9}{acc:>10}")
    bad = [r for r in rows if cons(r) and not right(r)]
    print(f"\nconsistent but WRONG: {len(bad)} (the failure the open test cannot see)")
    for r in bad[:15]:
        print(f"  {r['a']}->{r['b']} '{r['command']}' expected {r['expected']}, "
              f"said {r['fwd']['choice']} p={r['fwd']['p']:.2f}/{r['rev']['p']:.2f}")


if __name__ == "__main__":
    if sys.argv[1] == "ask":
        ask_all(sys.argv[2], sys.argv[3])
    else:
        score(sys.argv[2])
