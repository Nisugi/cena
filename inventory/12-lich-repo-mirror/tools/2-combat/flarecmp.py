import re, sys, collections
sys.path.insert(0, '.')
from cenamatch import load, covered

pats = load()
src = open('E:/Cena/reference/lich_repo_mirror/lib/flare_patterns.rb', encoding='utf-8', errors='replace').read().splitlines()
section = None
rows = []
for n, l in enumerate(src, 1):
    m = re.match(r'^(\w+) = \{', l)
    if m:
        section = m.group(1)
        continue
    m = re.match(r'\s*(\w+):\s*/(.*)/([imx]*),?\s*(#.*)?$', l)
    if m and section:
        rows.append((section, n, m.group(1), m.group(2), re.I if 'i' in m.group(3) else 0))
print('rows', len(rows), collections.Counter(r[0] for r in rows))


def split_top(rx):
    parts, depth, cur, i = [], 0, '', 0
    while i < len(rx):
        c = rx[i]
        if c == '\\':
            cur += rx[i:i + 2]; i += 2; continue
        if c == '(':
            depth += 1
        elif c == ')':
            depth -= 1
        if c == '|' and depth == 0:
            parts.append(cur); cur = ''
        else:
            cur += c
        i += 1
    parts.append(cur)
    return parts


hit = collections.Counter(); out = []
for sec, n, name, rx, fl in rows:
    alts = split_top(rx)
    res = [(a, covered(pats, a, fl)) for a in alts]
    got = [r for r in res if r[1]]
    labels = '; '.join(sorted({x[1] for r in got for x in r[1]}))[:140]
    missing = ' || '.join(r[0][:100] for r in res if not r[1])
    v = 'HAVE' if len(got) == len(res) else ('PARTIAL' if got else 'GAP')
    hit[v] += 1
    out.append((v, sec, n, name, labels, missing, len(res), len(got)))
print(dict(hit))
for o in out:
    print(f"{o[0]:8} {o[1][:6]} flare_patterns.rb:{o[2]} {o[3]} [{o[7]}/{o[6]} alts] :: {o[4]} :: MISSING {o[5]}")
