"""Parse bestiary.lic's creature_database and compare with Cena's creatures.tsv / creature_areas.tsv."""
import re, json, sys, collections
from pathlib import Path

LIB = Path(r"E:/Cena/reference/lich_repo_mirror/lib")
DATA = Path(r"E:/Cena/crates/cena-model/data")
OUT = Path(__file__).parent

src = (LIB / "bestiary.lic").read_bytes().decode("utf-8", "replace").splitlines()

entries = {}
commented = []
cur = None
for i, line in enumerate(src, 1):
    m = re.match(r'^\s*(#?)\s*"([^"]+)"\s*=>\s*\{', line)
    if m:
        cur = {"name": m.group(2), "line": i, "commented": bool(m.group(1))}
        if cur["commented"]:
            commented.append(cur)
        else:
            entries[cur["name"]] = cur
        continue
    if cur is None:
        continue
    m = re.match(r"^\s*#?\s*'level'\s*=>\s*([^,]*),", line)
    if m:
        cur["level"] = m.group(1).strip()
    m = re.match(r"^\s*#?\s*'undead'\s*=>\s*([^,]*),", line)
    if m:
        cur["undead"] = m.group(1).strip()
    m = re.match(r"^\s*#?\s*'corporeal'\s*=>\s*([^,]*),", line)
    if m:
        cur["corporeal"] = m.group(1).strip()
    m = re.match(r"^\s*#?\s*'locations'\s*=>\s*\[(.*)\]", line)
    if m:
        cur["locations"] = re.findall(r'"([^"]*)"', m.group(1))
        cur = None

def rows(p):
    lines = (DATA / p).read_bytes().decode("utf-8").splitlines()
    hdr = lines[0].split("\t")
    return [dict(zip(hdr, l.split("\t"))) for l in lines[1:] if l]

cena = rows("creatures.tsv")
areas = collections.defaultdict(set)
for r in rows("creature_areas.tsv"):
    areas[r["creature_id"]].add(r["area"])

def norm(s):
    return re.sub(r"\s+", " ", s.strip().lower().replace("\u2019", "'"))

cena_by = {norm(r["name"]): r for r in cena}
best_by = {norm(n): e for n, e in entries.items()}

both = sorted(set(cena_by) & set(best_by))
only_best = sorted(set(best_by) - set(cena_by))
only_cena = sorted(set(cena_by) - set(best_by))

level_conf = []
undead_conf = []
for n in both:
    b, c = best_by[n], cena_by[n]
    bl = b.get("level", "")
    if bl and bl != c["level"]:
        level_conf.append((b["name"], b["line"], bl, c["level"], c["id"]))
    bu = b.get("undead", "")
    if bu in ("true", "false") and bu != c["undead"]:
        undead_conf.append((b["name"], b["line"], bu, c["undead"], c["id"]))

area_diff = []
for n in both:
    b, c = best_by[n], cena_by[n]
    bl = set(x for x in b.get("locations", []) if x.strip())
    cl = areas.get(c["id"], set())
    if not cl:
        area_diff.append((b["name"], b["line"], sorted(bl), "NO CENA AREA"))

res = {
    "bestiary_active": len(entries),
    "bestiary_commented": len(commented),
    "cena": len(cena),
    "both": len(both),
    "only_bestiary": [(best_by[n]["name"], best_by[n]["line"], best_by[n].get("level"), best_by[n].get("locations")) for n in only_best],
    "only_cena": [(cena_by[n]["name"], cena_by[n]["level"]) for n in only_cena],
    "level_conflicts": level_conf,
    "undead_conflicts": undead_conf,
    "no_cena_area": area_diff,
    "commented": [(c["name"], c["line"]) for c in commented],
}
(OUT / "bestiary_cmp.json").write_text(json.dumps(res, indent=1), encoding="utf-8")
print("bestiary active", len(entries), "commented", len(commented), "cena", len(cena))
print("both", len(both), "only_bestiary", len(only_best), "only_cena", len(only_cena))
print("level conflicts", len(level_conf), "undead conflicts", len(undead_conf), "both but no cena area", len(area_diff))
