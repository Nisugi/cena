"""scan.py <script.lic> [minlit]: pull every regex literal from a script, and say whether Cena
has it: combat TSV patterns (three-way test in cenamatch), or a literal fragment in any Rust
source or TSV under crates/ (lowercased, regex escapes removed)."""
import re, sys, collections
from pathlib import Path
sys.path.insert(0, str(Path(__file__).parent))
from cenamatch import load, covered, literals

LIB = Path('E:/Cena/reference/lich_repo_mirror/lib')
CR = Path('E:/Cena/crates')

_corpus = None


def corpus():
    global _corpus
    if _corpus is None:
        parts = []
        for p in list(CR.rglob('*.rs')) + list(CR.rglob('*.tsv')):
            if 'target' in p.parts:
                continue
            t = p.read_bytes().decode('utf-8', 'replace')
            t = re.sub(r'\\(.)', r'\1', t)
            parts.append((str(p.relative_to(CR.parent)).replace('\\', '/'), t.lower()))
        _corpus = parts
    return _corpus


def where(frag):
    frag = frag.lower()
    for name, t in corpus():
        i = t.find(frag)
        if i >= 0:
            line = t.count('\n', 0, i) + 1
            return f'{name}:{line}'
    return None


RX = re.compile(r'(?<![\w)\]])/((?:[^/\\\n]|\\.){6,})/[imxo]*')


def regexes(path):
    src = path.read_bytes().decode('utf-8', 'replace').splitlines()
    out = []
    for n, l in enumerate(src, 1):
        s = l.strip()
        if s.startswith('#'):
            continue
        for m in RX.finditer(l):
            out.append((n, m.group(1)))
    return out


def main():
    path = LIB / sys.argv[1]
    minlit = int(sys.argv[2]) if len(sys.argv) > 2 else 14
    pats = load()
    seen = set()
    stats = collections.Counter()
    for n, rx in regexes(path):
        lits = sorted(literals(rx), key=len, reverse=True)
        if not lits or len(lits[0]) < minlit:
            continue
        if rx in seen:
            continue
        seen.add(rx)
        if 'pushBold' in rx or '<a exist' in rx or '<\\/a>' in rx:
            rx = re.sub(r'<\\?/?[a-zA-Z][^<>]*>', '', rx)
            lits = sorted(literals(rx), key=len, reverse=True)
        hits = covered(pats, rx)
        if hits:
            stats['combat'] += 1
            print(f'HAVE   {sys.argv[1]}:{n} [{hits[0][1]} {hits[0][0]}] {rx[:150]}')
            continue
        # fragment in Rust/TSV
        w = None
        for L in lits[:3]:
            if len(L) >= minlit:
                w = where(L[:40]) or where(L[-40:])
                if w:
                    break
        if w:
            stats['frag'] += 1
            print(f'FRAG   {sys.argv[1]}:{n} [{w}] {rx[:150]}')
        else:
            stats['gap'] += 1
            print(f'GAP    {sys.argv[1]}:{n} {rx[:170]}')
    print('#', dict(stats))


if __name__ == '__main__':
    main()
