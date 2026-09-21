"""A blind test of a stronger reader (a Claude subagent) on the pairs Jev is
least sure of.

build   writes results/agent_test/batch_N.txt -- readable text, one block per
        exit, with NOTHING that reveals the answer: no answer, no room ids, no
        way-back command, no exits line (README section 9), and each room's
        other exits listed WITHOUT the exit being judged. The key stays in
        results/agent_test/key.json, which the agents are told not to open.
score   reads results/agent_test/answers_N.jsonl and compares with Jev on the
        same pairs.

Usage:
    python agent_test.py build [labelled|open] [batch_size]
    python agent_test.py score
"""

import glob
import json
import os
import random
import sys
import textwrap

import numpy as np

import classify as C
from ask_v9 import _META, _TERRAIN, other_exits

OUT = "results/agent_test/"


def setting(room):
    words = {p.lower()[:13] for p in room.get("paths") or []}
    return {"obvious exits": "indoors", "obvious paths": "outdoors"}.get(
        next(iter(words)) if len(words) == 1 else "", None)


def room_block(label, rid, partner):
    r = C.rooms[rid]
    lines = [f"  {label}"]
    lines.append("    name: " + " / ".join(r.get("title") or ["(none)"]))
    if r.get("location"):
        lines.append(f"    area: {r['location']}")
    s = setting(r)
    if s:
        lines.append(f"    setting: {s}")
    g = _META.get(rid)
    if g is not None and g.get("terrain") in _TERRAIN:
        lines.append(f"    game terrain: {_TERRAIN[g['terrain']]}")
    for n, desc in enumerate(dict.fromkeys(r.get("description") or []), 1):
        lines += textwrap.wrap(desc, 150, initial_indent=f"    description {n}: ",
                               subsequent_indent="      ")
    others = other_exits(rid, partner)
    if others:
        lines.append("    other ways out of this room (the exit being judged is left out):")
        for o in others:
            lines.append(f"      {o['command']}  ->  {o['leads_to']}")
    return lines


def build(which, size):
    global OUT
    if which == "open":
        OUT = "results/agent_open/"      # keep the labelled test files intact
    os.makedirs(OUT, exist_ok=True)
    for old in glob.glob(OUT + "batch_*.txt"):
        os.remove(old)
    if which == "labelled":
        rows = C.load(C.R + "symk_answers.jsonl")
        X = np.array([C.features(r) for r in rows])
        y = np.array([float(C.decide(r)[0] == r["expected"]) for r in rows])
        est = C.predict(X, C.fit(X, y))
        picked = [(r["a"], r["b"], r["command"], r["expected"], C.decide(r)[0], float(e))
                  for r, e in zip(rows, est) if e < 0.93]
    else:
        picked = [(r["from"], r["to"], r["command"], None, r["direction"],
                   r["estimated_accuracy"])
                  for r in (json.loads(l) for l in open(C.R + "job1_to_settle.jsonl",
                                                        encoding="utf-8"))]
    random.Random(11).shuffle(picked)
    key = {}
    for n in range(0, len(picked), size):
        lines = []
        for i, (a, b, cmd, expected, jev, est) in enumerate(picked[n:n + size], n + 1):
            item = f"E{i:04d}"
            key[item] = {"from": a, "to": b, "command": cmd, "expected": expected,
                         "jev": jev, "jev_estimate": est}
            lines.append(f"=== {item} ===")
            lines.append(f"  command typed: {cmd}")
            lines += room_block("ROOM LEFT", a, b)
            lines += room_block("ROOM ARRIVED IN", b, a)
            lines.append("")
        with open(f"{OUT}batch_{n // size + 1}.txt", "w", encoding="utf-8") as fh:
            fh.write("\n".join(lines))
    json.dump(key, open(OUT + "key.json", "w"), indent=0)
    print(f"{which}: {len(picked)} exits in {len(glob.glob(OUT + 'batch_*.txt'))} batches of up to {size}")


def score():
    key = json.load(open(OUT + "key.json"))
    ans = {}
    for f in glob.glob(OUT + "answers_*.jsonl"):
        for l in open(f, encoding="utf-8"):
            l = l.strip()
            if l:
                a = json.loads(l)
                ans[a["item"]] = a
    both = [k for k in key if k in ans and key[k]["expected"]]
    print(f"exits in the test: {len(key)} | answered: {len(ans)} | scored: {len(both)}")
    if not both:
        return
    jev = sum(key[k]["jev"] == key[k]["expected"] for k in both)
    agent = sum(ans[k]["direction"] == key[k]["expected"] for k in both)
    print(f"Jev right:   {jev} ({jev / len(both):.1%})")
    print(f"agent right: {agent} ({agent / len(both):.1%})")
    for kind in ("up", "down", "same floor"):
        g = [k for k in both if key[k]["expected"] == kind]
        if g:
            print(f"   truly {kind:10} n={len(g):3}  Jev {sum(key[k]['jev'] == kind for k in g):3}"
                  f"  agent {sum(ans[k]['direction'] == kind for k in g):3}")
    print("agent accuracy by its own stated confidence:")
    for lo, hi in ((90, 101), (75, 90), (60, 75), (0, 60)):
        g = [k for k in both if lo <= ans[k].get("confidence", 0) < hi]
        if g:
            ok = sum(ans[k]["direction"] == key[k]["expected"] for k in g)
            print(f"   confidence {lo:3}-{min(hi, 100):3}: {len(g):3} exits, {ok / len(g):.1%} right")
    agree = [k for k in both if ans[k]["direction"] == key[k]["jev"]]
    ok = sum(ans[k]["direction"] == key[k]["expected"] for k in agree)
    print(f"agent and Jev agree on {len(agree)}; right when they agree: {ok / max(len(agree), 1):.1%}")
    dis = [k for k in both if k not in agree]
    print(f"they disagree on {len(dis)}: agent right {sum(ans[k]['direction'] == key[k]['expected'] for k in dis)},"
          f" Jev right {sum(key[k]['jev'] == key[k]['expected'] for k in dis)}")


if __name__ == "__main__":
    if sys.argv[1] == "build":
        build(sys.argv[2] if len(sys.argv) > 2 else "labelled",
              int(sys.argv[3]) if len(sys.argv) > 3 else 40)
    else:
        score()
