"""Print hook bodies with their verdicts for hand-checking: python hookshow.py <A|B> <verdict-prefix> <n> [seed]"""
import json
import random
import re
import sys

import census
from rblex import lex

coll, want, n = sys.argv[1], sys.argv[2], int(sys.argv[3])
seed = int(sys.argv[4]) if len(sys.argv) > 4 else 1
kind_filter = sys.argv[5] if len(sys.argv) > 5 else None
d = json.load(open('census.json'))
items = []
for r in d[coll]:
    for i, (k, v, det) in enumerate(r['hooks']):
        if v.startswith(want) and (kind_filter is None or k == kind_filter):
            items.append((r['file'], i, k, v, det))
random.Random(seed).shuffle(items)
base = census.COLLS[coll].replace('*.lic', '')
for f, i, k, v, det in items[:n]:
    src = census.read(base + f).replace('\r\n', '\n')
    lx = lex(src)
    code = census.NORM_PREFIX2.sub('', census.NORM_PREFIX.sub('', lx.code))
    ms = list(re.finditer(r'(?<![\w])(DownstreamHook|UpstreamHook)\.add\s*(\(?)', code))
    m = ms[i]
    line = code.count('\n', 0, m.start()) + 1
    print('=' * 100)
    print(f'{f}:{line}  {k}  {v}  [{det}]')
    lines = src.split('\n')
    # show from the add line, plus the proc definition if it's a variable
    if det.startswith('var ') or det.startswith('method('):
        name = det.split(' ', 1)[1] if det.startswith('var ') else det[7:-1]
        last = name.split('.')[-1]
        pm = re.search(r'(?<![\w.])(?:' + re.escape(name) + r'\s*=\s*(?:proc|lambda|Proc\.new|->)|def\s+(?:self\.)?' + re.escape(last) + r'\b)', code)
        if pm:
            pl = code.count('\n', 0, pm.start())
            print('\n'.join(f'{pl + 1 + j:5}: {x}' for j, x in enumerate(lines[pl:pl + 25])))
            continue
    print('\n'.join(f'{line + j:5}: {x}' for j, x in enumerate(lines[line - 1:line + 24])))
