"""Convert Lich's data/gameobj-data.xml into the TSV this crate ships.

    python crates/cena-model/tools/convert_gameobj_data.py \
        C:/Gemstone/lich-5/data/gameobj-data.xml \
        crates/cena-model/data/gameobj-data.tsv

The XML is two kinds of block, both direct children of <data> and both
holding the same four regex fields:

    <type name="gem">       <name>..</name> <noun>..</noun> <exclude>..</exclude>
    <sellable name="furrier"> ..            <suffix>..</suffix>

<sellable> is a SIBLING of <type>, not a child. Walking only <type> silently
drops 11 rows, which is why the output carries a `kind` column and why this
script asserts the block tag is one of the two it knows.

One pattern is edited rather than transcribed. Lich's jewelry <name> is
`^(^(?!some).*\bplate$)$`; the Rust `regex` crate rejects the negative
lookahead. The negation is folded into jewelry's existing <exclude> as a
leading `some.*` alternative, leaving a name/exclude pair the loader applies
as match-then-veto -- the shape `crit/match_index.rs` already uses for the one
lookahead in the crit tables. Note the absent word boundary: `(?!some)`
rejects `somersault plate` too, so the folded alternative is `some.*`, not
`some\b.*`. tests/gameobj_patterns.rs checks the equivalence.
"""

import sys
import xml.etree.ElementTree as ET
from collections import Counter

LOOKAHEAD = r"^(^(?!some).*\bplate$)$"
KINDS = ("type", "sellable")
FIELDS = ("name", "noun", "exclude", "suffix")


def convert(src: str, dst: str) -> None:
    root = ET.parse(src).getroot()

    pairs = []
    for block in root:
        assert block.tag in KINDS, f"unknown block <{block.tag}>"
        for child in block:
            assert child.tag in FIELDS, f"unknown field <{child.tag}>"
            assert len(child) == 0, f"unexpected nesting under <{child.tag}>"
            pairs.append(((block.tag, block.get("name"), child.tag), (child.text or "").strip()))

    # A duplicate key would let one row overwrite another unnoticed.
    dupes = [k for k, n in Counter(k for k, _ in pairs).items() if n > 1]
    assert not dupes, f"duplicate keys: {dupes}"

    rows = []
    for (kind, category, field), value in pairs:
        assert not set("\t\n\r") & set(value), f"{kind} {category}/{field}: whitespace breaks TSV"
        if value == LOOKAHEAD:
            assert (kind, category, field) == ("type", "jewelry", "name")
            value = r"^.*\bplate$"
        elif (kind, category, field) == ("type", "jewelry", "exclude"):
            assert value.startswith("^(") and value.endswith(")$")
            value = "^(some.*|" + value[2:]
        rows.append((kind, category, field, value))

    with open(dst, "w", encoding="utf-8", newline="") as f:
        f.write("kind\tcategory\tfield\tpattern\n")
        for row in rows:
            f.write("\t".join(row) + "\n")

    print(f"{len(rows)} rows: {dict(Counter(r[0] for r in rows))} {dict(Counter(r[2] for r in rows))}")


if __name__ == "__main__":
    if len(sys.argv) != 3:
        sys.exit(__doc__)
    convert(sys.argv[1], sys.argv[2])
