import re
t = open('E:/Cena/reference/lich_repo_mirror/lib/recolor.lic', 'rb').read().decode('utf-8', 'replace').splitlines()[739]
s = re.search(r'familiars = "(.*)"', t).group(1)
# split on '|' at paren depth 0
branches, depth, cur = [], 0, ''
for ch in s:
    if ch == '(':
        depth += 1
    elif ch == ')':
        depth -= 1
    if ch == '|' and depth == 0:
        branches.append(cur)
        cur = ''
    else:
        cur += ch
branches.append(cur)
nouns = sorted({re.sub(r'[^a-z-]', '', b.strip().split()[-1]) for b in branches if b.strip()})
print(len(branches), 'top-level branches; species nouns:', ' '.join(nouns))
