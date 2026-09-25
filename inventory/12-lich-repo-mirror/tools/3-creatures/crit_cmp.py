"""Compare crittracker.lic TABLES (crush/puncture/slash from gswiki) against Cena crit_tables.tsv."""
import re
from pathlib import Path

src = Path(r"E:/Cena/reference/lich_repo_mirror/lib/crittracker.lic").read_bytes().decode("utf-8", "replace").splitlines()
DATA = Path(r"E:/Cena/crates/cena-model/data/crit_tables.tsv").read_bytes().decode("utf-8").splitlines()
hdr = DATA[0].split("\t")
cena = [dict(zip(hdr, l.split("\t"))) for l in DATA[1:] if l]

rows = []
typ = loc = None
for i, l in enumerate(src, 1):
    m = re.match(r'^\s*"(CRUSH|PUNCTURE|SLASH)" => \{', l)
    if m:
        typ = m.group(1).lower(); continue
    m = re.match(r'^\s*"([A-Z ]+)" => \[', l)
    if m and typ:
        loc = m.group(1).lower().replace(" ", "_"); continue
    m = re.match(r'^\s*\[(\d+),\s*(\d+),\s*"(.*)",\s*"([^"]*)",\s*"([^"]*)"\],', l)
    if m and typ and loc:
        rows.append(dict(line=i, type=typ, loc=loc, rank=int(m.group(1)), dmg=int(m.group(2)), msg=m.group(3), st=m.group(4), wound=m.group(5)))

print("crittracker rows", len(rows))
def cena_match(r):
    msg = r["msg"].replace("[target]", "the troll")
    hits = []
    for c in cena:
        if c["type"] != r["type"]:
            continue
        try:
            if re.search(c["pattern"], msg, re.I):
                hits.append(c)
        except re.error:
            pass
    return hits

nomatch, dmgdiff, stundiff, fataldiff = [], [], [], []
for r in rows:
    hits = cena_match(r)
    if not hits:
        nomatch.append(r); continue
    same = [c for c in hits if c["rank"] == str(r["rank"])] or hits
    c = same[0]
    if c["damage"] != str(r["dmg"]):
        dmgdiff.append((r["line"], r["type"], r["loc"], r["rank"], r["dmg"], c["location"], c["rank"], c["damage"], r["msg"][:60]))
    stun = re.search(r"S(\d+)", r["st"])
    if stun and c["stunned"] != stun.group(1):
        stundiff.append((r["line"], r["type"], r["loc"], r["rank"], r["st"], c["stunned"], r["msg"][:60]))
    f = "F" in r["st"].split() or r["st"] == "F" or re.search(r"\bF\b", r["st"])
    if bool(f) != (c["fatal"] == "1"):
        fataldiff.append((r["line"], r["type"], r["loc"], r["rank"], r["st"], c["fatal"], r["msg"][:60]))
print("no Cena pattern matches:", len(nomatch))
for r in nomatch: print("  NOMATCH", r["line"], r["type"], r["loc"], r["rank"], r["msg"][:90])
print("damage differs:", len(dmgdiff))
for d in dmgdiff: print("  DMG", d)
print("stun differs:", len(stundiff))
for d in stundiff: print("  STUN", d)
print("fatal differs:", len(fataldiff))
for d in fataldiff: print("  FATAL", d)
