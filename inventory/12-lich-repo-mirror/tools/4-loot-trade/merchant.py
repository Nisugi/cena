"""Pile-wide sweep for merchant, bank and container reply texts.

Pulls every regex literal (/.../) and quoted string from capture lines in the
pile's scripts, splits alternations, and keeps the fragments that mention a
merchant/bank/container keyword. Output: fragment<TAB>script:line, deduped by
fragment (first occurrence kept, count of scripts appended).
"""
import re
from collections import defaultdict
from pathlib import Path

SURVEY = Path(__file__).resolve().parent.parent
LIB = Path(r"E:/Cena/reference/lich_repo_mirror/lib")
OUT = SURVEY / "tmp-4-loot-trade"

KEYS = {
    "bank": r"teller|deposit|withdraw|account|bank|debt|note worth|add up to|balance",
    "shop": r"silvers?\b|coins\b|hands you|appraise|worth|offer|sell|buy|purchase|pawn|furrier|gem ?shop|jeweler|merchant|clerk|sold|order|afford|junk|worthless|interested|field|trash|quote|price|cost",
    "container": r"won't fit|is full|closed|nothing in there|you see|Inside|In the|capacity|can store|too heavy|encumbr|load|weigh|put your|You put|You place|You remove|You get|Get what|stow|rummage|portion|jar|contain",
}
LIT = re.compile(r"/((?:\\/|[^/\n])+)/[imxo]*|\"((?:\\\"|[^\"\n]){6,})\"|'((?:\\'|[^'\n]){6,})'")
CAP = re.compile(r"=~|!~|waitfor|matchtimeout|matchwait|match\b|when\s|Regexp|%r|dothis|\.scan|\.match|waitre|reget|DownstreamHook|server_string|line")


def main():
    rows = [l.split("\t") for l in (SURVEY / "pile-4-loot-trade.tsv").read_text(encoding="utf-8").splitlines()[1:]]
    found = {k: defaultdict(list) for k in KEYS}
    for name, *_ in rows:
        text = (LIB / name).read_bytes().decode("utf-8", "replace").splitlines()
        for i, line in enumerate(text, 1):
            s = line.strip()
            if s.startswith("#") or not CAP.search(s):
                continue
            for m in LIT.finditer(s):
                lit = next(g for g in m.groups() if g)
                for frag in re.split(r"(?<!\\)\|", lit):
                    frag = frag.strip().strip("^$").strip()
                    if len(frag) < 8 or "#{" in frag and len(frag) < 16:
                        continue
                    for k, rx in KEYS.items():
                        if re.search(rx, frag, re.I):
                            found[k][frag].append(f"{name}:{i}")
    for k, d in found.items():
        lines = []
        for frag, where in sorted(d.items(), key=lambda kv: (-len({w.split(':')[0] for w in kv[1]}), kv[0])):
            scripts = sorted({w.split(":")[0] for w in where})
            lines.append(f"{len(scripts)}\t{frag}\t{where[0]}\t{','.join(scripts[:6])}")
        (OUT / f"sweep_{k}.tsv").write_text("\n".join(lines), encoding="utf-8")
        print(k, len(lines))


if __name__ == "__main__":
    main()
