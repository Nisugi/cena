"""calcredux.lic's incoming-crit table vs Cena crit_tables.tsv; and its DF tables vs weapons.tsv."""
import re, collections
src = open('E:/Cena/reference/lich_repo_mirror/lib/calcredux.lic', encoding='utf-8', errors='replace').read().splitlines()
rows = [l.split('\t') for l in open('E:/Cena/crates/cena-model/data/crit_tables.tsv', encoding='utf-8').read().splitlines()]
hdr = rows[0]
cena = []
for i, r in enumerate(rows[1:], start=2):
    d = dict(zip(hdr, r)); d['i'] = i
    d['rx'] = re.compile(re.sub(r"\(\?<(?![=!])", "(?P<", d['pattern']))
    cena.append(d)
crits = []
inside = False
for n, l in enumerate(src, 1):
    if l.startswith('crits = {'):
        inside = True
    if inside:
        for m in re.finditer(r'"([^"]+)"\s*=>\s*(\d+)', l):
            crits.append((n, m.group(1), int(m.group(2))))
        if l.strip().endswith('}') and not l.strip().startswith('crits'):
            break
print('calcredux crit rows', len(crits))
stat = collections.Counter()
for n, text, dmg in crits:
    sample = text.replace('.', '.').strip()
    hits = [c for c in cena if c['rx'].search(sample)]
    if not hits:
        # try as prefix: cena may need a trailing char
        hits = [c for c in cena if c['rx'].search(sample + '.')]
    if not hits:
        stat['missing'] += 1
        print(f"MISSING calcredux.lic:{n} d{dmg} '{text}'")
        continue
    hits.sort(key=lambda c: -len(c['pattern']))
    c = hits[0]
    if int(c['damage']) != dmg:
        stat['dmgdiff'] += 1
        print(f"DMG calcredux.lic:{n} d{dmg} '{text}' vs cena tsv:{c['i']} {c['type']}/{c['location']}/{c['rank']} d{c['damage']} '{c['pattern']}'")
    else:
        stat['same'] += 1
print(dict(stat))

# DF tables
ARMS = ['skin', 'leather', 'scale', 'chain', 'plate']
df = collections.defaultdict(dict)
cur = None
for n, l in enumerate(src, 1):
    m = re.match(r'df_(\w+) = \{', l)
    if m:
        cur = m.group(1)
    if cur:
        for mm in re.finditer(r'"([^"]+)"\s*=>\s*([\d.]+)', l):
            df[mm.group(1)][cur] = (float(mm.group(2)), n)
        if l.strip().endswith('}'):
            cur = None
W = [l.split('\t') for l in open('E:/Cena/crates/cena-model/data/weapons.tsv', encoding='utf-8').read().splitlines()]
wh = W[0]
AL = [l.split('\t') for l in open('E:/Cena/crates/cena-model/data/armament_aliases.tsv', encoding='utf-8').read().splitlines()]
alias = collections.defaultdict(set)
for r in AL[1:]:
    if r[0] == 'weapon':
        alias[r[1].lower()].add(r[2])
byid = collections.defaultdict(list)
for r in W[1:]:
    d = dict(zip(wh, r)); byid[d['id']].append(d)
same = 0; unm = []; dif = []
for name, vals in sorted(df.items()):
    ids = alias.get(name.lower()) or ({name.replace(' ', '_').replace('-', '_')} & set(byid))
    cands = [d for i in ids for d in byid.get(i, [])]
    if not cands:
        unm.append(name); continue
    best = None
    for d in cands:
        cdf = d['damage_factor_by_ag_0_to_5'].split('|')
        dd = []
        for ag, a in enumerate(ARMS, start=1):
            if a in vals:
                v, n = vals[a]
                c = cdf[ag] if ag < len(cdf) else ''
                if c == '' or abs(float(c) - v) > 1e-6:
                    dd.append(f"{a} {v} vs cena {c or '-'} (calcredux.lic:{n})")
        if best is None or len(dd) < len(best[1]):
            best = (d, dd)
    if best[1]:
        dif.append((name, best[0]['category'] + '/' + best[0]['id'], best[1]))
    else:
        same += 1
print('DF weapons', len(df), 'same', same, 'differ', len(dif), 'unmapped', unm)
for name, cid, dd in dif:
    print(f"  {name} -> {cid}: " + '; '.join(dd))
