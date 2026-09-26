"""Compare atlas_data.db3 (atlas.lic's creature + hunting-area DB) with Cena's creatures.tsv."""
import sqlite3, re, json
from pathlib import Path

DB = r"file:E:/Cena/reference/lich_repo_mirror/lib/atlas_data.db3?mode=ro"
DATA = Path(r"E:/Cena/crates/cena-model/data")
OUT = Path(__file__).parent
c = sqlite3.connect(DB, uri=True)
print("version", c.execute("select * from version").fetchall())
cre = c.execute("select id, level, undead, noncorp, hated, name, skin from creatures").fetchall()

def rows(p):
    lines = (DATA / p).read_bytes().decode("utf-8").splitlines()
    hdr = lines[0].split("\t")
    return [dict(zip(hdr, l.split("\t"))) for l in lines[1:] if l]
cena = rows("creatures.tsv")
norm = lambda s: re.sub(r"\s+", " ", s.strip().lower())
cb = {norm(r["name"]): r for r in cena}
ab = {norm(r[5]): r for r in cre}
both = set(cb) & set(ab)
only_a = sorted(set(ab) - set(cb))
only_c = sorted(set(cb) - set(ab))
lvl = [(n, ab[n][1], cb[n]["level"]) for n in sorted(both) if str(ab[n][1]) != cb[n]["level"]]
und = [(n, ab[n][2], cb[n]["undead"]) for n in sorted(both) if (ab[n][2] == 1) != (cb[n]["undead"] == "true")]
skin = [(n, ab[n][6], cb[n]["skin"]) for n in sorted(both) if (ab[n][6] or "") and norm(ab[n][6] or "") != norm(cb[n]["skin"] or "") ]
skin_fill = [(n, ab[n][6]) for n in sorted(both) if (ab[n][6] or "") and not cb[n]["skin"]]
print("atlas creatures", len(cre), "cena", len(cena), "both", len(both), "only atlas", len(only_a), "only cena", len(only_c))
print("level conflicts", len(lvl), "undead conflicts", len(und), "skin differs", len(skin), "skin where cena blank", len(skin_fill))
areas = c.execute("select id, region, start from areas").fetchall()
bounds = c.execute("select area, boundary from areaboundaries").fetchall()
ac = c.execute("select area, creature from areacreatures").fetchall()
ids = {r[0]: r[5] for r in cre}
cre_in_area = set(ids.get(x[1]) for x in ac)
print("areas", len(areas), "boundaries", len(bounds), "area-creature links", len(ac), "distinct creatures placed", len(cre_in_area))
print("hated count", sum(1 for r in cre if r[4]))
json.dump({"only_atlas": [(n, ab[n][1], ab[n][6]) for n in only_a], "level": lvl, "undead": und, "skin": skin,
           "skin_fill": skin_fill, "areas": areas[:200]}, open(OUT / "atlas_cmp.json", "w", encoding="utf-8"), indent=1)
