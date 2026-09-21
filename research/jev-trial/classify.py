"""Give every open exit an estimated accuracy, and band them in 1% steps.

The 553 labelled pairs are too few to measure a 1% band directly, so a small
logistic model is fitted on them and applied to the open rows. Inputs: do the
two directions agree, the lower and higher of the two probabilities, and
whether the author's distance-from-surface rule agrees, disagrees or is silent.
Target: was the pair's decided answer right.

Answers used: the terrain-aware prompt (v9) for rows touching a subterranean
room, the neighbour prompt (v7) for the rest. Each is calibrated on its own
labelled run. The two-option prompt (v8) is NOT used: every labelled exit it
could be calibrated on is truly vertical, so its estimates would be blind to
open exits that are really flat.

Usage:
    python classify.py            # writes results/job1_classified.jsonl + .txt
"""

import collections
import json

import numpy as np
from scipy.optimize import minimize

R = "results/"
OPP = {"up": "down", "down": "up", "same floor": "same floor"}
MAP = "E:/Cena/reference/mapdb/map-1789942730.json"


def load(path):
    rows = (json.loads(l) for l in open(path, encoding="utf-8"))
    return [r for r in rows if "error" not in r]


def decide(r):
    f, v = r["fwd"], r["rev"]
    if OPP.get(f["choice"]) == v["choice"]:
        return f["choice"], True
    if f["p"] >= v["p"]:
        return f["choice"], False
    return OPP.get(v["choice"]), False


# ---- the author's distance-from-surface rule (README 11h onward) ----------
rooms = {str(r["id"]): r for r in json.load(open(MAP, encoding="utf-8"))}
meta = json.load(open(R + "roommeta_from_survey.json"))
levels = json.load(open(R + "name_levels.json", encoding="utf-8"))
ground = set(json.load(open(R + "ground.json")))
sub = {k for k, v in meta.items() if v.get("terrain") == 14}
adj = collections.defaultdict(set)
for a, r in rooms.items():
    for b, c in r["wayto"].items():
        if not str(c).startswith(";e") and str(b) in rooms:
            adj[a].add(str(b))
            adj[str(b)].add(a)


def bfs(src):
    dist = {s: 0 for s in src}
    q = collections.deque(src)
    while q:
        x = q.popleft()
        for y in adj[x]:
            if y not in dist:
                dist[y] = dist[x] + 1
                q.append(y)
    return dist


from_ground = bfs(ground)
from_surface = bfs({k for k in meta if k not in sub and k in rooms})


def name_region(a, b):
    for rid in (b, a):
        for t in rooms[rid].get("title") or []:
            v = levels.get(t) or {}
            if v.get("p", 0) >= 0.9 and v.get("choice") in ("below ground", "above ground"):
                return v["choice"]


def rule(a, b):
    if a not in rooms or b not in rooms:
        return None
    if a in sub or b in sub:
        region, dist = "below ground", from_surface
    else:
        region, dist = name_region(a, b), from_ground
    x, y = dist.get(a), dist.get(b)
    if region is None or x is None or y is None or x == y:
        return None
    deeper = "down" if region == "below ground" else "up"
    return deeper if y > x else OPP[deeper]


def features(r):
    ans, agree = decide(r)
    lo, hi = min(r["fwd"]["p"], r["rev"]["p"]), max(r["fwd"]["p"], r["rev"]["p"])
    ru = rule(r["a"], r["b"])
    return [1.0, float(agree), lo, hi, agree * lo,
            float(ru is not None and ru == ans),
            float(ru is not None and ru != ans)]


def fit(X, y, l2=1.0):
    def loss(w):
        z = X @ w
        return np.sum(np.logaddexp(0, z) - y * z) + l2 * np.sum(w[1:] ** 2)
    return minimize(loss, np.zeros(X.shape[1]), method="L-BFGS-B").x


def predict(X, w):
    return 1 / (1 + np.exp(-(X @ w)))


def calibrate(path, label):
    rows = load(path)
    X = np.array([features(r) for r in rows])
    y = np.array([float(decide(r)[0] == r["expected"]) for r in rows])
    # 5-fold check: are held-out estimates honest?
    idx = np.random.RandomState(7).permutation(len(rows))
    held = np.zeros(len(rows))
    for k in range(5):
        test = idx[k::5]
        train = np.setdiff1d(idx, test)
        held[test] = predict(X[test], fit(X[train], y[train]))
    print(f"{label}: {len(rows)} labelled pairs, {y.mean():.1%} right overall")
    print("   held-out check   estimate band      pairs   estimated   actually right")
    for lo, hi in ((0.97, 1.01), (0.93, 0.97), (0.88, 0.93), (0.80, 0.88), (0.0, 0.80)):
        m = (held >= lo) & (held < hi)
        if m.sum():
            print(f"                    {lo:.2f}-{min(hi, 1):.2f}      {m.sum():>7}"
                  f"{held[m].mean():>11.1%}{y[m].mean():>16.1%}")
    return fit(X, y)


def main():
    w7 = calibrate(R + "symk_answers.jsonl", "v7 neighbours")
    w9 = calibrate(R + "symk9_answers.jsonl", "v9 + game terrain")

    v9 = {(r["a"], r["b"]): r for r in load(R + "sub_answers_v9.jsonl")}
    out = []
    for kind, path in (("pair", "sym_answers_v7.jsonl"), ("single", "single_answers.jsonl")):
        for r in load(R + path):
            use9 = (r["a"], r["b"]) in v9
            src = v9[(r["a"], r["b"])] if use9 else r
            ans, agree = decide(src)
            est = float(predict(np.array([features(src)]), w9 if use9 else w7)[0])
            ru = rule(r["a"], r["b"])
            out.append({
                "from": r["a"], "to": r["b"], "command": r["a_to_b"],
                "direction": ans, "estimated_accuracy": round(est, 4),
                "band": f"{int(est * 100)}%",
                "directions_agree": agree,
                "p_forward": src["fwd"]["p"], "p_reverse": src["rev"]["p"],
                "distance_rule": ru, "kind": kind,
                "prompt": "v9" if use9 else "v7", "source": "llm",
                "model": "typesafe/jev-1.13-20260917",
            })
    out.sort(key=lambda o: -o["estimated_accuracy"])
    with open(R + "job1_classified.jsonl", "w", encoding="utf-8") as fh:
        for o in out:
            fh.write(json.dumps(o, ensure_ascii=False) + "\n")

    n = len(out)
    bands = collections.Counter(int(o["estimated_accuracy"] * 100) for o in out)
    lines = [f"{'band':>6}{'rows':>7}{'running total':>15}{'share':>8}"
             f"{'expected accuracy of everything kept so far':>46}"]
    run, acc = 0, 0.0
    for b in range(99, -1, -1):
        if not bands[b]:
            continue
        rows_b = [o["estimated_accuracy"] for o in out if int(o["estimated_accuracy"] * 100) == b]
        run += len(rows_b)
        acc += sum(rows_b)
        lines.append(f"{b:>5}%{len(rows_b):>7}{run:>15}{run / n:>8.1%}{acc / run:>46.1%}")
    open(R + "job1_classified.txt", "w", encoding="utf-8").write("\n".join(lines) + "\n")
    print()
    print("\n".join(lines))


if __name__ == "__main__":
    main()
