import re, sys, pathlib
lib = pathlib.Path(r"E:/Cena/reference/lich_repo_mirror/lib")
pile = pathlib.Path(r"../pile-8-bounty-events-travel.tsv").read_text(encoding="utf-8").splitlines()[1:]
pat = re.compile(r"=~\s*/|!~\s*/|waitforre|matchtimeout|matchwait|matchfind|DownstreamHook|when\s+/|Regexp\.new|%r\{|\.match\(|waitfor\s|dothistimeout|\.scan\(/|/\s*=~|match\s*\(")
out = pathlib.Path("caps"); out.mkdir(exist_ok=True)
for row in pile:
    name = row.split("\t")[0]
    txt = (lib/name).read_bytes().decode("utf-8","replace").splitlines()
    lines = [f"{i+1}: {l.strip()[:400]}" for i,l in enumerate(txt) if pat.search(l)]
    (out/(name+".caps")).write_text("\n".join(lines), encoding="utf-8")
print("done")
