"""prime/rooms.json ids ARE uids and the file is >= 1 year old (author, 2026-09-20).
Compare them against the Lich mapdb's uids: what is only in one, and do shared rooms agree?"""
import json, collections

M = 'E:/Cena/reference/mapdb/'
lich = json.load(open(M + 'map-1789942730.json', encoding='utf-8'))
prime = json.load(open(M + 'map-data/prime/rooms.json', encoding='utf-8'))['rooms']
P = {r['id']: r for r in prime}
L = {r['id']: r for r in lich}
u2l = collections.defaultdict(list)
for r in lich:
    for u in r.get('uid') or []:
        u2l[u].append(r['id'])
LU, PU = set(u2l), set(P)
both, lonly, ponly = LU & PU, LU - PU, PU - LU
print(f'lich uids {len(LU)}  prime ids {len(PU)}  both {len(both)}  lich-only {len(lonly)}  prime-only {len(ponly)}')
print(f'share of lich uids found in prime: {len(both)/len(LU):.1%}; share of prime ids known to lich: {len(both)/len(PU):.1%}')

def norm(s): return ' '.join((s or '').split())

# --- lich-only: newer than prime, or a class prime never harvested?
print('\nLICH-ONLY uids')
print(' negative', sum(1 for u in lonly if u < 0))
pos = [u for u in lonly if u >= 0]
print(' by location', collections.Counter(str(L[u2l[u][0]].get('location')) for u in pos).most_common(12))
segs_p = {u // 1000 for u in PU}
print(' in a segment prime has at all:', sum(1 for u in pos if u // 1000 in segs_p), 'in a segment prime lacks:', sum(1 for u in pos if u // 1000 not in segs_p))
ids = sorted(u2l[u][0] for u in pos)
q = lambda a, f: a[int(len(a) * f)]
print(' lich id quartiles of those rooms', q(ids, .25), q(ids, .5), q(ids, .75), 'vs all lich', q(sorted(L), .25), q(sorted(L), .5), q(sorted(L), .75))
missing_seg = collections.Counter(u // 1000 for u in pos if u // 1000 not in segs_p)
print(' segments wholly absent from prime:', len(missing_seg), missing_seg.most_common(8))

# --- prime-only: unmapped by lich, or gone?
print('\nPRIME-ONLY ids')
print(' by loc', collections.Counter(P[i].get('loc') for i in ponly).most_common(14))
segs_l = {u // 1000 for u in LU}
print(' in a segment lich has at all:', sum(1 for i in ponly if i // 1000 in segs_l), 'in a segment lich lacks:', sum(1 for i in ponly if i // 1000 not in segs_l))
print(' with no exits', sum(1 for i in ponly if not P[i].get('x')), 'with no desc', sum(1 for i in ponly if not P[i].get('desc')))
# prime-only rooms that are copies (same title+desc) of a room lich DOES have -> instanced copies lich folded
sig = collections.defaultdict(list)
for i in both:
    sig[(norm(P[i]['t']), norm(P[i].get('desc')))].append(i)
copies = sum(1 for i in ponly if P[i].get('desc') and (norm(P[i]['t']), norm(P[i].get('desc'))) in sig)
print(' identical title+desc to a room lich has (likely instanced copies / unrecorded uids):', copies)
# reachable from the known world?
adj_known = sum(1 for i in ponly if any(x[0] in both for x in P[i].get('x', [])))
print(' with an exit into a room lich knows:', adj_known)

# --- rooms in both: do they agree?
print('\nIN BOTH')
t_ok = d_ok = d_cmp = 0
for u in both:
    lr = L[u2l[u][0]]; pr = P[u]
    if norm(pr['t']) in {norm(t.strip('[]')) for t in lr.get('title') or []}: t_ok += 1
    if pr.get('desc') and lr.get('description'):
        d_cmp += 1
        if norm(pr['desc']) in {norm(d) for d in lr['description']}: d_ok += 1
print(f' title agrees {t_ok}/{len(both)} ({t_ok/len(both):.1%}); description agrees {d_ok}/{d_cmp} ({d_ok/d_cmp:.1%})')

# edge agreement: prime edge (u -> v, cmd) vs lich wayto between the mapped ids
DIRS = {'north','south','east','west','northeast','northwest','southeast','southwest','up','down','out'}
agree = differ_cmd = lich_missing = 0; lich_proc = 0
for u in both:
    a = u2l[u][0]
    for v, cmd, kind in P[u].get('x', []):
        if v not in both: continue
        b = str(u2l[v][0]); w = L[a]['wayto'].get(b)
        if w is None: lich_missing += 1
        elif w.startswith(';e'): lich_proc += 1
        elif norm(w) == norm(cmd): agree += 1
        else: differ_cmd += 1
tot = agree + differ_cmd + lich_missing + lich_proc
print(f' prime edges between shared rooms {tot}: same command {agree} ({agree/tot:.1%}), lich scripted {lich_proc}, different command {differ_cmd}, lich has no such edge {lich_missing}')
# and the reverse: lich plain edges between shared rooms that prime lacks
l2u = {}
for u in both: l2u.setdefault(u2l[u][0], u)
rev_missing = rev_tot = 0
for a, u in l2u.items():
    pe = {x[0] for x in P[u].get('x', [])}
    for b, w in L[a]['wayto'].items():
        v = l2u.get(int(b))
        if v is None: continue
        rev_tot += 1
        if v not in pe: rev_missing += 1
print(f' lich edges between shared rooms {rev_tot}: absent from prime {rev_missing} ({rev_missing/rev_tot:.1%})')
