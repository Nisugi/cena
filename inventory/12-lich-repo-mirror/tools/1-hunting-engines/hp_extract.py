import re, sys, collections

path = sys.argv[1]
start = int(sys.argv[2]) if len(sys.argv) > 2 else 1
end = int(sys.argv[3]) if len(sys.argv) > 3 else 10**9
lines = open(path, 'rb').read().decode('utf-8', 'replace').split('\n')

pat = re.compile(r'(?:line|result|\$result)\s*=~\s*(/.*?/[imx]*)(?=\s*(?:$|\)|&&|\|\||then|unless|if))')
out = collections.OrderedDict()
for i, l in enumerate(lines, 1):
    if i < start or i > end:
        continue
    s = l.strip()
    if s.startswith('#'):
        continue
    m = pat.search(l)
    if not m:
        continue
    rx = m.group(1)
    # action: next non-blank non-comment line
    act = ''
    for j in range(i, min(i + 6, len(lines))):
        t = lines[j].strip()
        if t and not t.startswith('#') and not t.startswith('end'):
            act = t
            break
    out.setdefault(rx, []).append((i, act[:110]))

for rx, occ in out.items():
    lns = ','.join(str(o[0]) for o in occ[:12]) + ('...(%d)' % len(occ) if len(occ) > 12 else '')
    print('%s\t%s\t%s' % (lns, rx[:200], occ[0][1]))
