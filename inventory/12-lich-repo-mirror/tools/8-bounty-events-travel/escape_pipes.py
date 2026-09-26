import pathlib
p = pathlib.Path(r"../8-bounty-events-travel.md")
out = []
changed = 0
for line in p.read_text(encoding="utf-8").split("\n"):
    if not line.startswith("|"):
        out.append(line)
        continue
    buf = []
    in_code = False
    prev = ""
    for ch in line:
        if ch == "`":
            in_code = not in_code
            buf.append(ch)
        elif ch == "|" and in_code and prev != "\\":
            buf.append("\\|")
            changed += 1
        else:
            buf.append(ch)
        prev = ch
    out.append("".join(buf))
p.write_text("\n".join(out), encoding="utf-8")
print("escaped", changed)
