# Re-check every absence claim in 1-hunting-engines.md with Python (not grep).
from pathlib import Path
frags = ["tried to join your group", "group status is closed", "hold your hand",
 "sandstorm", "sealed fissure", "gas cloud", "air whooshes", "Suddenly you feel sick",
 "aim that high", "already missing that", "does not have a head", "find an opening for your strike",
 "no need to fight here", "spells of war", "searing pain in your throat", "must be the creature that you",
 "The spirits distract", "Your thoughts scatter", "angered beyond all reason", "think clearly enough to prepare",
 "knocked from your grasp", "You cannot attack with", "swing a closed fist", "sprouting scales",
 "Maximum Mana Points", "Mana gained off node", "renewal cost is", "is around here somewhere",
 "recover your hurled", "flies back to your waiting hand", "You spy ", "has no effect",
 "Killing ", "should do nicely", "unable to follow you", "no valid target"]
roots = {"crates": (Path(r"E:/Cena/crates"), (".rs", ".tsv")), "lich": (Path(r"E:/Cena/reference/lich-5/lib"), (".rb",))}
for name,(root,exts) in roots.items():
    files=[p for p in root.rglob("*") if p.suffix in exts and "target" not in p.parts]
    texts={p:p.read_bytes().decode("utf-8","replace").lower() for p in files}
    print("==",name,len(files),"files")
    for f in frags:
        hits=[str(p.relative_to(root)) for p,t in texts.items() if f.lower() in t]
        print(f"{f!r}: {len(hits)} {hits[:4]}")
