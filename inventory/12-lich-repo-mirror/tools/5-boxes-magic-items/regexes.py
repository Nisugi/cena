import re, sys
from pathlib import Path

LIB = Path(r"E:/Cena/reference/lich_repo_mirror/lib")

CAP = re.compile(r"(?:=~|when|waitforre|matchtimeout|dothistimeout[^/]*|Regexp\.new\(|match\(|\bscan\(|,)\s*/((?:[^/\\\n]|\\.){8,})/")
PCT = re.compile(r"%r\{((?:[^}\\]|\\.){8,})\}")


def rx(name):
    p = LIB / f"{name}.lic"
    if not p.exists():
        p = Path(name)
    t = p.read_bytes().decode("utf-8", "replace")
    out = []
    for i, line in enumerate(t.splitlines(), 1):
        for m in CAP.finditer(line):
            out.append((i, m.group(1)))
        for m in PCT.finditer(line):
            out.append((i, m.group(1)))
    return out


def norm(s):
    s = re.sub(r"\\(.)", r"\1", s)
    s = re.sub(r"[^a-z ]+", " ", s.lower())
    return " ".join(w for w in s.split() if len(w) > 3)


if __name__ == "__main__":
    base = set()
    for b in sys.argv[1].split(","):
        if b:
            for _, r in rx(b):
                for part in r.split("|"):
                    base.add(norm(part))
    for name in sys.argv[2:]:
        rows = rx(name)
        new = []
        for i, r in rows:
            parts = [p for p in r.split("|") if len(norm(p)) >= 12]
            if not parts:
                continue
            if all(norm(p) in base for p in parts):
                continue
            new.append((i, r))
        print(f"== {name}: {len(rows)} regexes, {len(new)} not in base")
        for i, r in new:
            print(f"  {i}: {r[:230]}")
