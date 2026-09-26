import re
from pathlib import Path

src = Path(r"E:/Cena/reference/lich_repo_mirror/lib/wands.lic").read_bytes().decode("utf-8", "replace")
lines = src.splitlines()
rows = []
cur = None
for i, line in enumerate(lines, 1):
    m = re.search(r":(\w+)\s*=>\s*(.+?),?\s*$", line)
    if line.strip() == "{":
        cur = {"line": i}
        continue
    if line.strip().startswith("}") and cur is not None:
        rows.append(cur)
        cur = None
        continue
    if cur is not None and m:
        k, v = m.group(1), m.group(2).strip().rstrip(",")
        cur[k] = v.strip("'\"")

spells = {}
for line in Path(r"E:/Cena/crates/cena-model/data/spells.tsv").read_bytes().decode("utf-8").splitlines():
    if line.startswith("#") or line.startswith("number"):
        continue
    f = line.split("\t")
    if f and f[0].isdigit():
        spells[int(f[0])] = f[1]

gameobj = Path(r"E:/Cena/crates/cena-model/data/gameobj-data.tsv").read_bytes().decode("utf-8").splitlines()
pats = []
for line in gameobj:
    f = line.split("\t")
    if len(f) == 4 and f[0] in ("type", "sellable"):
        try:
            pats.append((f[0], f[1], f[2], re.compile(f[3])))
        except re.error:
            pass


def classify(name):
    noun = name.split()[-1]
    out = []
    for kind, cat, field, rx in pats:
        if field == "exclude":
            continue
        target = noun if field == "noun" else name
        if rx.search(target):
            excl = [r for k2, c2, f2, r in pats if k2 == kind and c2 == cat and f2 == "exclude"]
            if any(r.search(name) for r in excl):
                continue
            out.append(f"{kind}:{cat}")
    return sorted(set(out))


print(f"rows={len(rows)}")
bad = 0
for r in rows:
    n = int(r.get("spell_num", "0") or 0)
    ours = spells.get(n)
    flag = ""
    if ours is None:
        flag = "MISSING_IN_SPELLS_TSV"
    elif ours.lower() != r.get("spell", "").lower():
        flag = f"NAME_DIFFERS(cena={ours})"
    cls = classify(r.get("name", ""))
    if flag:
        bad += 1
    print(f"{r['line']}\t{r.get('name')}\t{n}\t{r.get('spell')}\t{r.get('crumbly')}\t{r.get('treasure')}\t{r.get('alchemy')}\t{r.get('circle')}\t{flag}\t{','.join(cls)}")
print(f"spell mismatches={bad}")
