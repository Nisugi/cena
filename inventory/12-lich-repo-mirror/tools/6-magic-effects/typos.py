# Scan spells.tsv message patterns for shapes that cannot match a one-sentence game line:
# an unescaped '.' immediately followed by an escaped '\.' at the end, a doubled '\.\.',
# and patterns that do not compile.
import re, pathlib
p = pathlib.Path(r"E:/Cena/crates/cena-model/data/spells.tsv")
for line in p.read_text(encoding="utf-8").split(chr(10)):
    if line.startswith('#') or line.startswith('number') or not line.strip():
        continue
    f = line.split('\t')
    for col, name in ((10, 'up'), (11, 'down'), (12, 'target')):
        if len(f) <= col or not f[col]:
            continue
        m = f[col]
        flags = []
        if re.search(r"(?<!\\)\.\\\.$", m):
            flags.append("dot-then-escaped-dot at end")
        if re.search(r"\\\.\\\.$", m):
            flags.append("doubled escaped dot")
        try:
            re.compile(m.replace('(?<', '(?P<').replace('(?P<=', '(?<=').replace('(?P<!', '(?<!'))
        except Exception as e:
            flags.append(f"does not compile: {e}")
        if flags:
            print(f"{f[0]}\t{f[1]}\t{name}\t{'; '.join(flags)}\t{m[:120]}")
