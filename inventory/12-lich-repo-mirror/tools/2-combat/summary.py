import json, collections
r = json.load(open('critcmp.json', encoding='utf-8'))
rows = [l.split('\t') for l in open('E:/Cena/crates/cena-model/data/crit_tables.tsv', encoding='utf-8').read().splitlines()]
hdr = rows[0]; keys = {}
for i, row in enumerate(rows[1:], start=2):
    d = dict(zip(hdr, row)); keys[(d['type'], d['location'], int(d['rank']))] = (i, d)
cena_types = collections.Counter(k[0] for k in keys)

def has_key(t, l, k):
    locs = [l] if l not in ('arm', 'leg', 'hand', 'eye', 'foot') else ['right_' + l, 'left_' + l]
    if l.startswith('right_'): locs.append('left_' + l[6:])
    if l.startswith('left_'): locs.append('right_' + l[5:])
    return any((t, x, k) in keys for x in locs)

for s, xs in r.items():
    types = collections.Counter((x['parsed'] or {}).get('type') for x in xs)
    cls = collections.Counter()
    for x in xs:
        p = x['parsed']
        if p is None:
            cls['not a crit-table row'] += 1
            continue
        if x['status'] == 'SAME':
            cls['same'] += 1
        elif x['status'] == 'DIFF':
            d = [z for z in x['diffs'] if not z.startswith('loc ')]
            cls['same (side-less location)' if not d else 'field differs'] += 1
        else:
            cls['text differs, Cena has key' if has_key(p['type'], p['loc'], p['rank']) else 'no Cena key'] += 1
    print(s, len(xs), dict(cls))
    print('   types', dict(types))
    missing_types = sorted(set(cena_types) - set(types))
    print('   Cena types absent from script:', missing_types)
print('cena types', dict(cena_types))
