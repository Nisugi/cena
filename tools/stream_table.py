#!/usr/bin/env python3
"""Extract the ifClosed routing table from a captured session.

The wiki (`reference/wiki_clean/Wrayth protocol.txt` :73-80) documents four
closed-window routing behaviours. The server DECLARES each stream's behaviour
in a `<streamWindow>` tag, so one session with every window open yields the
table as evidence rather than inference.

Usage:  python tools/stream_table.py <capture.xml> [more.xml ...]

Reads only the files named on the command line -- never walks the archive.
"""
import re
import sys
from collections import defaultdict

TAG = re.compile(r"<streamWindow\b([^>]*)>")
ATTR = re.compile(r"""(\w+)\s*=\s*(?:'([^']*)'|"([^"]*)")""")


def behaviour(attrs):
    """Classify per wiki :76-79."""
    has_if = "ifClosed" in attrs
    has_style = "styleIfClosed" in attrs
    dest = attrs.get("ifClosed", "")
    if has_if and dest:
        return f"Routed -> {dest}"
    if has_if and not dest:
        return "Copy (also sent to main)"
    if has_style:
        return f"Exclusive (styled {attrs['styleIfClosed']!r})"
    return "Unspecified (falls to main, unstyled)"


def main(paths):
    seen = defaultdict(lambda: defaultdict(int))
    bare = 0
    for path in paths:
        with open(path, "r", encoding="utf-8", errors="replace") as handle:
            for line in handle:
                for raw in TAG.findall(line):
                    # A bare `ifClosed=` (no quotes) is the wiki's own spelling
                    # and is invisible to the parser; count it separately so we
                    # know whether the live server ever uses that form.
                    if re.search(r"\bifClosed\s*=\s*(?![\"'])", raw):
                        bare += 1
                    attrs = {
                        k: (q1 or q2) for k, q1, q2 in ATTR.findall(raw)
                    }
                    sid = attrs.get("id")
                    if sid:
                        seen[sid][behaviour(attrs)] += 1

    if not seen:
        print("No <streamWindow> declarations found.")
        return

    print(f"{'stream':<18} {'n':>6}  behaviour")
    print("-" * 72)
    for sid in sorted(seen):
        for how, n in sorted(seen[sid].items(), key=lambda kv: -kv[1]):
            print(f"{sid:<18} {n:>6}  {how}")

    print(f"\n{len(seen)} distinct streams.")
    print(f"bare `ifClosed=` (unquoted, unparseable today): {bare}")
    conflicts = {s: dict(v) for s, v in seen.items() if len(v) > 1}
    if conflicts:
        print("\nDECLARED MORE THAN ONE WAY -- worth a look:")
        for sid, how in conflicts.items():
            print(f"  {sid}: {how}")


if __name__ == "__main__":
    if len(sys.argv) < 2:
        print(__doc__)
        sys.exit(2)
    main(sys.argv[1:])
