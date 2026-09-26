import re
l=open('E:/Cena/reference/lich_repo_mirror/lib/crit-tracking.lic',encoding='utf-8',errors='replace').read().splitlines()[361]
parts=[r"\s*'\b\((\d+)\|([^)]*)\)\b'", r"\s*=>\s*\{\s*:armor_type\s*=>\s*\"([^\"]+)\"", r",\s*:avd\s*=>\s*(-?\d+)", r",\s*:df\s*=>\s*([\d.]+)", r",\s*:wt\s*=>\s*\"([^\"]+)\""]
acc=""
for p in parts:
    acc+=p; print(bool(re.match(acc,l)), p)
