"""Fill MISSING climate and terrain in a copy of the Lich map, from two sources.

  1. survey    the game's own <roommeta> readings from the author's survey
               sessions, 2026-08 (results/roommeta_from_survey.json)
  2. official  Simutronics room data, map-data/prime/rooms.json, keyed by uid,
               about a year old. MEASURED against the survey where both cover a
               room: terrain agrees 99.3%, climate 100%.
  3. official-blank  terrain only. An official record with no terrain reads
               "no terrain set" in the survey in 617 of 619 rooms, so it is
               written "none". A blank official CLIMATE is never used: the
               survey reads temperate, humid, moist or none in those rooms.

The input file is never touched. Only missing fields (absent, null, blank) are
filled; a value the map already has is left alone even where a source
disagrees, and those are listed in <out>.disagreements.tsv.

Names follow patch_roommeta.py: the map's own spelling where it is a longer
form of the survey's ("deciduous forest"), the survey's otherwise; code 0 is
"none". The official file already uses the map's spellings.

Usage:
    python patch_terrain.py <map.json> <out_map.json>
"""

import collections
import json
import sys

from patch_roommeta import FIELDS, missing, survey_names

R = "results/"
STATE = "E:/Gemstone/data/forge data/forge_survey/state-GST-Nisugi.yml"
OFFICIAL = "E:/Cena/reference/mapdb/map-data/prime/rooms.json"
OFF_KEY = {"terrain": "ter", "climate": "cli"}


def main():
    map_path, out_path = sys.argv[1:3]
    rooms = json.load(open(map_path, encoding="utf-8"))
    meta = json.load(open(R + "roommeta_from_survey.json"))
    official = {r["id"]: r for r in json.load(open(OFFICIAL, encoding="utf-8"))["rooms"]}
    survey = survey_names(STATE)

    names = {}
    for f in FIELDS:
        seen = collections.defaultdict(collections.Counter)
        for r in rooms:
            v, g = r.get(f), meta.get(str(r["id"]))
            if g is not None and not missing(v) and v != "none":
                seen[str(g.get(f))][v] += 1
        names[f] = dict(survey[f])
        for c, n in seen.items():
            top = n.most_common(1)[0][0]
            if c in survey[f] and survey[f][c] in top:
                names[f][c] = top
        names[f]["0"] = "none"

    def from_sources(r, f):
        """(value, source) for one field, or (None, None)."""
        g = meta.get(str(r["id"]))
        if g is not None and names[f].get(str(g.get(f))) is not None:
            return names[f][str(g.get(f))], "survey"
        recs = [official[u] for u in r.get("uid") or [] if u in official]
        vals = {o[OFF_KEY[f]] for o in recs if o.get(OFF_KEY[f]) is not None}
        if len(vals) == 1:
            return vals.pop(), "official"
        if f == "terrain" and recs and not vals:
            return "none", "official-blank"
        return None, None

    filled = collections.Counter()
    log, stale, out = [], [], []
    for r in rooms:
        new = dict(r)
        for f in FIELDS:
            val, src = from_sources(r, f)
            if val is None:
                continue
            if missing(r.get(f)):
                new[f] = val
                filled[(f, src)] += 1
                log.append(f"{r['id']}\t{f}\t{r.get(f)!r}\t{val}\t{src}")
            elif r[f] != val:
                stale.append(f"{r['id']}\t{f}\t{r[f]}\t{val}\t{src}")
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

    with open(out_path, "w", encoding="utf-8", newline="\n") as fh:
        fh.write("[" + ",".join(json.dumps(r, ensure_ascii=False, indent=2) for r in out) + "]")
    base = out_path.rsplit(".", 1)[0]
    with open(base + ".changes.tsv", "w", encoding="utf-8") as fh:
        fh.write("room_id\tfield\twas\tnow\tsource\n" + "\n".join(log) + "\n")
    with open(base + ".disagreements.tsv", "w", encoding="utf-8") as fh:
        fh.write("room_id\tfield\tmap_says\tsource_says\tsource\n" + "\n".join(stale) + "\n")

    print(f"rooms={len(rooms)}")
    for f in FIELDS:
        was = sum(1 for r in rooms if missing(r.get(f)))
        now = sum(1 for r in out if missing(r.get(f)))
        parts = ", ".join(f"{s} {n}" for (ff, s), n in sorted(filled.items()) if ff == f)
        print(f"{f}: missing {was} -> {now}   filled from: {parts}")
    print(f"left alone, the map and a source disagree: {len(stale)}")


if __name__ == "__main__":
    main()
