import json,collections,sys
r=json.load(open('critcmp.json',encoding='utf-8'))
rows=[l.split('\t') for l in open('E:/Cena/crates/cena-model/data/crit_tables.tsv',encoding='utf-8').read().splitlines()]
hdr=rows[0]; keys={}
for i,row in enumerate(rows[1:],start=2):
    d=dict(zip(hdr,row)); keys[(d['type'],d['location'],int(d['rank']))]=(i,d)
def look(t,l,k):
    locs=[l] if l not in ('arm','leg','hand','eye','foot') else ['right_'+l,'left_'+l]
    for loc in locs:
        if (t,loc,k) in keys: return keys[(t,loc,k)]
    # also try other side for side rows
    if l.startswith('right_') and (t,'left_'+l[6:],k) in keys: return keys[(t,'left_'+l[6:],k)]
    if l.startswith('left_') and (t,'right_'+l[5:],k) in keys: return keys[(t,'right_'+l[5:],k)]
    return None
s=sys.argv[1]
by=collections.defaultdict(list)
for x in r[s]:
    p=x['parsed']
    if x['status']=='MISSING':
        if not p: by['UNPARSED-MISSING'].append(f"{x['line']}: {x['sample'][:120]}"); continue
        c=look(p['type'],p['loc'],p['rank'])
        by[p['type']].append(f"{x['line']} {p['loc']}/{p['rank']} d{p['damage']} | {x['sample'][:80]}\n        cena {('tsv:'+str(c[0])+' d'+c[1]['damage']+' '+c[1]['pattern'][:90]) if c else 'NO ROW'}")
for k,v in by.items():
    print('==',k,len(v))
    for l in v: print('  ',l)
