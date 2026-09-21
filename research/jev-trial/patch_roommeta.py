"""Fill MISSING climate and terrain in a copy of the Lich map, from the game's
own <roommeta> readings saved by the author's survey sessions.

It never touches the input file. It writes a patched copy and a change log.

Only fields that are missing (absent, null, or a blank string) are filled. A
room where the map already says something -- including the string "none" --
is left alone, even where the game disagrees; those are listed in a separate
report for the author to decide on.

Names: the game sends numeric codes. Each code is written under the name the
map already uses most often for it (code 6 is "deciduous forest" in the map,
"deciduous" in the survey's table), so new values match existing ones. Codes
the map has never recorded take the survey's name. Code 0 is written "none",
the map's existing spelling for "the game sets none".

Usage:
    python patch_roommeta.py <map.json> <roommeta.json> <survey_state.yml> <out_map.json>
"""

import collections
import json
import re
import sys

FIELDS = ("climate", "terrain")


def missing(v):
    return v is None or v == ""


def survey_names(state_path):
    codes = open(state_path, encoding="utf-8").read().split("\ncodes:")[1]
    climate, terrain = codes.split("terrain:")
    pairs = lambda s: {a: b.strip() for a, b in re.findall(r"'(\d+)': (.+)", s)}
    return {"climate": pairs(climate), "terrain": pairs(terrain)}


def main():
    map_path, meta_path, state_path, out_path = sys.argv[1:5]
    rooms = json.load(open(map_path, encoding="utf-8"))
    meta = json.load(open(meta_path, encoding="utf-8"))
    survey = survey_names(state_path)

    # code -> the name the map itself uses most for that code
    names = {}
    for f in FIELDS:
        seen = collections.defaultdict(collections.Counter)
        for r in rooms:
            v, g = r.get(f), meta.get(str(r["id"]))
            if g is not None and not missing(v) and v != "none":
                seen[str(g.get(f))][v] += 1
        # Take the map's spelling only when it is a longer form of the survey's
        # own name ("deciduous forest" for "deciduous"). A bare majority is not
        # safe: the one map room with code 16 says "plain dirt", which is a
        # stale value, not another name for "icy glacier".
        names[f] = dict(survey[f])
        for c, n in seen.items():
            top = n.most_common(1)[0][0]
            if c in survey[f] and survey[f][c] in top:
                names[f][c] = top
        names[f]["0"] = "none"

    filled = collections.Counter()
    stale = []
    log = []
    out = []
    for r in rooms:
        g = meta.get(str(r["id"]))
        new = dict(r)
        if g is not None:
            for f in FIELDS:
                game = names[f].get(str(g.get(f)))
                if game is None:
                    continue
                if missing(r.get(f)):
                    new[f] = game
                    filled[f] += 1
                    log.append(f"{r['id']}\t{f}\t{r.get(f)!r}\t{game}")
                elif r[f] != game:
                    stale.append(f"{r['id']}\t{f}\t{r[f]}\t{game}")
        # keep the map's key order: climate and terrain sit after location
        if any(f in new and f not in r for f in FIELDS):
            ordered = {}
            for k, v in r.items():
                ordered[k] = v
                if k == "location":
                    for f in FIELDS:
                        if f in new:
                            ordered[f] = new[f]
            for k, v in new.items():
                ordered.setdefault(k, v)
            new = ordered
        out.append(new)

    # Match the input's layout exactly -- "[{", rooms flush left joined by
    # "},{", no trailing newline -- so a diff shows only the filled fields.
    with open(out_path, "w", encoding="utf-8", newline="\n") as fh:
        body = ",".join(json.dumps(r, ensure_ascii=False, indent=2) for r in out)
        fh.write("[" + body + "]")
    base = out_path.rsplit(".", 1)[0]
    with open(base + ".changes.tsv", "w", encoding="utf-8") as fh:
        fh.write("room_id\tfield\twas\tnow\n" + "\n".join(log) + "\n")
    with open(base + ".disagreements.tsv", "w", encoding="utf-8") as fh:
        fh.write("room_id\tfield\tmap_says\tgame_says\n" + "\n".join(stale) + "\n")

    print(f"rooms={len(rooms)} with a game reading={sum(1 for r in rooms if str(r['id']) in meta)}")
    print(f"filled: climate={filled['climate']} terrain={filled['terrain']}")
    print(f"left alone, map and game disagree: {len(stale)}")
    for f in FIELDS:
        print(f"{f} names used:", dict(sorted(names[f].items(), key=lambda x: int(x[0]))))


if __name__ == "__main__":
    main()
