import re, pathlib, csv
SURVEY = pathlib.Path(r"C:/Users/shawn/AppData/Local/Temp/claude/e--Cena/a4e2b7d3-05c6-411f-9da3-d545e41fa29c/scratchpad/survey")
LIB = pathlib.Path(r"E:/Cena/reference/lich_repo_mirror/lib")
OUT = SURVEY / "tmp-7-character-healing-crafting" / "dumps"
OUT.mkdir(exist_ok=True)
CAP = re.compile(r"=~ */|!~ */|waitforre|matchtimeout|matchwait|matchfind|waitfor|DownstreamHook|when +/|Regexp.new|%r\{|dothistimeout|dothis |reget|\.match\(|\.scan\(|issue_command|quiet_command")
HDR = re.compile(r"author|version|date|tags|name:|required|license|copyright|purpose|description|#\s*v\d", re.I)
rows = list(csv.reader(open(SURVEY / "pile-7-character-healing-crafting.tsv", encoding="utf-8"), delimiter="\t"))[1:]
for script, lines, cap, data, api in rows:
    if int(cap) + int(data) < 5: continue
    p = LIB / script
    txt = p.read_bytes().decode("utf-8", "replace").splitlines()
    out = [f"### {script} lines={lines} cap={cap} data={data} api={api}"]
    out.append("--- head")
    for i, l in enumerate(txt[:60], 1):
        if HDR.search(l) or i <= 6:
            out.append(f"{i}: {l.rstrip()[:200]}")
    out.append("--- captures")
    for i, l in enumerate(txt, 1):
        if CAP.search(l):
            out.append(f"{i}: {l.strip()[:260]}")
    (OUT / (script + ".txt")).write_text("\n".join(out), encoding="utf-8")
print("done")
