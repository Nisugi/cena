"""Which link shapes does the wire actually nest?

Answers one question: for every `<a>`/`<d>` in a corpus of wire logs, what
kinds of link appear INSIDE what other kinds. Written for `plan/20` step 6,
where the parser was found to drop an `<a exist=>` nested inside a
`<d cmd=>` -- and the immediate follow-up question was whether the mirror
case (a command inside an object) also occurs.

MEASURED 2026-09-20 over E:/Gemstone/dev/lich-5/logs (208 files):

           outer -> inner        count
             cmd -> exist          322

That is the whole table, at any depth. No `exist` inside `cmd`, no `<a>` in
`<a>`, no `<d>` in `<d>`.

WHY THIS AND NOT grep. Two `grep -P` passes with `(?:(?!</a>).)*?` lookaheads
returned 0 for BOTH directions, including the one that occurs 322 times. The
regex was failing, not the data, and its zero would have read as "this shape
does not occur" -- evidence of absence manufactured by a query that silently
did not run. A tag-stack walk cannot fail that way: it either parses the tags
or it does not.

Usage:  python link_nesting.py [corpus-dir]
"""

import collections
import glob
import os
import re
import sys

DEFAULT_ROOT = r"E:/Gemstone/dev/lich-5/logs"

TAG = re.compile(r"<(/?)(a|d)\b([^>]*)>")
EXIST = re.compile(r'\bexist="')
CMD = re.compile(r"\bcmd\s*=")


def kind_of(name, attrs):
    """What sort of link this tag opens, in the terms the parser cares about."""
    if name == "a" and EXIST.search(attrs):
        return "exist"
    if CMD.search(attrs):
        return "cmd"
    return "plain-" + name


def scan(path, counts, examples):
    """Walk one file, counting every (enclosing kind, enclosed kind) pair."""
    with open(path, encoding="utf-8", errors="replace") as handle:
        for lineno, line in enumerate(handle, 1):
            stack = []
            for match in TAG.finditer(line):
                closing, name, attrs = match.group(1), match.group(2), match.group(3)
                if closing:
                    # Tolerate unbalanced markup rather than aborting: a log
                    # line can be truncated, and a dropped close tag should
                    # not silently skew every later count on the line.
                    if stack and stack[-1][0] == name:
                        stack.pop()
                    continue
                if attrs.rstrip().endswith("/"):
                    continue  # self-closing: encloses nothing
                kind = kind_of(name, attrs)
                if stack:
                    key = (stack[-1][1], kind)
                    counts[key] += 1
                    examples.setdefault(
                        key, (os.path.basename(path), lineno, line.strip()[:170])
                    )
                stack.append((name, kind))


def main():
    root = sys.argv[1] if len(sys.argv) > 1 else DEFAULT_ROOT
    files = glob.glob(os.path.join(root, "**", "*.xml"), recursive=True)
    if not files:
        print(f"no .xml under {root}")
        return 1

    counts = collections.Counter()
    examples = {}
    for path in files:
        scan(path, counts, examples)

    print(f"files scanned: {len(files)}\n")
    print(f"{'outer':>12} -> {'inner':<12} {'count':>7}")
    for (outer, inner), total in counts.most_common():
        print(f"{outer:>12} -> {inner:<12} {total:>7}")
    if not counts:
        print("  (no nesting at all)")

    print("\n--- one example of each ---")
    for (outer, inner), (name, lineno, text) in sorted(examples.items()):
        print(f"\n{outer} -> {inner}   ({name}:{lineno})\n  {text}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
