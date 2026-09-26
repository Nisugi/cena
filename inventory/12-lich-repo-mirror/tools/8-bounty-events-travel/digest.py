import re, pathlib, sys
lib = pathlib.Path(r"E:/Cena/reference/lich_repo_mirror/lib")
rows = [r.split("\t") for r in pathlib.Path(r"../pile-8-bounty-events-travel.tsv").read_text(encoding="utf-8").splitlines()[1:]]
start, end = int(sys.argv[1]), int(sys.argv[2])
pat = re.compile(r"=~\s*/|waitforre|matchtimeout|matchwait|dothistimeout|when\s+/|Regexp\.new|%r\{|waitfor\s|match\s+\"|matchfind")
lit = re.compile(r"/((?:[^/\\\n]|\\.){8,})/|\"([^\"\n]{10,})\"")
skip = re.compile(r"script\.vars|UserVars|CharSettings|Settings\[|help|^\^?\w+\$?$|respond|echo", re.I)
meta = re.compile(r"author|version|date|tags|game:", re.I)
for i, (name, lines, cap, data, api) in enumerate(rows):
    if i < start or i >= end:
        continue
    txt = (lib / name).read_bytes().decode("utf-8", "replace").splitlines()
    head = [l.strip() for l in txt[:50] if l.strip() and not l.strip().startswith(("=begin", "=end"))]
    desc = next((h for h in head if len(h) > 25 and not meta.search(h) and not h.startswith(("require", "def ", "if ", "unless ", "$", "@"))), "")
    metas = [h for h in head if meta.search(h)][:3]
    caps = []
    for j, l in enumerate(txt):
        if l.strip().startswith("#") or not pat.search(l):
            continue
        for m in lit.finditer(l):
            s = m.group(1) or m.group(2)
            if s and not skip.search(s) and s not in [c[1] for c in caps]:
                caps.append((j + 1, s[:150]))
    print(f"### [{i}] {name} lines={lines} cap={cap} data={data} api={api}")
    print(f"  desc: {desc[:170]}")
    if metas:
        print("  meta: " + " | ".join(m[:70] for m in metas))
    for j, s in caps[:9]:
        print(f"  {j}: {s}")
