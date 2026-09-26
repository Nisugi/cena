# usage: msgcheck.py script.lic [line_from line_to]
# For each /regex/ literal (or quoted matchwait string) on the given lines, derive a
# sample sentence and test whether any message pattern Cena holds matches it:
# spells.tsv msg columns, combat_*.tsv patterns, creature_messages.tsv; else whether
# its longest literal chunk appears in any crates/**/*.rs file.
import re, sys, pathlib, subprocess
D = pathlib.Path(r"E:/Cena/crates/cena-model/data")
pats = []

def rx(p):
    if len(p.strip()) < 4:
        return None
    p = p.replace('(?<', '(?P<').replace('(?P<=', '(?<=').replace('(?P<!', '(?<!')
    p = p.replace('\\h', '[ \\t]')
    try:
        return re.compile(p)
    except Exception:
        return None

for line in (D / "spells.tsv").read_text(encoding="utf-8").split(chr(10)):
    if line.startswith('#') or line.startswith('number'):
        continue
    f = line.split('\t')
    for col, name in ((10, 'up'), (11, 'down'), (12, 'target')):
        if len(f) > col and f[col]:
            r = rx(f[col])
            if r:
                pats.append((f"spells.tsv {f[0]} {name}", r))
for tsv in ("combat_effects.tsv", "combat_results.tsv", "combat_attacks.tsv"):
    for i, line in enumerate((D / tsv).read_text(encoding="utf-8").split(chr(10)), 1):
        f = line.split('\t')
        if len(f) >= 7 and f[0] != 'family':
            r = rx(f[6])
            if r:
                pats.append((f"{tsv}:{i} {f[0]}/{f[1]}", r))
for i, line in enumerate((D / "creature_messages.tsv").read_text(encoding="utf-8").split(chr(10)), 1):
    f = line.split('\t')
    if len(f) >= 4 and i > 1:
        r = rx(f[3])
        if r:
            pats.append((f"creature_messages.tsv:{i} {f[0]}", r))

RS = subprocess.run(["grep", "-rhF", "--include=*.rs", "-e", "", r"E:/Cena/crates"],
                    capture_output=True, text=True, encoding="utf-8", errors="replace").stdout


def sample(fr):
    s = fr
    s = re.sub(r"^\^|\$$", "", s)
    s = re.sub(r"\(\?:([^()|]*)\|[^()]*\)\??", r"\1", s)
    s = re.sub(r"\(\?:([^()]*)\)\?", r"\1", s)
    s = re.sub(r"\(\.\*\??\)|\.\*\??|\.\+\??|\(\.\+\??\)", "XYZ", s)
    s = re.sub(r"\(\\d\+\)|\\d\+|\[0-9\]\+|\(\[0-9\]\+\)|\(\[\\d,\]\+\)", "5", s)
    s = re.sub(r"\\([.'!?()\[\]\-\"/,:])", r"\1", s)
    s = s.replace("\\s", " ")
    return s


lib = pathlib.Path(r"E:/Cena/reference/lich_repo_mirror/lib")
t = (lib / sys.argv[1]).read_bytes().decode("utf-8", "replace").splitlines()
lo = int(sys.argv[2]) if len(sys.argv) > 2 else 1
hi = int(sys.argv[3]) if len(sys.argv) > 3 else len(t)
REGEX_LIT = re.compile(r"=~\s*/((?:[^/\\]|\\.)+)/")
QUOTED = re.compile(r'(?:matchtimeout|matchwait|waitfor|dothistimeout)[^"]*"([^"]+)"')
for n in range(lo - 1, min(hi, len(t))):
    frs = REGEX_LIT.findall(t[n]) + QUOTED.findall(t[n])
    for fr in frs:
        for alt in [a for a in re.split(r"\|(?![^()]*\))", fr) if len(a) > 12]:
            s = sample(alt)
            hits = [name for name, r in pats if r.search(s) or r.search(s + ".") or r.search(s + "!")]
            if not hits and "XYZ" not in s:
                hits = [name for name, r in pats if s in re.sub(r"\\(.)", r"\1", r.pattern)]
            frag = max(re.split(r"XYZ", s), key=len).strip()[:40]
            rs = frag in RS if len(frag) > 10 else False
            v = hits[0] if hits else ("rs-literal" if rs else "NONE")
            print(f"{n+1}\t{v}\t{s[:120]}")
