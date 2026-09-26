import pathlib, sys, re
# Print capture-ish lines of script A whose regex literal does not appear in baseline file(s) B...
lib = pathlib.Path(r"E:/Cena/reference/lich_repo_mirror/lib")
a = sys.argv[1]
bases = sys.argv[2:]
pat = re.compile(r"=~\s*/|!~\s*/|waitforre|matchtimeout|matchwait|dothistimeout|when\s+/|Regexp\.new|%r\{|waitfor\s")
lit = re.compile(r"/((?:[^/\\\n]|\\.){6,})/")
btxt = ""
for b in bases:
    p = pathlib.Path(b)
    if not p.exists():
        p = lib / b
    btxt += p.read_bytes().decode("utf-8", "replace")
for i, l in enumerate((lib / a).read_bytes().decode("utf-8", "replace").splitlines()):
    if not pat.search(l):
        continue
    lits = lit.findall(l)
    if not lits:
        continue
    new = [x for x in lits if x not in btxt]
    if new:
        print(f"{a}:{i+1}: " + " || ".join(n[:220] for n in new))
