import re
from pathlib import Path
src = Path(r"E:/Cena/reference/lich_repo_mirror/lib/huntplan.lic").read_bytes().decode("utf-8","replace").splitlines()
rows=[]
for i,l in enumerate(src,1):
    m=re.search(r'Creature\.new\(\[(.*?)\],\s*(nil|\d+),\s*\[(.*?)\]\)',l)
    if m:
        names=re.findall(r'"([^"]+)"|\'([^\']+)\'',m.group(1))
        names=[a or b for a,b in names]
        lvl=None if m.group(2)=="nil" else int(m.group(2))
        rows.append((i,names,lvl,[x for x in m.group(3).split(",") if x.strip()]))
cena={}
for l in Path(r"E:/Cena/crates/cena-model/data/creatures.tsv").read_bytes().decode("utf-8").splitlines()[1:]:
    f=l.split("\t")
    cena[f[1].lower()]=(f[0],f[3])
print("huntplan rows",len(rows),"names",sum(len(r[1]) for r in rows),"nil level",sum(1 for r in rows if r[2] is None))
match=miss=conf=0; conflicts=[]; missing=[]
for i,names,lvl,sp in rows:
    for n in names:
        c=cena.get(n.lower())
        if not c: miss+=1; missing.append((i,n,lvl)); continue
        if lvl is None: continue
        if str(lvl)!=c[1]: conf+=1; conflicts.append((i,n,lvl,c[1]))
        else: match+=1
print("level match",match,"conflict",conf,"name not in cena",miss)
print("conflicts sample",conflicts[:25])
print("missing sample",missing[:40])
