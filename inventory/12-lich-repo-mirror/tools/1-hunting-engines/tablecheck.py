import re
from pathlib import Path

PIPE = re.compile(r"(?<!\\)\|")
path = Path(r"C:\Users\shawn\AppData\Local\Temp\claude\e--Cena\a4e2b7d3-05c6-411f-9da3-d545e41fa29c\scratchpad\survey\1-hunting-engines.md")
lines = path.read_bytes().decode("utf-8").split("\n")
cols = None
hdr = 0
bad = 0
for i, line in enumerate(lines, 1):
    if line.startswith("|"):
        n = len(PIPE.findall(line))
        if cols is None:
            cols, hdr = n, i
        elif n != cols:
            bad += 1
            print(f"line {i}: {n} pipes, header line {hdr} has {cols}: {line[:100]}")
    else:
        cols = None
print("rows out of shape:", bad)
