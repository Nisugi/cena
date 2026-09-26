import re, pathlib
LIB = pathlib.Path(r"E:/Cena/reference/lich_repo_mirror/lib")
cp = (LIB / "character-planner.lic").read_bytes().decode("utf-8", "replace").splitlines()[37:438]
planner = {}; prof = None
for l in cp:
    m = re.match(r'^\t"([A-Za-z]+)" => \{', l)
    if m: prof = m.group(1); continue
    m = re.search(r'"([^"]+)"\s*=>\s*\{\s*"PTP" => (\d+),\s*"MTP" => (\d+),\s*"max_ranks" => (\d+)', l)
    if m and prof: planner[(prof, m.group(1))] = (int(m.group(2)), int(m.group(3)), int(m.group(4)))
alias = {"Multi Opponent Combat": "Multi-Opponent Combat", "Polearm Weapons": "Pole Arm Weapons", "Elemental Mana Control": "Mana Control: Elemental", "Mental Mana Control": "Mana Control: Mental", "Spirit Mana Control": "Mana Control: Spiritual", "Stalking and Hiding": "Stalking & Hiding", "Pickpocketing": "Picking Pockets"}
mc = (LIB / "maxcap.lic").read_bytes().decode("utf-8", "replace").splitlines()
maxcap = {}; prof = None
for l in mc:
    m = re.match(r'^all_skills_([a-z]+) = \{', l)
    if m: prof = m.group(1).capitalize(); continue
    m = re.search(r"'([^']+)'\s*=>\s*\{\s*:max_ranks => (\d+), :PTP => (\d+),\s*:MTP => (\d+)", l)
    if m and prof:
        maxcap[(prof, alias.get(m.group(1), m.group(1)))] = (int(m.group(3)), int(m.group(4)), int(m.group(2)))
print("planner", len(planner), "maxcap", len(maxcap))
diffs = []
for k, v in maxcap.items():
    p = planner.get(k)
    if not p: diffs.append((k, "missing in planner", v)); continue
    if (p[0], p[1]) != (v[0], v[1]) or p[2] * 101 != v[2]:
        diffs.append((k, "planner", p, "maxcap", v))
print(len(diffs))
for d in diffs: print(d)
