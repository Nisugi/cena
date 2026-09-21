"""Compare reference/mapdb/map-data/prime (uid-native) against the Lich mapdb."""
import json, collections

M = 'E:/Cena/reference/mapdb/'
lich = json.load(open(M + 'map-1789942730.json', encoding='utf-8'))
prime = json.load(open(M + 'map-data/prime/rooms.json', encoding='utf-8'))['rooms']
tags = json.load(open(M + 'map-data/prime/tags.json', encoding='utf-8'))['tags']
cre = json.load(open(M + 'map-data/prime/creatures.json', encoding='utf-8'))
human = json.load(open(M + 'map-data/human-authored-map-data.json', encoding='utf-8'))

print('prime rooms', len(prime))
keys = collections.Counter(k for r in prime for k in r)
print('room keys', keys.most_common())
P = {r['id']: r for r in prime}
kinds = collections.Counter(); ncols = collections.Counter(); cmds = collections.Counter()
edges = 0; dangling = 0
for r in prime:
    for x in r.get('x', []):
        edges += 1; ncols[len(x)] += 1; kinds[x[2] if len(x) > 2 else None] += 1
        if x[0] not in P: dangling += 1
        if len(x) > 2 and x[2] not in ('c',): cmds[(x[2], (str(x[1]).split() or ['<empty>'])[0])] += 1
print('edges', edges, 'dangling', dangling, 'cols', dict(ncols), 'kinds', kinds.most_common())
print('non-cardinal edge verbs', cmds.most_common(15))
ex = [r for r in prime if any(len(x) > 3 for x in r.get('x', []))][:2]
print('sample wide edges', [r['x'] for r in ex])

# uid overlap
lich_uids = {}
for r in lich:
    for u in r.get('uid') or []:
        lich_uids.setdefault(u, []).append(r['id'])
both = set(lich_uids) & set(P)
print('lich distinct uids', len(lich_uids), 'prime ids', len(P), 'in both', len(both),
      'lich-only', len(set(lich_uids) - set(P)), 'prime-only', len(set(P) - set(lich_uids)))
neg = [u for u in lich_uids if u < 0]
print('negative lich uids in prime', sum(1 for u in neg if u in P), 'of', len(neg))

# can prime fill in uids for lich rooms that lack one? match by title+desc
def norm(s): return ' '.join((s or '').split())
idx = collections.defaultdict(list)
for r in prime:
    idx[(norm(r.get('t')), norm(r.get('desc')))].append(r['id'])
no_uid = [r for r in lich if not r.get('uid')]
hit1 = hitn = 0
for r in no_uid:
    c = set()
    for t in r.get('title') or []:
        for d in r.get('description') or []:
            c.update(idx.get((norm(t.strip('[]')), norm(d)), []))
    if len(c) == 1: hit1 += 1
    elif len(c) > 1: hitn += 1
print('lich no-uid rooms', len(no_uid), 'matched to exactly one prime room by title+desc', hit1, 'ambiguous', hitn)

# segments: id // 1000 ?
seg = collections.Counter(i // 1000 for i in P)
print('segments (id//1000)', len(seg), 'largest', seg.most_common(5))
shopseg = [i for i in P if 631 <= i // 1000 <= 646]
print('prime rooms in playershop segments 631..646:', len(shopseg))
print('prime-only by loc', collections.Counter(P[i].get('loc') for i in set(P) - set(lich_uids)).most_common(12))

# multi-uid lich rooms: what are they
multi = [r for r in lich if len(r.get('uid') or []) > 1]
print('multi-uid rooms', len(multi))
print(' by size', collections.Counter(len(r['uid']) for r in multi).most_common(8))
print(' by location', collections.Counter(str(r.get('location')) for r in multi).most_common(10))
big = sorted(multi, key=lambda r: -len(r['uid']))[:6]
for r in big: print('  ', r['id'], len(r['uid']), r.get('title'), r.get('location'), 'same segment' if len({u // 1000 for u in r['uid']}) == 1 else 'segments ' + str(sorted({u // 1000 for u in r['uid']}))[:60])
# do prime's rooms for one multi-uid lich room differ?
r = big[0]; ts = collections.Counter((P[u].get('t'), norm(P[u].get('desc'))[:40]) for u in r['uid'] if u in P)
print('  prime variants for', r['id'], len(ts), list(ts.items())[:3])

# shared uids
shared = {u: ids for u, ids in lich_uids.items() if len(ids) > 1}
print('shared uids', len(shared))
L = {r['id']: r for r in lich}
for u, ids in list(shared.items())[:8]:
    print('  u%s' % u, [(i, (L[i].get('title') or ['?'])[0], len(L[i].get('description') or []), [t for t in (L[i].get('tags') or []) if 'meta' in t and 'forage' not in t][:2]) for i in ids])

print('tags', len(tags), 'creature rooms', len(cre['byRoom']), 'human maps', len([k for k in human if not k.startswith('_')]))
hk = [k for k in human if not k.startswith('_')][0]
print('human sample', hk, json.dumps(human[hk])[:600])
