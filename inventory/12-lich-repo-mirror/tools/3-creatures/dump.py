"""Dump header + capture lines of each named script into tmp-3-creatures/dump/<script>.txt."""
import re, sys
from pathlib import Path

LIB = Path(r"E:/Cena/reference/lich_repo_mirror/lib")
OUT = Path(__file__).parent / "dump"
OUT.mkdir(exist_ok=True)
CAP = re.compile(r"=~ */|!~ */|waitforre|matchtimeout|matchwait|matchfind|DownstreamHook|when +/|Regexp\.new|%r\{|\.match\(|waitfor |/\w[^/]{8,}/[imx]*")

for name in sys.argv[1:]:
    p = LIB / name
    text = p.read_bytes().decode("utf-8", "replace").splitlines()
    out = []
    out.append(f"#### {name} ({len(text)} lines)")
    # header: first 45 lines or =begin..=end
    hdr_end = 45
    for i, l in enumerate(text[:200]):
        if l.strip().startswith("=end"):
            hdr_end = min(i + 1, 80)
            break
    out += [f"H{i+1}: {l}" for i, l in enumerate(text[:hdr_end])]
    for i, l in enumerate(text, 1):
        if CAP.search(l) and not l.strip().startswith("#"):
            out.append(f"{i}: {l.strip()[:300]}")
    (OUT / (name + ".txt")).write_text("\n".join(out), encoding="utf-8")
    print(name, len(out))
