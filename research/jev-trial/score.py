"""Score Jev's answers against the key and print the report's tables.

Usage:
    python score.py results/job1_answers.jsonl [key.jsonl for mistake text]
"""

import collections
import json
import sys

BANDS = [(0.0, 0.5), (0.5, 0.7), (0.7, 0.8), (0.8, 0.9),
         (0.9, 0.95), (0.95, 0.99), (0.99, 1.01)]


def pct(n, d):
    return f"{n / d:.1%}" if d else "-"


def main():
    rows = [json.loads(l) for l in open(sys.argv[1], encoding="utf-8")]
    n = len(rows)
    right = sum(1 for r in rows if r["choice"] == r["expected"])

    dist = collections.Counter(r["expected"] for r in rows)
    common, common_n = dist.most_common(1)[0]

    print(f"n = {n}")
    print(f"accuracy         {pct(right, n)}  ({right}/{n})")
    print(f"baseline (always '{common}')  {pct(common_n, n)}")
    print()

    print("key distribution")
    for k, v in dist.most_common():
        print(f"  {k:<12} {v:>5}  {pct(v, n)}")
    print()

    print("confusion  expected -> chosen")
    conf = collections.Counter((r["expected"], r["choice"]) for r in rows)
    for (e, c), v in sorted(conf.items(), key=lambda x: -x[1]):
        mark = "" if e == c else "  <-- wrong"
        print(f"  {e:<12} -> {str(c):<12} {v:>5}{mark}")
    print()

    # The threshold table: is a stated probability trustworthy?
    print("accuracy by stated probability of the chosen option")
    print(f"  {'band':<14}{'n':>6}{'correct':>9}{'accuracy':>10}"
          f"{'cumulative >= lo':>19}")
    for lo, hi in BANDS:
        band = [r for r in rows
                if lo <= (r["probabilities"] or {}).get(r["choice"], 0) < hi]
        cum = [r for r in rows
               if (r["probabilities"] or {}).get(r["choice"], 0) >= lo]
        bc = sum(1 for r in band if r["choice"] == r["expected"])
        cc = sum(1 for r in cum if r["choice"] == r["expected"])
        label = f"{lo:.2f}-{hi:.2f}" if hi <= 1 else f"{lo:.2f}-1.00"
        print(f"  {label:<14}{len(band):>6}{bc:>9}{pct(bc, len(band)):>10}"
              f"{pct(cc, len(cum)) + ' (n=' + str(len(cum)) + ')':>19}")
    print()

    print("accuracy by stated confidence")
    print(f"  {'band':<14}{'n':>6}{'correct':>9}{'accuracy':>10}")
    for lo, hi in BANDS:
        band = [r for r in rows if lo <= (r.get("confidence") or 0) < hi]
        bc = sum(1 for r in band if r["choice"] == r["expected"])
        label = f"{lo:.2f}-{hi:.2f}" if hi <= 1 else f"{lo:.2f}-1.00"
        print(f"  {label:<14}{len(band):>6}{bc:>9}{pct(bc, len(band)):>10}")
    print()

    wrong = [r for r in rows if r["choice"] != r["expected"]]
    print(f"mistakes: {len(wrong)}  (first 20 below)")
    key = {}
    if len(sys.argv) > 2:
        for l in open(sys.argv[2], encoding="utf-8"):
            it = json.loads(l)
            key[(it["from"]["id"], it["to"]["id"], it["command"])] = it
    for r in wrong[:20]:
        p = (r["probabilities"] or {}).get(r["choice"], 0)
        print(f"\n  {r['from']} -> {r['to']}  '{r['command']}'")
        print(f"    expected {r['expected']}, chose {r['choice']} "
              f"(p={p:.2f}, conf={r.get('confidence')})")
        it = key.get((r["from"], r["to"], r["command"]))
        if it:
            print(f"    back command: '{it['back_command']}'")
            for side in ("from", "to"):
                t = it[side]["title"]
                print(f"    {side}: {t[0] if t else '?'}")


if __name__ == "__main__":
    main()
