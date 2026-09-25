import re, sys, pathlib
LIB = pathlib.Path(r"E:/Cena/reference/lich_repo_mirror/lib")
CAP = re.compile(r"matchtimeout|matchwait|waitforre|waitfor\b|match\(|DownstreamHook|Regexp\.new|%r\{|when +/|=~ */|dothistimeout|dothis\b|matchfind|\.match\(|/\)|/i\)")
NOISE = re.compile(r"prepped\? =~|checkprep =~|Char\.name =~|\.noun =~|target\.to_i|script\.vars|variable\[")
def header(lines):
    out=[]
    for i,l in enumerate(lines[:60]):
        s=l.strip()
        if re.search(r"author|version|game:|tags:|required:|contributors|license|updated|date|=begin|^#.*(purpose|description)", s, re.I):
            out.append(f"{i+1}: {s[:160]}")
    return out[:14]
for name in sys.argv[1:]:
    p = LIB/name
    t = p.read_bytes().decode("utf-8","replace").splitlines()
    print(f"===== {name} ({len(t)} lines)")
    for h in header(t): print("  H", h)
    seen=set()
    n=0
    for i,l in enumerate(t):
        if CAP.search(l) and not NOISE.search(l):
            s=l.strip()
            if s in seen: continue
            seen.add(s); n+=1
            if n<=int(__import__('os').environ.get('MAXC','60')):
                print(f"  C{i+1}: {s[:230]}")
    print(f"  (unique capture lines: {n})")
