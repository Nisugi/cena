import re, pathlib, csv, io
LIB = pathlib.Path(r"E:/Cena/reference/lich_repo_mirror/lib")
src = (LIB / "character-planner.lic").read_bytes().decode("utf-8", "replace").splitlines()
block = src[1646:4369]
weapons = {}
cat = None; name = None
for line in block:
    m = re.match(r'^\t"([^"]+)" => \{', line)
    if m: cat = m.group(1); continue
    m = re.match(r'^\t\t"([^"]+)" => \{', line)
    if m: name = m.group(1); weapons[name] = {"cat": cat, "df": {}, "avd": {}}; continue
    m = re.search(r'"Slash" => ([\d.]+), "Crush" => ([\d.]+), "Puncture" => ([\d.]+)', line)
    if m and name: weapons[name]["dmg"] = tuple(float(x) for x in m.groups()); continue
    m = re.search(r'"ASG (\d+)"\s*=>\s*\{\s*"DF" => ([\d.]+), "AvD" => (-?\d+)', line)
    if m and name:
        weapons[name]["df"][int(m.group(1))] = float(m.group(2))
        weapons[name]["avd"][int(m.group(1))] = int(m.group(3))
print("planner weapons:", len(weapons))
# Cena
rows = list(csv.DictReader(io.StringIO(pathlib.Path(r"E:/Cena/crates/cena-model/data/weapons.tsv").read_text(encoding="utf-8")), delimiter="\t"))
cena = {r["base_name"].lower(): r for r in rows}
print("cena weapons:", len(cena))
AG = {1: 1, 5: 2, 9: 3, 13: 4, 17: 5}
unmatched = []
diffs = []
for n, w in weapons.items():
    key = n.lower()
    r = cena.get(key)
    if r is None:
        unmatched.append(n); continue
    dfs = r["damage_factor_by_ag_0_to_5"].split("|")
    avds = r["avd_by_asg_1_to_20"].split("|")
    for asg, ag in AG.items():
        pv = w["df"].get(asg)
        cv = dfs[ag] if ag < len(dfs) else ""
        if pv is not None and cv != "" and abs(float(cv) - pv) > 1e-6:
            diffs.append((n, f"DF AG{ag}", pv, cv))
    for asg, pv in w["avd"].items():
        cv = avds[asg - 1] if asg - 1 < len(avds) else ""
        if cv != "" and int(cv) != pv:
            diffs.append((n, f"AvD ASG{asg}", pv, cv))
    d = w.get("dmg")
    if d:
        cd = (float(r["slash"] or 0), float(r["crush"] or 0), float(r["puncture"] or 0))
        if any(abs(a - b) > 0.05 for a, b in zip(d, cd)):
            diffs.append((n, "slash/crush/puncture", d, cd))
print("unmatched by name:", unmatched)
bynm = {}
for d in diffs: bynm.setdefault(d[0], []).append(d[1:])
print("weapons with diffs:", len(bynm), "total diffs:", len(diffs))
for n, ds in bynm.items():
    print(n, ds)
