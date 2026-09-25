import re, pathlib, sys
lib = pathlib.Path(r"E:/Cena/reference/lich_repo_mirror/lib")
keys = re.compile(r"author|version|date|updated|license|game|tags|required|purpose|description|^\s*#?\s*[A-Z][a-z]+ .*(script|will|does)", re.I)
for n in sys.argv[1:]:
    txt = (lib / n).read_bytes().decode("utf-8", "replace").splitlines()
    head = txt[:60]
    picked = []
    for i, l in enumerate(head):
        s = l.strip()
        if not s:
            continue
        if keys.search(s) or i < 6:
            picked.append(f"{i+1}: {s[:160]}")
    print(f"=== {n} ({len(txt)} lines)")
    print("\n".join(picked[:14]))
