import pathlib, sys
lib = pathlib.Path(r"E:/Cena/reference/lich_repo_mirror/lib")
base_paths = {
    "go2(baseline)": pathlib.Path(r"E:/Cena/reference/scripts/scripts/go2.lic"),
    "go2(mapdb)": pathlib.Path(r"E:/Cena/reference/mapdb/go2.lic"),
    "sbounty(baseline)": pathlib.Path(r"E:/Cena/reference/scripts/scripts/sbounty.lic"),
    "ebounty(baseline)": pathlib.Path(r"E:/Cena/reference/scripts/scripts/ebounty.lic"),
    "escortgo2(baseline)": pathlib.Path(r"E:/Cena/reference/scripts/scripts/escortgo2.lic"),
}
def lines(p):
    return set(l.strip() for l in p.read_bytes().decode("utf-8", "replace").splitlines() if len(l.strip()) > 12)
bases = {k: lines(v) for k, v in base_paths.items() if v.exists()}
names = sys.argv[1:]
for extra in names:
    if extra.startswith("+"):
        bases[extra[1:]] = lines(lib / extra[1:])
for n in names:
    if n.startswith("+"):
        continue
    s = lines(lib / n)
    best = sorted(((len(s & b) / max(1, len(s)), k) for k, b in bases.items() if k != n), reverse=True)[:2]
    print(f"{n}\t{len(s)}\t" + "  ".join(f"{k}:{r:.2f}" for r, k in best))
