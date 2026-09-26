import re, pathlib, csv, io, collections
LIB = pathlib.Path(r"E:/Cena/reference/lich_repo_mirror/lib")
src = (LIB / "character-planner.lic").read_bytes().decode("utf-8", "replace").splitlines()
block = src[4369:16547]
crits = {}
t = loc = rk = None
for line in block:
    m = re.match(r'^\t"([^"]+)" => \{', line)
    if m: t = m.group(1); continue
    m = re.match(r'^\t\t"([^"]+)" => \{', line)
    if m: loc = m.group(1); continue
    m = re.match(r'^\t\t\t"Rank (\d+)" => \{', line)
    if m: rk = int(m.group(1)); crits[(t, loc, rk)] = {}; continue
    m = re.match(r'^\t\t\t\t"([^"]+)" => (.*?),?\s*$', line)
    if m and rk is not None:
        crits[(t, loc, rk)][m.group(1)] = m.group(2).strip().rstrip(",").strip('"')
print("planner crit rows:", len(crits))
types = collections.Counter(k[0] for k in crits)
print("types:", dict(types))
locs = collections.Counter(k[1] for k in crits)
print("locations:", dict(locs))
se = collections.Counter()
for v in crits.values():
    for tok in re.split(r"[ ,]+", v.get("Status Effect", "")):
        se[re.sub(r"\d+", "#", tok)] += 1
print("status effect tokens:", se.most_common(40))
wd = collections.Counter(v.get("Wounds", "") for v in crits.values())
print("wounds values (top):", wd.most_common(15))
# compare damage against Cena
rows = list(csv.DictReader(io.StringIO(pathlib.Path(r"E:/Cena/crates/cena-model/data/crit_tables.tsv").read_text(encoding="utf-8")), delimiter="\t"))
cena = collections.defaultdict(set)
cwound = collections.defaultdict(set)
for r in rows:
    cena[(r["type"], r["location"], int(r["rank"]))].add(r["damage"])
    cwound[(r["type"], r["location"], int(r["rank"]))].add(r["wound_rank"])
tmap = {"Slash":"slash","Crush":"crush","Puncture":"puncture","Unbalance":"unbalance","Plasma":"plasma","Disintegration":"disintegrate","Fire":"fire","Impact":"impact","Acid":"acid","Cold":"cold","Lightning":"lightning","Steam":"steam","Grapple UCS":"ucs_grapple","Jab UCS":"ucs_jab","Punch UCS":"ucs_punch","Kick UCS":"ucs_kick","Vacuum":"vacuum","Disruption":"disruption","Grapple":"grapple"}
def lmap(l):
    return {"Head":"head","Neck":"neck","Right Eye":"right_eye","Left Eye":"left_eye","Chest":"chest","Abdomen":"abdomen","Back":"back","Right Arm":"right_arm","Left Arm":"left_arm","Right Hand":"right_hand","Left Hand":"left_hand","Right Leg":"right_leg","Left Leg":"left_leg","Nerves":"nerves"}.get(l, l.lower().replace(" ", "_"))
cena_locs = collections.Counter(k[1] for k in cena)
print("cena locations:", dict(cena_locs))
matched = 0; dmgdiff = []; missing = []; wdiff = []
for (t, l, r), v in crits.items():
    key = (tmap.get(t, t.lower()), lmap(l), r)
    if key not in cena:
        missing.append(key); continue
    matched += 1
    d = v.get("Damage")
    if d is not None and d not in cena[key]:
        dmgdiff.append((key, d, sorted(cena[key])))
    w = v.get("Wounds", "")
    m = re.search(r"R(\d)", w)
    pw = m.group(1) if m else "0"
    cw = {x or "0" for x in cwound[key]}
    if pw not in cw:
        wdiff.append((key, w, sorted(cw)))
print("matched:", matched, "missing in cena:", len(missing))
print("missing sample:", missing[:15])
print("damage diffs:", len(dmgdiff)); [print(" ", x) for x in dmgdiff[:25]]
print("wound diffs:", len(wdiff)); [print(" ", x) for x in wdiff[:15]]
