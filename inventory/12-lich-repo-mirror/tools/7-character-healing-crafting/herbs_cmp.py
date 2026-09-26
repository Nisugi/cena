import re, pathlib, sys
LIB = pathlib.Path(r"E:/Cena/reference/lich_repo_mirror/lib")
cena_lines = pathlib.Path(r"E:/Cena/crates/cena-model/data/herbs.tsv").read_text(encoding="utf-8").splitlines()
cena = {}
for l in cena_lines:
    if l.startswith("#") or l.startswith("name\t"): continue
    f = l.split("\t")
    cena[f[0].lower()] = f
cena_short = {f[1].lower(): f for f in cena.values()}
print("cena herbs.tsv rows:", len(cena))
for script in sys.argv[1:]:
    src = (LIB / script).read_bytes().decode("utf-8", "replace")
    rows = re.findall(r'\{\s*:name\s*=>\s*"([^"]+)"\s*,\s*:type\s*=>\s*"([^"]+)"([^}]*)\}', src)
    print(f"\n== {script}: {len(rows)} herb rows")
    missing = []; typediff = []; dosediff = []
    for name, typ, rest in rows:
        c = cena.get(name.lower()) or cena_short.get(name.lower())
        if not c:
            missing.append((name, typ)); continue
        if c[2] != typ: typediff.append((name, typ, c[2]))
        m = re.search(r':store_doses\s*=>\s*(\d+)', rest)
        if m and c[3] and m.group(1) != c[3]: dosediff.append((name, m.group(1), c[3]))
    print("not in Cena:", len(missing)); [print("  ", x) for x in missing]
    print("type differs:", typediff)
    print("store_doses differs:", dosediff)
