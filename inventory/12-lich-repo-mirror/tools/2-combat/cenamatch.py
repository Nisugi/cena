"""Shared: load every Cena combat pattern (combat_*.tsv, crit_tables.tsv) and decide whether a
script regex is covered: either the script's synthesized sample matches a Cena pattern, or a
Cena pattern's synthesized sample (every alternation expanded, capped) matches the script regex."""
import re, itertools
from pathlib import Path

DATA = Path('E:/Cena/crates/cena-model/data')


def rb2py(p):
    p = re.sub(r"\(\?<(?![=!])", "(?P<", p)
    p = p.replace('\\h', '[ \\t]').replace('\\A', '^').replace('\\z', '$').replace('\\Z', '$')
    p = re.sub(r'#\{[^}]*\}', '.+?', p)
    p = p.replace('(?i:', '(?:')
    return p


def expand(p, cap=24):
    """Expand the innermost alternation groups of a regex into literal variants."""
    out = [p]
    for _ in range(6):
        nxt = []
        changed = False
        for s in out:
            m = re.search(r'\((?:\?:|\?P<\w+>|\?<\w+>)?([^()]*)\)([?*+]?)', s)
            if not m:
                nxt.append(s); continue
            changed = True
            opts = m.group(1).split('|')
            if m.group(2) in ('?', '*'):
                opts = opts + ['']
            for o in opts:
                nxt.append(s[:m.start()] + o + s[m.end():])
        out = nxt[:cap]
        if not changed:
            break
    return out


def synth(rx):
    res = []
    for s in expand(rx):
        s = s.strip().lstrip('^').rstrip('$')
        s = s.replace('\\b', '').replace('\\A', '').replace('\\z', '')
        s = re.sub(r'#\{[^}]*\}', 'kobold', s)
        s = re.sub(r'\[\^[^\]]*\][+*]\??', 'kobold', s)
        s = re.sub(r'\[[^\]]*\][+*]\??', 'x', s)
        s = re.sub(r'\\w\+\??|\\S\+\??|\\w\*|\\D\+', 'kobold', s)
        s = re.sub(r'\.\*\?|\.\+\?|\.\*|\.\+', 'kobold', s)
        s = s.replace('\\s+', ' ').replace('\\s*', ' ').replace('\\s', ' ').replace('\\d+', '5').replace('\\d', '5')
        s = re.sub(r'(?<!\\)[?](?![a-z])', '', s)
        s = re.sub(r'\\(.)', r'\1', s)
        res.append(s)
    return res


def load():
    pats = []
    for name in ['combat_attacks.tsv', 'combat_effects.tsv', 'combat_results.tsv', 'crit_tables.tsv']:
        lines = (DATA / name).read_text(encoding='utf-8').splitlines()
        hdr = lines[0].split('\t')
        for i, l in enumerate(lines[1:], start=2):
            f = dict(zip(hdr, l.split('\t')))
            if not f.get('pattern'):
                continue
            try:
                rx = re.compile(rb2py(f['pattern']))
            except re.error:
                continue
            label = f"{f['family']}/{f['name']}" if 'family' in f else f"crit/{f['type']}/{f['location']}/{f['rank']}"
            pats.append((f'{name}:{i}', label, rx, synth(f['pattern'])))
    return pats


def literals(rx):
    """Literal runs of a regex, unescaped."""
    s = re.sub(r'\(\?<\w+>|\(\?P<\w+>|\(\?:|\(\?i:', '(', rx)
    s = re.sub(r'#\{[^}]*\}', '\x00', s)
    s = re.sub(r'\[[^\]]*\][+*?]?|\\[wWsSdDbBAzZh][+*?]*\??|\.[+*]\??|[()|^$]|[+*?]\??|\{\d+(,\d*)?\}', '\x00', s)
    s = re.sub(r'\\(.)', r'\1', s)
    return [p.strip() for p in s.split('\x00') if p.strip()]


def pattern_text(p):
    return ' '.join(literals(p)).lower()


def covered(pats, script_rx, flags=0):
    lits = sorted(literals(script_rx), key=len, reverse=True)
    key = lits[0].lower() if lits and len(lits[0]) >= 18 else None
    if key is not None:
        keys = {key[:28], key[-28:]}
        frag = [(src, label) for src, label, rx, cs in pats if any(k in pattern_text(rx.pattern) for k in keys)]
        if frag:
            return frag
    return covered_strict(pats, script_rx, flags)


def covered_strict(pats, script_rx, flags=0):
    try:
        srx = re.compile(rb2py(script_rx), flags)
    except re.error:
        srx = None
    samples = [s + suf for s in synth(script_rx) for suf in ('', '!', '.', '! **', '!  **', '. **', '!"', ' kobold!', ' kobold.', ' kobold! **')]
    hits = []
    for src, label, rx, csamples in pats:
        if any(rx.search(s) for s in samples) or (srx is not None and any(srx.search(c) for c in csamples)):
            hits.append((src, label))
    return hits
