import re, pathlib, sys
lib = pathlib.Path(r"E:/Cena/reference/lich_repo_mirror/lib")
pile = [r.split("\t")[0] for r in pathlib.Path(r"../pile-8-bounty-events-travel.tsv").read_text(encoding="utf-8").splitlines()[1:]]
names = set(pathlib.Path("bounty_all.txt").read_text().split())
kw = re.compile(r"tasked|assign|succeeded|task|bounty|escort|heirloom|child|rescue|gem dealer|furrier|herbalist|healer|provoke|bandit|guard|taskmaster|voucher|Come back|locate|concoction|suppress|territory|divinist|client|samples|pristine|skins? |dangerous|reassign|expedite|removed you|boost", re.I)
rx = re.compile(r"(/(?:[^/\\\n]|\\.){12,}/[imx]*)")
out = []
for n in pile:
    if n not in names:
        continue
    txt = (lib / n).read_bytes().decode("utf-8", "replace").splitlines()
    for i, l in enumerate(txt):
        if l.strip().startswith("#"):
            continue
        for m in rx.finditer(l):
            s = m.group(1)
            if kw.search(s):
                out.append(f"{n}:{i+1}\t{s[:320]}")
pathlib.Path("bounty_frags.tsv").write_text("\n".join(out), encoding="utf-8")
print(len(out))
