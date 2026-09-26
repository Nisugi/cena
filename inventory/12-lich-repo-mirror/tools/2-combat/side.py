import json,sys,csv
r=json.load(open('critcmp.json',encoding='utf-8'))
rows=[l.split('\t') for l in open('E:/Cena/crates/cena-model/data/crit_tables.tsv',encoding='utf-8').read().splitlines()]
hdr=rows[0]; idx={}
for i,row in enumerate(rows[1:],start=2):
    d=dict(zip(hdr,row)); idx[(d['type'],d['location'],d['rank'])]=(i,d)
def look(t,l,k):
    for loc in ([l] if '_' in l or l in('head','neck','chest','abdomen','back','nerves') else ['right_'+l,'left_'+l]):
        if (t,loc,str(k)) in idx: return idx[(t,loc,str(k))]
    return None
for s in sys.argv[1:]:
  print('=====',s)
  for x in r[s]:
    if x['status']=='MISSING':
      p=x['parsed']
      if not p: print(x['line'],'UNPARSED',x['sample'][:100]); continue
      c=look(p['type'],p['loc'],p['rank'])
      print(f"{x['line']} {p['type']}/{p['loc']}/{p['rank']} d{p['damage']} | {x['sample'][:90]}")
      print('      cena:', (f"tsv:{c[0]} d{c[1]['damage']} {c[1]['pattern']}" if c else 'NO ROW'))
