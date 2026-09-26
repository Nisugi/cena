import json,re
r=json.load(open('critcmp.json',encoding='utf-8'))
rows=[l.split('\t') for l in open('E:/Cena/crates/cena-model/data/crit_tables.tsv',encoding='utf-8').read().splitlines()]
hdr=rows[0]; cena=[]
for i,row in enumerate(rows[1:],start=2):
    d=dict(zip(hdr,row)); d['i']=i; d['rx']=re.compile(re.sub(r"\(\?<(?![=!])","(?P<",d['pattern'])); cena.append(d)
def first_hit(s):
    hits=[c for c in cena if c['rx'].search(s)]
    hits.sort(key=lambda c:(-len(c['pattern']),c['type'],c['location'],int(c['rank'])))
    return hits
def sameloc(a,b):
    return a==b or a.split('_')[-1]==b.split('_')[-1]
n=0
for x in r['crittracker']:
    p=x['parsed']; msg=x['sample'].replace('the the kobold','the kobold').replace("[target's]","the kobold's")
    sents=[s.strip() for s in re.split(r'(?<=[.!?])\s+(?=[A-Z"])',msg) if s.strip()]
    for k,s in enumerate(sents):
        h=first_hit(s)
        if h:
            c=h[0]
            if not (c['type']==p['type'] and sameloc(c['location'],p['loc']) and int(c['rank'])==p['rank']):
                n+=1
                print(f"crittracker.lic:{x['line']} {p['type']}/{p['loc']}/{p['rank']} d{p['damage']} [{p['stat']}] sentence{k+1} '{s}' -> cena tsv:{c['i']} {c['type']}/{c['location']}/{c['rank']} d{c['damage']} fatal={c['fatal']} '{c['pattern']}'")
            break
    else:
        print(f"crittracker.lic:{x['line']} NO SENTENCE MATCHES: {msg}")
print(n)
