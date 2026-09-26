import re,collections
src=open('E:/Cena/reference/lich_repo_mirror/lib/crit-tracking.lic',encoding='utf-8',errors='replace').read().splitlines()
ARM=["No Armor","Light Leather","Full Leather","Reinforced Leather","Double Leather","Leather Breastplate","Cuirbouili Leather","Studded Leather","Brigandine Armor","Chain Mail","Double Chain","Augmented Chain","Chain Hauberk","Metal Breastplate","Augmented Plate","Half Plate","Full Plate"]
ASG={a:(1 if i==0 else i+4) for i,a in enumerate(ARM)}
AG=lambda asg: 1 if asg<=4 else (asg-1)//4+1
ct=collections.defaultdict(dict); lines={}
for n,l in enumerate(src,1):
    m=re.match(r"\s*'.b\((\d+)\|([^)]*)\).b'\s*=>\s*\{\s*:armor_type\s*=>\s*\"([^\"]+)\",\s*:avd\s*=>\s*(-?\d+),\s*:df\s*=>\s*([\d.]+),\s*:wt\s*=>\s*\"([^\"]+)\",\s*:dt\s*=>\s*\"([^\"]+)\"",l)
    if m:
        wt=m.group(6); asg=ASG[m.group(3).strip()]
        ct[wt][asg]=(int(m.group(4)),float(m.group(5)),n)
        lines.setdefault(wt,n)
print('crit-tracking weapons:',len(ct),'rows',sum(len(v) for v in ct.values()))
W=[l.split('\t') for l in open('E:/Cena/crates/cena-model/data/weapons.tsv',encoding='utf-8').read().splitlines()]
hdr=W[0]; cw={}
for r in W[1:]:
    d=dict(zip(hdr,r)); cw.setdefault(d['base_name'].lower(),[]).append(d)
AL=[l.split('\t') for l in open('E:/Cena/crates/cena-model/data/armament_aliases.tsv',encoding='utf-8').read().splitlines()]
alias={}
for r in AL[1:]:
    if r[0]=='weapon': alias.setdefault(r[1].lower(),r[2])
byid={}
for r in W[1:]:
    d=dict(zip(hdr,r)); byid.setdefault(d['id'],[]).append(d)
nodiff=0; diffs=[]; missing=[]
for wt,rows in ct.items():
    key=wt.lower()
    cands=cw.get(key) or byid.get(alias.get(key,''),[]) or byid.get(key.replace(' ','_'),[])
    if not cands: missing.append((wt,lines[wt])); continue
    best=None
    for d in cands:
        avd=d['avd_by_asg_1_to_20'].split('|'); df=d['damage_factor_by_ag_0_to_5'].split('|')
        dd=[]
        for asg,(a,f,n) in sorted(rows.items()):
            ca=avd[asg-1] if asg-1<len(avd) else ''
            cf=df[AG(asg)] if AG(asg)<len(df) else ''
            if ca=='' or int(ca)!=a: dd.append(f"asg{asg} avd {a} vs cena {ca or '-'} (crit-tracking.lic:{n})")
            if cf=='' or abs(float(cf)-f)>1e-6: dd.append(f"asg{asg} df {f} vs cena {cf or '-'} (crit-tracking.lic:{n})")
        if best is None or len(dd)<len(best[1]): best=(d,dd)
    if best[1]: diffs.append((wt,best[0]['category'],best[0]['id'],best[1]))
    else: nodiff+=1
print('identical',nodiff,'differ',len(diffs),'unmapped',missing)
for wt,cat,i,dd in diffs:
    print(f"== {wt} -> cena {cat}/{i}: {len(dd)} diffs"); [print('   ',x) for x in dd[:8]]
