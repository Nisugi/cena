"""Compare energywings.lic's CREATURE_SPELL_PREPS_WING_PIN (KSwole's creature spell-prep list)
against Cena's creature_messages.tsv spell_prep rows (and every other kind)."""
import re, sys
from pathlib import Path

script = sys.argv[1] if len(sys.argv) > 1 else "energywings.lic"
src = Path(r"E:/Cena/reference/lich_repo_mirror/lib", script).read_bytes().decode("utf-8", "replace").splitlines()
rows = [l.split("\t") for l in Path(r"E:/Cena/crates/cena-model/data/creature_messages.tsv").read_bytes().decode("utf-8").splitlines()[1:]]

def fill(t):
    return (t.replace("{pronoun}", "her").replace("{Pronoun}", "Her").replace("{target}", "you")
             .replace("{direction}", "north"))

texts_prep = [fill(r[3]) for r in rows if len(r) > 3 and r[1] == "spell_prep"]
texts_all = [(r[0], r[1], fill(r[3])) for r in rows if len(r) > 3]

inside = False
pats = []
area = ""
for i, l in enumerate(src, 1):
    if "CREATURE_SPELL_PREPS" in l and "Regexp.union" in l:
        inside = True; continue
    if inside:
        s = l.strip()
        if s.startswith(")"):
            break
        if s.startswith("#"):
            area = s.lstrip("# ").strip(); continue
        m = re.match(r"^/(.*)/[imx]*,?$", s)
        if m:
            pats.append((i, area, m.group(1)))

print("patterns", len(pats), "cena spell_prep rows", len(texts_prep))
hit_prep = hit_other = miss = 0
misses = []
for i, area, p in pats:
    py = p.replace("(?<", "(?P<").replace("\\/", "/")
    try:
        rx = re.compile(py)
    except re.error as e:
        print("  BADRX", i, p, e); continue
    if any(rx.search(t) for t in texts_prep):
        hit_prep += 1
    else:
        other = [(c, k) for c, k, t in texts_all if rx.search(t)]
        if other:
            hit_other += 1
            misses.append((i, area, p, "matches other kind: %s" % sorted(set(k for c, k in other))))
        else:
            miss += 1
            misses.append((i, area, p, "NO MATCH"))
print("match a Cena spell_prep:", hit_prep, " match only another kind:", hit_other, " match nothing:", miss)
for m in misses:
    print("  %s:%d [%s] %s -- %s" % (script, m[0], m[1], m[2][:110], m[3]))
