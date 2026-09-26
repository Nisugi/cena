import pathlib, re
t = pathlib.Path(r"../8-bounty-events-travel.md").read_text(encoding="utf-8").split("\n")
pipe = re.compile(r"(?<!\\)\|")
hdr = None
bad = 0
for i, l in enumerate(t):
    if l.startswith("|"):
        n = len(pipe.findall(l))
        if hdr is None:
            hdr = n
        elif n != hdr:
            bad += 1
            print(i + 1, n, hdr, l[:100])
    else:
        hdr = None
print("bad rows", bad)
