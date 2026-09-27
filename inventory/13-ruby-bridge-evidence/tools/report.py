"""Tables from census.json.  python report.py"""
import json
from collections import Counter, defaultdict

d = json.load(open('census.json'))
GROUPS = ['G1', 'G2', 'G3', 'G4', 'G5', 'G6', 'G7', 'G8', 'G9', 'G15', 'G10', 'G11', 'G12', 'G13', 'G14']
BLOCKERS = ['G10rw', 'G11', 'G12', 'G13', 'G14']
HYDRA = {'go2': 'travel', 'eloot': 'loot', 'eherbs': 'heal', 'useherbs': 'heal', 'bigshot': 'hunt',
         'ewaggle': 'waggle', 'waggle': 'waggle', 'spellactive': 'keep', 'spellcaster': 'sc', 'sorter': 'sorter',
         'foreach': 'foreach', 'multi': 'multi', 'loottracker': 'loot ledger'}
ALIAS = {'repo': 'repository', 'chat': 'lnet'}


def hook_state(r):
    """'none' | 'ro' | 'rw' for G10."""
    g = r['groups'].get('G10')
    if not g:
        return 'none'
    for h in r['hooks']:
        if not h[1].startswith('read-only'):
            return 'rw'
    return 'ro'


def blockers(r):
    b = set()
    if hook_state(r) == 'rw':
        b.add('G10rw')
    for g in ('G11', 'G12', 'G13', 'G14'):
        if r['groups'].get(g):
            b.add(g)
    return b


def tier(r):
    gs = set(r['groups'])
    if blockers(r):
        return 'blocked'
    if 'G10' in gs:
        return 'T3 +read-only hooks'
    if gs & {'G8', 'G9', 'G15'}:
        return 'T2 +engine/stores'
    if gs:
        return 'T1 connection+model'
    return 'T0 no Lich API'


for c in ('A', 'B'):
    rows = [r for r in d[c] if not r['dr']]
    dr = [r for r in d[c] if r['dr']]
    n = len(rows)
    L = sum(r['lines'] for r in rows)
    print(f'\n### Collection {c}: {n} GemStone files ({L} lines); DragonRealms files: {len(dr)} {[r["file"] for r in dr]}')
    print('| Group | files | % |')
    for g in GROUPS:
        k = sum(1 for r in rows if r['groups'].get(g))
        print(f'| {g} | {k} | {100 * k / n:.1f} |')
    # sub rows
    sub = Counter()
    for r in rows:
        for g in ('G8', 'G9', 'G10', 'G12', 'G13', 'G14', 'G15', 'G6', 'G2', 'G1', 'G4', 'G5', 'G7', 'G3', 'G11'):
            for lab in r['groups'].get(g, []):
                sub[(g, lab)] += 1
    print('sub-rows:')
    for (g, lab), k in sorted(sub.items()):
        print(f'   {g} {lab}: {k}')
    # hooks
    hk = Counter()
    hfiles = Counter()
    xmlro = 0
    for r in rows:
        kinds = defaultdict(set)
        for h in r['hooks']:
            v = h[1].split(' (')[0]
            hk[(h[0], v)] += 1
            kinds[h[0]].add(v)
            if h[0] == 'DownstreamHook' and v == 'read-only' and h[3]:
                xmlro += 1
        for kname, vs in kinds.items():
            hfiles[(kname, 'rw' if vs - {'read-only'} else 'ro')] += 1
        if r['groups'].get('G10') and not r['hooks']:
            hfiles[('upstream_get only', 'ro')] += 1
    print('hooks (count of hooks):', sorted(hk.items()))
    print('hook files:', sorted(hfiles.items()))
    print('read-only downstream hooks matching XML markup:', xmlro)
    print('G10 files: ro', sum(1 for r in rows if hook_state(r) == 'ro'), 'rw', sum(1 for r in rows if hook_state(r) == 'rw'))
    print('unclear/unresolved hooks:', [(r['file'], h[0], h[1]) for r in rows for h in r['hooks'] if h[1] in ('unclear', 'unresolved')])
    # tiers
    print('| Tier | files | % files | lines | % lines |')
    tc = Counter()
    tl = Counter()
    for r in rows:
        t = tier(r)
        tc[t] += 1
        tl[t] += r['lines']
    for t in ['T0 no Lich API', 'T1 connection+model', 'T2 +engine/stores', 'T3 +read-only hooks', 'blocked']:
        print(f'| {t} | {tc[t]} | {100 * tc[t] / n:.1f} | {tl[t]} | {100 * tl[t] / L:.1f} |')
    # cumulative blockers
    print('| Blocker | files with it | only blocker | new | cumulative | cum lines |')
    seen = set()
    cl = 0
    for b in BLOCKERS:
        withb = [r for r in rows if b in blockers(r)]
        only = [r for r in withb if blockers(r) == {b}]
        new = [r for r in withb if r['file'] not in seen]
        for r in new:
            seen.add(r['file'])
            cl += r['lines']
        print(f'| {b} | {len(withb)} | {len(only)} | {len(new)} | {len(seen)} | {cl} ({100 * cl / L:.1f}%) |')
        if len(only) <= 35:
            print('     only:', sorted(r['file'][:-4] for r in only))
    # squelch-only vs text-changing hooks
    sq = Counter()
    for r in rows:
        for h in r['hooks']:
            v = h[1]
            if v.startswith('rewrite'):
                sig = set(v[v.index('(') + 1:-1].split(','))
                sq[(h[0], 'squelch only' if sig <= {'nil', 'branch', 'conditional'} else 'changes text/other')] += 1
            elif v in ('unclear', 'unresolved'):
                sq[(h[0], v)] += 1
    print('rewrite hooks split:', sorted(sq.items()))
    # variants
    string_eval_only = [r for r in rows if blockers(r) == {'G14'} and 'eval of strings / bindings' not in r['groups']['G14']]
    print('blocked only by G14 dispatch (send/define_method), no string eval:', len(string_eval_only), sum(r['lines'] for r in string_eval_only))
    fr = [r for r in rows if not blockers(r) and r['info'].get('I_file_read')]
    print('unblocked files that read files (would block if reads counted as G12):', len(fr), sorted(r['file'][:-4] for r in fr))
    # blocker combos
    combo = Counter(tuple(sorted(blockers(r))) for r in rows if blockers(r))
    print('blocker combos:', combo.most_common(8))
    # info
    for k in ('T_threads', 'I_file_read', 'I_quiet_command_xml', 'I_frontend_branch', 'I_ivar_const_get', 'I_method_missing'):
        print('info', k, sum(1 for r in rows if r['info'].get(k)))
    print('file reads without G12:', sum(1 for r in rows if r['info'].get('I_file_read') and not r['groups'].get('G12')))
    print('lex errors:', [r['file'] for r in rows if r['lex_errors']])
    print('T0 examples:', sorted(r['file'] for r in rows if tier(r).startswith('T0'))[:30])
    print('dynamic starts files:', sum(1 for r in rows if r['dynamic_starts']))
    # tiers ignoring G13-only-from-XMLData etc: nothing
    # largest files per tier
    for t in sorted(tc):
        big = sorted((r for r in rows if tier(r) == t), key=lambda r: -r['lines'])[:6]
        print('  largest', t, [(r['file'][:-4], r['lines']) for r in big])

# dependency graph across both
print('\n### Dependencies')
callers = defaultdict(lambda: {'A': set(), 'B': set()})
kinds = defaultdict(Counter)
edges_n = Counter()
dyn = Counter()
exists = {c: {r['file'][:-4].lower() for r in d[c]} for c in d}
for c in d:
    for r in d[c]:
        dyn[c] += r['dynamic_starts']
        for k, t in r['edges']:
            # Lich resolves a unique name prefix (lich-5 lib/common/script.rb:432-434): ;repo -> repository.
            # ;chat is lnet's command, caught by lnet's UpstreamHook (scripts/lnet.lic:1263).
            t = ALIAS.get(t, t)
            if t == r['file'][:-4].lower():
                continue
            callers[t][c].add(r['file'][:-4].lower())
            kinds[t][k] += 1
            edges_n[c] += 1
print('edges', dict(edges_n), 'dynamic', dict(dyn))
for c in d:
    print(c, 'callers with any edge', len({r['file'] for r in d[c] if r['edges']}),
          'distinct targets', len({t for r in d[c] for k, t in r['edges']}),
          'callers starting', len({r['file'] for r in d[c] if any(k == 'start' for k, t in r['edges'])}))
rank = sorted(callers, key=lambda t: -(len(callers[t]['A']) + len(callers[t]['B'])))
print('| # | target | A callers | B callers | total | start/query/control | in A | in B | Hydra |')
for i, t in enumerate(rank[:45], 1):
    a, b = len(callers[t]['A']), len(callers[t]['B'])
    kk = kinds[t]
    q = kk['running'] + kk['exists']
    print(f"| {i} | {t} | {a} | {b} | {a + b} | {kk['start']}/{q}/{kk['control']} | {'y' if t in exists['A'] else '-'} | {'y' if t in exists['B'] else '-'} | {HYDRA.get(t, 'no Hydra equivalent')} |")
# callers with start edges only, per target, top
tot_hydra = Counter()
for c in d:
    for r in d[c]:
        ts = {t for k, t in r['edges']}
        if ts:
            tot_hydra[(c, 'any')] += 1
            if ts & set(HYDRA):
                tot_hydra[(c, 'calls a Hydra built-in')] += 1
            if ts - set(HYDRA):
                tot_hydra[(c, 'calls a non-Hydra script')] += 1
print(tot_hydra)
