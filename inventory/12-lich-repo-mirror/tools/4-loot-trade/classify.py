"""Classify item names with Cena's gameobj-data.tsv, the way gameobj.rs does.

A category matches when any name pattern matches the name or any noun
pattern matches the noun, and no exclude pattern matches the name.
`suffix` rows are skipped, as gameobj.rs skips them.

usage: python classify.py names.txt   (one name per line; noun = last word,
       or give "name<TAB>noun" to set the noun)
Prints: name, types, sellable.
"""
import re
import sys
from pathlib import Path

TSV = Path(r"E:/Cena/crates/cena-model/data/gameobj-data.tsv")


def load():
    cats = {}
    for line in TSV.read_text(encoding="utf-8").splitlines()[1:]:
        if not line:
            continue
        kind, name, field, pattern = line.split("\t", 3)
        if field == "suffix":
            continue
        try:
            rx = re.compile(pattern)
        except re.error:
            continue
        c = cats.setdefault((kind, name), {"name": [], "noun": [], "exclude": []})
        c[field].append(rx)
    return cats


def classify(cats, name, noun):
    types, sell = [], []
    for (kind, cname), c in cats.items():
        hit = any(r.search(name) for r in c["name"]) or any(r.search(noun) for r in c["noun"])
        if hit and not any(r.search(name) for r in c["exclude"]):
            (types if kind == "type" else sell).append(cname)
    return types, sell


def main():
    cats = load()
    for raw in Path(sys.argv[1]).read_text(encoding="utf-8").splitlines():
        raw = raw.strip()
        if not raw:
            continue
        if "\t" in raw:
            name, noun = raw.split("\t", 1)
        else:
            name, noun = raw, raw.split()[-1]
        t, s = classify(cats, name, noun)
        print(f"{name}\t{','.join(t) or '-'}\t{','.join(s) or '-'}")


if __name__ == "__main__":
    main()
