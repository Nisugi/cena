import re, pathlib, csv, io
LIB = pathlib.Path(r"E:/Cena/reference/lich_repo_mirror/lib")
src = (LIB / "health-tracker.lic").read_bytes().decode("utf-8", "replace").splitlines()
ht = {}
for line in src:
    m = re.match(r"\s*'([^']+)'\s*=>\s*\{\s*:minhealth => (\d+), :maxhealth => (\d+)", line)
    if m: ht[m.group(1).lower()] = (int(m.group(2)), int(m.group(3)))
known = {k: v for k, v in ht.items() if v[1] > 0}
print("health-tracker rows:", len(ht), "with a value:", len(known))
rows = list(csv.DictReader(io.StringIO(pathlib.Path(r"E:/Cena/crates/cena-model/data/creatures.tsv").read_text(encoding="utf-8")), delimiter="\t"))
cena = {r["name"].lower(): r for r in rows}
print("cena creatures:", len(cena), "with max_hp:", sum(1 for r in rows if r["max_hp"]))
both = [k for k in known if k in cena]
print("in both with a value:", len(both))
agree = diff = cena_blank = 0
ex = []
for k in both:
    c = cena[k]["max_hp"]
    if not c:
        cena_blank += 1; ex.append((k, known[k], "cena blank")); continue
    c = int(c)
    lo, hi = known[k]
    if lo <= c <= hi: agree += 1
    else:
        diff += 1; ex.append((k, known[k], c))
print("agree:", agree, "differ:", diff, "cena blank but tracker has:", cena_blank)
for e in ex[:40]: print(" ", e)
only_tracker = [k for k in known if k not in cena]
print("tracker-only names with a value:", len(only_tracker), only_tracker[:30])
