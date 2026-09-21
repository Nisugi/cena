"""Cut the unported worklist into slices for parallel porting. plan/21 step 7.

Shapes are sorted by their normalised text, so spellings of one family land in
the same slice, then cut into contiguous runs of roughly equal shape count.
Costs are one slice of their own. Shapes reserved for routines are left out.

usage: python slice_tail.py <conversion-dir> <out-dir> [crossing-slices] [letters]
"""
import csv
import os
import sys

root, out = sys.argv[1], sys.argv[2]
parts = int(sys.argv[3]) if len(sys.argv) > 3 else 3
# One letter per crossing slice, then one for the costs slice.
letters = sys.argv[4] if len(sys.argv) > 4 else "abcd"

# Kept by the main session: searches, tables and scripts with side effects.
RESERVED = (
    "force_go",              # errands that start a second trip
    "day_pass",              # finds, buys and reads a pass: side effects
    "$mapdb_confluence",     # the Confluence's own search, a room to itself
    "$mapdb_seeking",        # the seeking script, a room to itself
)


def rows(name):
    with open(os.path.join(root, "report", name), encoding="utf-8", newline="") as f:
        return list(csv.DictReader(f, delimiter="\t"))


def write(name, chosen):
    with open(os.path.join(out, name), "w", encoding="utf-8", newline="\n") as f:
        f.write("edges\tshape_id\tsample\tshape\n")
        for r in chosen:
            f.write(f"{r['edges']}\t{r['shape_id']}\t{r['sample']}\t{r['shape']}\n")
    print(name, len(chosen), "shapes", sum(int(r["edges"]) for r in chosen), "exits")


os.makedirs(out, exist_ok=True)
crossings = [r for r in rows("unported_crossings.tsv") if not any(k in r["shape"] for k in RESERVED)]
crossings.sort(key=lambda r: r["shape"])
size = -(-len(crossings) // parts)
for i in range(parts):
    write(f"slice_{letters[i]}.tsv", crossings[i * size:(i + 1) * size])
costs = [r for r in rows("unported_costs.tsv") if not any(k in r["shape"] for k in RESERVED)]
write(f"slice_{letters[parts]}_costs.tsv", sorted(costs, key=lambda r: r["shape"]))
reserved = [r for r in rows("unported_crossings.tsv") if any(k in r["shape"] for k in RESERVED)]
write("reserved_for_routines.tsv", reserved)
