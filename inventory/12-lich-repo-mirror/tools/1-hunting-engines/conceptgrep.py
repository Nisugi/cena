import os, re, sys

ROOT = r'E:\Cena\crates'
here = os.path.dirname(os.path.abspath(__file__))
concepts = [l.rstrip('\n') for l in open(os.path.join(here, sys.argv[1]), encoding='utf-8') if l.strip()]

files = {}
for dp, dn, fn in os.walk(ROOT):
    if 'target' in dp.split(os.sep):
        continue
    for f in fn:
        if f.endswith(('.rs', '.tsv', '.toml')):
            p = os.path.join(dp, f)
            files[p] = open(p, 'rb').read().decode('utf-8', 'replace').split('\n')

only_src = len(sys.argv) > 2 and sys.argv[2] == 'src'
for c in concepts:
    rx = re.compile(c, re.I)
    hits = []
    for p, lines in files.items():
        rel = os.path.relpath(p, r'E:\Cena').replace('\\', '/')
        if only_src and ('/tests/' in rel):
            continue
        for i, l in enumerate(lines, 1):
            if rx.search(l):
                hits.append('%s:%d' % (rel, i))
    print('%s\t(%d)\t%s' % (c, len(hits), ' '.join(hits[:6])))
