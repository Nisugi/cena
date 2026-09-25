"""Compare crit message tables in four lich_repo_mirror scripts against Cena's crit_tables.tsv.

For each script entry: synthesize a sample line from its regex, run every Cena pattern
against it (Python re), and compare type/location/rank/damage/stun/fatal of the best hit.
"""
import re, sys, json, collections
from pathlib import Path

LIB = Path("E:/Cena/reference/lich_repo_mirror/lib")
TSV = Path("E:/Cena/crates/cena-model/data/crit_tables.tsv")
OUT = Path(__file__).parent

def read(p):
    return p.read_bytes().decode("utf-8", "replace")

# ---------------- Cena
cena = []
lines = read(TSV).splitlines()
hdr = lines[0].split("\t")
for i, l in enumerate(lines[1:], start=2):
    f = dict(zip(hdr, l.split("\t")))
    pat = f["pattern"]
    pp = re.sub(r"\(\?<(?![=!])", "(?P<", pat)
    try:
        rx = re.compile(pp)
    except re.error as e:
        rx = None
    f["rx"] = rx
    f["tsvline"] = i
    cena.append(f)

# ---------------- sample synthesis
def synth(rx):
    s = rx
    s = s.strip()
    s = s.lstrip("^").rstrip("$")
    s = s.replace("\\b", "")
    s = re.sub(r"\(\?:([^|()]+)\|[^()]*\)", r"\1", s)
    s = re.sub(r"\(([^|()?]+)\|[^()]*\)", r"\1", s)
    s = re.sub(r"\.\*\?|\.\+\?|\.\*|\.\+", "kobold", s)
    s = s.replace("\\s+", " ").replace("\\s", " ").replace("\\d+", "5")
    s = re.sub(r"\\(.)", r"\1", s)
    return s

TYPES = ["UAC-GRAPPLE", "UAC-JAB", "UAC-KICK", "UAC-PUNCH", "NON-CORP", "DISINTEGRATE", "DISRUPTION",
         "UNBALANCE", "LIGHTNING", "PLASMA", "IMPACT", "GRAPPLE", "VACUUM", "VACCUM", "STEAM", "ACID",
         "COLD", "FIRE", "CRUSH", "PUNCTURE", "SLASH"]
TYPEMAP = {"UAC-GRAPPLE": "ucs_grapple", "UAC-JAB": "ucs_jab", "UAC-KICK": "ucs_kick", "UAC-PUNCH": "ucs_punch",
           "NON-CORP": "non_corporeal", "VACCUM": "vacuum"}

def norm_type(t):
    t = t.upper()
    return TYPEMAP.get(t, t.lower())

def norm_loc(l):
    l = l.strip().lower().replace(" ", "_")
    return l

def parse_label(label):
    """RANK n LOC TYPE: Damage: N Status Effect: X Wounds: Y  (many spacing/comma variants)."""
    m = re.match(r"\s*RANK\s+(\d+)\s*,?\s*(.*)", label, re.I)
    if not m:
        return None
    rank = int(m.group(1))
    rest = m.group(2)
    typ = None
    for t in TYPES:
        mm = re.search(r"(^|[\s,])" + re.escape(t) + r"([\s,:]|$)", rest, re.I)
        if mm:
            typ = t
            loc = rest[: mm.start()].strip(" ,")
            tail = rest[mm.end():]
            break
    if typ is None:
        return None
    dm = re.search(r"Damage:?\s*(\d+)", tail, re.I)
    dmg = int(dm.group(1)) if dm else None
    st = re.search(r"Status Effect:\s*(.*?)(,?\s*Wounds|$)", tail, re.I)
    stat = st.group(1).strip(" ,") if st else ""
    wd = re.search(r"Wounds:\s*(.*)$", tail, re.I)
    wound = wd.group(1).strip() if wd else ""
    return dict(type=norm_type(typ), loc=norm_loc(loc), rank=rank, damage=dmg, stat=stat, wound=wound)

def stun_of(stat):
    m = re.search(r"Stun(?:ned)?\s*(\d+)", stat, re.I)
    return int(m.group(1)) if m else (0 if not re.search(r"stun", stat, re.I) else None)

def fatal_of(stat):
    return bool(re.search(r"fatal|^F$", stat.strip(), re.I))

entries = collections.defaultdict(list)

# crit_type_tracker: /re/ => "label",
src = read(LIB / "crit_type_tracker.lic").splitlines()
for n, l in enumerate(src, 1):
    m = re.match(r"\s*/(.*)/[imx]*\s*=>\s*\"(.*)\",?\s*$", l)
    if m:
        lab = parse_label(m.group(2))
        entries["crit_type_tracker"].append(dict(line=n, rx=m.group(1), label=m.group(2), p=lab))

# ctt_win: if/elsif line =~ /re/  then next line cttmsg("label")
src = read(LIB / "ctt_win.lic").splitlines()
for n, l in enumerate(src, 1):
    m = re.match(r"\s*(?:if|elsif)\s+line\s*=~\s*/(.*)/[imx]*\s*$", l)
    if m and n < len(src):
        mm = re.search(r'cttmsg\("(.*)"\)', src[n])
        if mm:
            entries["ctt_win"].append(dict(line=n, rx=m.group(1), label=mm.group(1), p=parse_label(mm.group(1))))

# crit-tracking: 're' => { :r => 0, :d => 0, :l => "Head", :t => "Crush", :stat => "None", :wound => "None" },
src = read(LIB / "crit-tracking.lic").splitlines()
for n, l in enumerate(src, 1):
    m = re.match(r"\s*'(.*)'\s*=>\s*\{\s*:r\s*=>\s*(\d+),\s*:d\s*=>\s*(\d+),\s*:l\s*=>\s*\"([^\"]*)\",\s*:t\s*=>\s*\"([^\"]*)\",\s*:stat\s*=>\s*\"([^\"]*)\",\s*:wound\s*=>\s*\"([^\"]*)\"", l)
    if m:
        p = dict(type=norm_type(m.group(5)), loc=norm_loc(m.group(4)), rank=int(m.group(2)), damage=int(m.group(3)), stat=m.group(6), wound=m.group(7))
        entries["crit-tracking"].append(dict(line=n, rx=m.group(1).replace("\\'", "'"), label="", p=p))

# crittracker: "TYPE" => { "LOC" => [ [rank, dmg, "msg", "stat", "wound"], ...
src = read(LIB / "crittracker.lic").splitlines()
ctype = cloc = None
for n, l in enumerate(src, 1):
    m = re.match(r'\s*"(CRUSH|PUNCTURE|SLASH)"\s*=>\s*\{', l)
    if m:
        ctype = m.group(1)
        continue
    m = re.match(r'\s*"([A-Z ]+)"\s*=>\s*\[', l)
    if m and ctype:
        cloc = m.group(1)
        continue
    m = re.match(r'\s*\[(\d+),\s*(\d+),\s*"(.*)",\s*"([^"]*)",\s*"([^"]*)"\],?\s*$', l)
    if m and ctype and cloc:
        stat = m.group(4)
        stat2 = re.sub(r"\bS(\d+)", r"Stun \1", stat)
        stat2 = re.sub(r"^F$", "Fatal", stat2)
        p = dict(type=norm_type(ctype), loc=norm_loc(cloc), rank=int(m.group(1)), damage=int(m.group(2)), stat=stat2, wound=m.group(5))
        rx = re.escape(m.group(3).replace("[target]", "\x00")).replace("\x00", ".*?")
        entries["crittracker"].append(dict(line=n, rx=rx, label=m.group(3), p=p, literal=m.group(3).replace("[target]", "the kobold")))

def locs_equiv(a, b):
    if a == b:
        return True
    # scripts sometimes collapse sides
    if a in ("arm", "leg", "hand", "eye", "foot"):
        return b.endswith("_" + a)
    if b in ("arm", "leg", "hand", "eye"):
        return a.endswith("_" + b)
    return False

report = {}
for script, es in entries.items():
    rows = []
    for e in es:
        sample = e.get("literal") or synth(e["rx"])
        hits = [c for c in cena if c["rx"] is not None and c["rx"].search(sample)]
        hits.sort(key=lambda c: -len(c["pattern"]))
        p = e["p"]
        status = None
        diffs = []
        best = None
        if not hits:
            status = "MISSING"
        else:
            # prefer a hit with the same type and location
            same = [c for c in hits if p and c["type"] == p["type"] and locs_equiv(p["loc"], c["location"])]
            best = (same or [c for c in hits if p and c["type"] == p["type"]] or hits)[0]
            if p:
                if best["type"] != p["type"]:
                    diffs.append(f"type {p['type']} vs cena {best['type']}")
                if not locs_equiv(p["loc"], best["location"]):
                    diffs.append(f"loc {p['loc']} vs cena {best['location']}")
                if int(best["rank"]) != p["rank"]:
                    diffs.append(f"rank {p['rank']} vs cena {best['rank']}")
                if p["damage"] is not None and int(best["damage"]) != p["damage"]:
                    diffs.append(f"damage {p['damage']} vs cena {best['damage']}")
                s = stun_of(p["stat"])
                cs = int(best["stunned"]) if best["stunned"].lstrip("-").isdigit() else None
                if s is not None and cs is not None and s != cs and not fatal_of(p["stat"]):
                    diffs.append(f"stun {s} vs cena {cs}")
                if fatal_of(p["stat"]) != (best["fatal"] == "1"):
                    diffs.append(f"fatal {fatal_of(p['stat'])} vs cena {best['fatal']=='1'}")
            status = "DIFF" if diffs else "SAME"
        rows.append(dict(script=script, line=e["line"], rx=e["rx"], sample=sample, parsed=p, status=status, diffs=diffs,
                         cena=(dict(tsvline=best["tsvline"], type=best["type"], loc=best["location"], rank=best["rank"], damage=best["damage"], stunned=best["stunned"], fatal=best["fatal"], pattern=best["pattern"]) if best else None)))
    report[script] = rows

(OUT / "critcmp.json").write_text(json.dumps(report, indent=1), encoding="utf-8")
for script, rows in report.items():
    c = collections.Counter(r["status"] for r in rows)
    unparsed = sum(1 for r in rows if r["parsed"] is None)
    print(script, len(rows), dict(c), "unparsed-labels", unparsed)
bad = [c for c in cena if c["rx"] is None]
print("cena patterns not compiled by python re:", len(bad), [b["tsvline"] for b in bad][:10])
