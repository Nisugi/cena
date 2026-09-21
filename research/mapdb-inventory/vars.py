"""Every UserVars.* and $global the mapdb StringProcs touch, with read/write counts."""
import json, collections, re

d = json.load(open('E:/Cena/reference/mapdb/map-1789942730.json', encoding='utf-8'))
BS = chr(92)
VAR = re.compile('(UserVars' + BS + '.[A-Za-z_]+|' + BS + '$[a-z][A-Za-z0-9_]*|CharSettings' + BS + "[[^" + BS + "]]+" + BS + ']|Vars' + BS + "[[^" + BS + "]]+" + BS + '])')
WRITE = re.compile(BS + 's*(?:' + BS + '|' + BS + '|)?=(?!=|~)')

seen = collections.defaultdict(lambda: collections.Counter())
for r in d:
    for field in ('wayto', 'timeto'):
        for v in r[field].values():
            if not (isinstance(v, str) and v.startswith(';e')):
                continue
            for m in VAR.finditer(v):
                name = m.group(1)
                kind = 'write' if WRITE.match(v, m.end()) else 'read'
                seen[name][(field, kind)] += 1
    for t in r.get('tags') or []:
        if t.startswith('silver-cost:'):
            seen['TAG silver-cost'][('tags', 'n')] += 1
        if t.startswith('meta:'):
            seen['TAG ' + ':'.join(t.split(':')[:2])][('tags', 'n')] += 1

rows = sorted(seen.items(), key=lambda kv: -sum(kv[1].values()))
with open('E:/Cena/research/mapdb-inventory/vars.tsv', 'w', encoding='utf-8') as f:
    for name, c in rows:
        f.write(name + '\t' + '\t'.join(f'{a}.{b}={n}' for (a, b), n in sorted(c.items())) + '\n')
print(len(rows))
for name, c in rows[:70]:
    print(name, dict(c))
