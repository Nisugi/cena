"""Parse findcreature.lic's creature blocks: name, level, areas, room-id arrays. Compare with Cena."""
import re, json, collections
from pathlib import Path

LIB = Path(r"E:/Cena/reference/lich_repo_mirror/lib")
DATA = Path(r"E:/Cena/crates/cena-model/data")
OUT = Path(__file__).parent
src = (LIB / "findcreature.lic").read_bytes().decode("utf-8", "replace").splitlines()

blocks = []
cur = None
for i, line in enumerate(src, 1):
    m = re.match(r'^\s*if critter_to_find == "([^"]+)"(.*)$', line)
    if m:
        names = [m.group(1)] + re.findall(r'critter_to_find == "([^"]+)"', m.group(2))
        cur = {"names": names, "line": i, "rooms": {}, "level": None, "areas": None}
        blocks.append(cur)
        continue
    if cur is None:
        continue
    m = re.match(r'^\s*critter_level = (\d+)', line)
    if m and cur["level"] is None:
        cur["level"] = int(m.group(1))
    m = re.match(r'^\s*available_areas = "(.*)"', line)
    if m and cur["areas"] is None:
        cur["areas"] = m.group(1).replace("\\'", "'")
    m = re.match(r'^\s*(\w+_rooms) = \[(.*)\]', line)
    if m:
        cur["rooms"][m.group(1)] = re.findall(r'"?(\d+)"?', m.group(2))

def rows(p):
    lines = (DATA / p).read_bytes().decode("utf-8").splitlines()
    hdr = lines[0].split("\t")
    return [dict(zip(hdr, l.split("\t"))) for l in lines[1:] if l]

cena = rows("creatures.tsv")
norm = lambda s: re.sub(r"\s+", " ", s.strip().lower())
cena_by = {norm(r["name"]): r for r in cena}

names = {}
for b in blocks:
    for n in b["names"]:
        names.setdefault(norm(n), b)
only_fc = sorted(n for n in names if n not in cena_by)
level_conf = []
for n, b in names.items():
    c = cena_by.get(n)
    if c and b["level"] is not None and str(b["level"]) != c["level"]:
        level_conf.append((n, b["line"], b["level"], c["level"]))
total_rooms = sum(len(v) for b in blocks for v in b["rooms"].values())
uniq_rooms = len(set(r for b in blocks for v in b["rooms"].values() for r in v))
res = {
    "blocks": len(blocks), "names": len(names), "in_cena": len(set(names) & set(cena_by)),
    "only_findcreature": [(n, names[n]["line"], names[n]["level"], names[n]["areas"]) for n in only_fc],
    "level_conflicts": sorted(level_conf),
    "room_refs": total_rooms, "unique_rooms": uniq_rooms,
    "cena_not_in_fc": sorted(n for n in cena_by if n not in names),
}
(OUT / "findcreature_cmp.json").write_text(json.dumps(res, indent=1), encoding="utf-8")
print({k: (v if not isinstance(v, list) else len(v)) for k, v in res.items()})
