"""Per-script header and capture-line extraction for the 4-loot-trade pile.

Writes one text file per batch into tmp-4-loot-trade/caps/, each holding for
every script: its line count, the first comment block (trimmed), and every
line that captures game text (regex literals, waits, hooks) or holds a data
literal (quoted strings in arrays/hashes).
"""
import re
import sys
from pathlib import Path

SURVEY = Path(__file__).resolve().parent.parent
LIB = Path(r"E:/Cena/reference/lich_repo_mirror/lib")
OUT = SURVEY / "tmp-4-loot-trade" / "caps"
OUT.mkdir(parents=True, exist_ok=True)

CAP = re.compile(
    r"=~\s*/|!~\s*/|waitforre|waitfor\b|matchtimeout|matchwait|match\b|DownstreamHook|"
    r"when\s+/|Regexp\.new|%r\{|dothistimeout|dothis\b|\.scan\(/|\.match\(/|reget|/\)|\bwhen\s+\""
)


def header(lines):
    out = []
    in_block = False
    for i, ln in enumerate(lines[:80]):
        s = ln.rstrip()
        if s.startswith("=begin"):
            in_block = True
            continue
        if s.startswith("=end"):
            break
        if in_block or s.lstrip().startswith("#"):
            if s.strip():
                out.append(s.strip()[:160])
        elif out and not in_block:
            break
    return out[:18]


def main():
    rows = []
    for ln in (SURVEY / "pile-4-loot-trade.tsv").read_text(encoding="utf-8").splitlines()[1:]:
        name, lines, cap, data, api = ln.split("\t")
        rows.append((name, int(lines), int(cap), int(data), int(api)))
    lo, hi = int(sys.argv[1]), int(sys.argv[2])
    tag = sys.argv[3]
    buf = []
    for name, nlines, cap, data, api in rows[lo:hi]:
        path = LIB / name
        text = path.read_bytes().decode("utf-8", "replace")
        lines = text.splitlines()
        buf.append(f"\n########## {name} ({nlines} lines; cap {cap} data {data} api {api})")
        for h in header(lines):
            buf.append("  H| " + h)
        seen = set()
        for i, l in enumerate(lines, 1):
            s = l.strip()
            if not s or s.startswith("#"):
                continue
            if CAP.search(s):
                key = s[:220]
                if key in seen:
                    continue
                seen.add(key)
                buf.append(f"  {i}: {s[:300]}")
    (OUT / f"{tag}.txt").write_text("\n".join(buf), encoding="utf-8")
    print(len(buf), "lines written to", OUT / f"{tag}.txt")


if __name__ == "__main__":
    main()
