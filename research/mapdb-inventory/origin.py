import json, re, sys, collections
rooms = json.load(open(sys.argv[1], encoding="utf-8"))
W = re.compile(r"UserVars\.(mapdb_\w+?_origin|mapdb_fwi_return_room)\s*=\s*([^;=\n]+)")
same = other = 0; odd = []
dests = collections.defaultdict(set)
for r in rooms:
    for to, v in (r.get("wayto") or {}).items():
        if not isinstance(v, str): continue
        for name, val in W.findall(v):
            val = val.strip()
            dests[name].add(int(to))
            if val == str(r["id"]): same += 1
            else: other += 1; odd.append((r["id"], to, name, val[:60]))
print("writes equal to the room being left:", same, " other:", other)
for o in odd[:8]: print(o)
for n, d in dests.items(): print(n, "-> lands in", sorted(d)[:6], len(d))
R = re.compile(r"UserVars\.(mapdb_\w+?_origin|mapdb_fwi_return_room)")
shapes = collections.Counter()
for r in rooms:
    for to, v in (r.get("timeto") or {}).items():
        if isinstance(v, str) and R.search(v):
            ok = re.search(r"==\s*%s\b" % to, v) is not None
            shapes[ok] += 1
print("reads comparing against the exit's own destination:", dict(shapes))
