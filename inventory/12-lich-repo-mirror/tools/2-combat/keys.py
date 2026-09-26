import json,collections
r=json.load(open('critcmp.json',encoding='utf-8'))
rows=[l.split('\t') for l in open('E:/Cena/crates/cena-model/data/crit_tables.tsv',encoding='utf-8').read().splitlines()]
hdr=rows[0]; keys={}
for i,row in enumerate(rows[1:],start=2):
    d=dict(zip(hdr,row)); keys[(d['type'],d['location'],int(d['rank']))]=(i,d)
per_type=collections.Counter(k[0] for k in keys)
print('cena rows per type', dict(per_type))
seen=collections.defaultdict(set)
for s,xs in r.items():
    for x in xs:
        p=x['parsed']
        if not p: continue
        locs=[p['loc']] if p['loc'] not in ('arm','leg','hand','eye','foot') else ['right_'+p['loc'],'left_'+p['loc']]
        if any((p['type'],l,p['rank']) in keys for l in locs): continue
        seen[(p['type'],p['loc'],p['rank'])].add((s,x['line'],x['status'],(x['cena'] or {}).get('type'),(x['cena'] or {}).get('loc'),(x['cena'] or {}).get('rank'),(x['cena'] or {}).get('damage'),p['damage'],x['sample'][:70]))
for k in sorted(seen):
    print(k)
    for v in sorted(seen[k]): print('    ',v)
