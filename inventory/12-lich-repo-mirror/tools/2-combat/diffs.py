import json,sys,collections
r=json.load(open('critcmp.json',encoding='utf-8'))
for s in sys.argv[1:]:
  c=collections.Counter(); out=[]
  for x in r[s]:
    if x['status']!='DIFF': continue
    d=[z for z in x['diffs'] if not z.startswith('loc ')]
    if not d: c['loc-only']+=1; continue
    for z in d: c[z.split()[0]]+=1
    p=x['parsed']; cc=x['cena']
    out.append(f"{x['line']} {p['type']}/{p['loc']}/{p['rank']} d{p['damage']} stat[{p['stat'][:30]}] | {'; '.join(d)} | cena tsv:{cc['tsvline']} {cc['pattern'][:70]}")
  print('=====',s,dict(c))
  for o in out: print('  ',o)
