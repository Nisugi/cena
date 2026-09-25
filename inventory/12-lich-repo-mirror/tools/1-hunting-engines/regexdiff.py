import re

SRCS = [
    'E:/Cena/reference/scripts/scripts/bigshot.lic',
    'C:/Gemstone/scripts/scripts/bigshot.lic',
    'E:/Gemstone/dev/lich5-docker/scripts/bigshot.lic',
    'C:/Gemstone/lich-5/scripts/bigshot.lic',
]
RX = re.compile(r'(?<![\w\)\]])/((?:[^/\\\n]|\\.)+)/[imxo]*')


def regexes(path):
    text = open(path, 'rb').read().decode('utf-8', 'replace')
    out = {}
    for i, line in enumerate(text.splitlines(), 1):
        s = line.strip()
        if s.startswith('#'):
            continue
        for m in RX.finditer(line):
            body = m.group(1).strip()
            if len(body) < 6:
                continue
            out.setdefault(body, i)
    return out


known = set()
for s in SRCS:
    known.update(regexes(s).keys())
b2 = regexes('E:/Cena/reference/lich_repo_mirror/lib/bigshit2.lic')
novel = sorted((ln, body) for body, ln in b2.items() if body not in known)
with open('b2_novel_regex.txt', 'w', encoding='utf-8') as f:
    for ln, body in novel:
        f.write(f'{ln}: {body[:220]}\n')
print('bigshit2 regexes', len(b2), 'novel vs all bigshot copies', len(novel))
