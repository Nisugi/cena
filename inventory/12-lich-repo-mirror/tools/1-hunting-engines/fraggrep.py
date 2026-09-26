import os, sys

ROOT_CENA = r'E:\Cena\crates'
ROOT_LICH = r'E:\Cena\reference\lich-5\lib'
here = os.path.dirname(os.path.abspath(__file__))
frags = [l.strip() for l in open(os.path.join(here, sys.argv[1] if len(sys.argv) > 1 else 'frags.txt'), encoding='utf-8') if l.strip()]


def load(root, exts):
    files = {}
    for dp, dn, fn in os.walk(root):
        if 'target' in dp.split(os.sep):
            continue
        for f in fn:
            if f.endswith(exts):
                p = os.path.join(dp, f)
                try:
                    files[p] = open(p, 'rb').read().decode('utf-8', 'replace').split('\n')
                except OSError:
                    pass
    return files


cena = load(ROOT_CENA, ('.rs', '.tsv'))
lich = load(ROOT_LICH, ('.rb',))


def hits(files, frag, root, limit=4):
    out = []
    fl = frag.lower()
    for p, lines in files.items():
        for i, l in enumerate(lines, 1):
            ll = l.lower()
            # tolerate regex escaping in sources: strip backslashes
            if fl in ll.replace('\\', ''):
                out.append('%s:%d' % (os.path.relpath(p, root).replace('\\', '/'), i))
    return out


for frag in frags:
    c = hits(cena, frag, r'E:\Cena')
    l = hits(lich, frag, r'E:\Cena\reference\lich-5')
    print('%s\tCENA(%d): %s\tLICH(%d): %s' % (frag, len(c), ' '.join(c[:5]), len(l), ' '.join(l[:4])))
