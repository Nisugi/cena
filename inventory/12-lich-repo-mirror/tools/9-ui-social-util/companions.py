import re
t = open('E:/Cena/reference/lich_repo_mirror/lib/recolor.lic', 'rb').read().decode('utf-8', 'replace').splitlines()
for i in range(745, 757):
    line = t[i]
    m = re.search(r':noun => /\^\(\?:(.*?)\)\$/', line)
    if m:
        print('line', i + 1, 'companion nouns', len(m.group(1).split('|')))
    m = re.search(r':exclude => /\^\(\?:(.*?)\|#\{familiars\}', line)
    if m:
        print('line', i + 1, 'exclude names', len(m.group(1).split('|')))
    m = re.search(r'familiars = "(.*)"', line)
    if m:
        print('line', i + 1, 'familiar alternation top-level branches', len(re.findall(r'\)\s*(?:cockatiel|bat|cat|chameleon|cormorant|coyote|falcon|fox|frog|gyrfalcon|hare|hawk|jackal|kitten)', m.group(1))))
