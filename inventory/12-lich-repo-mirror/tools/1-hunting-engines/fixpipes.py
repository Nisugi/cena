import os

here = os.path.dirname(os.path.abspath(__file__))
p = os.path.join(here, 'huntpro-family.md')
lines = open(p, encoding='utf-8').read().split('\n')
subs = [
    ('`grep -cE "line =~|when /"`', '`grep -cE "line =~\\|when /"`'),
    ('`time_?limit|timelimit|duration`', '`time_?limit\\|timelimit\\|duration`'),
    ('`stomach|belly of the beast|swallow`', '`stomach\\|belly of the beast\\|swallow`'),
    ('`misfire|divergen`', '`misfire\\|divergen`'),
]
n = 0
for i, l in enumerate(lines):
    if not l.startswith('|'):
        continue
    for a, b in subs:
        if a in l:
            lines[i] = lines[i].replace(a, b)
            n += 1
open(p, 'w', encoding='utf-8').write('\n'.join(lines))
print('replaced', n)
