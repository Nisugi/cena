import json, collections, re
from pathlib import Path
exec(open('bestiary_cmp.py',encoding='utf-8').read().split('res = {')[0])
miss = collections.Counter(); extra = collections.Counter()
nmiss = 0; per = []
cena_area_vocab = set(a for s in areas.values() for a in s)
for n in both:
    b, c = best_by[n], cena_by[n]
    bl = set(x for x in b.get('locations', []) if x.strip() and 'Not Listed' not in x and x!='Unknown')
    cl = areas.get(c['id'], set())
    if not cl: continue
    m = bl - cl
    if m:
        nmiss += 1
        per.append((b['name'], sorted(m), sorted(cl)))
        for x in m: miss[x] += 1
print('creatures in both with a cena area whose bestiary lists a location Cena does not:', nmiss)
print('missing location names not in cena vocab at all:')
for k,v in miss.most_common():
    if k not in cena_area_vocab: print('  ', v, k)
print('missing location names that ARE cena vocab (so a creature-area pair is missing):', sum(v for k,v in miss.items() if k in cena_area_vocab))
for p in per[:400]:
    if any(x in cena_area_vocab for x in p[1]): print('  PAIR', p)
