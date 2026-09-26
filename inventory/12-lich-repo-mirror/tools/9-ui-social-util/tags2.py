"""Tags the scripts MATCH against the server stream (regex/literal-test lines only), known vs not."""
import re, sys, collections
D = 'C:/Users/shawn/AppData/Local/Temp/claude/e--Cena/a4e2b7d3-05c6-411f-9da3-d545e41fa29c/scratchpad/survey/tmp-9-ui-social-util/'
L = 'E:/Cena/reference/lich_repo_mirror/lib/'
known = set(open(D + 'cena_tags.txt', encoding='utf-8').read().split())
files = sys.argv[1:] or [l.strip() for l in open(D + 'pile.txt', encoding='utf-8') if l.strip()]
TAG = re.compile(r"(?<![?(])<\\?/?([A-Za-z][A-Za-z0-9_]*)(?=[\s/>\\'\"]|\\s)")
MATCH = re.compile(r"=~|\.match\??\(|when\s+/|%r\{|Regexp\.new|scan\(|include\?\(|waitfor|matchtimeout|matchfind|=~")
seen = collections.defaultdict(set)
for s in files:
    try:
        lines = open(L + s, 'rb').read().decode('utf-8', 'replace').splitlines()
    except FileNotFoundError:
        continue
    for i, line in enumerate(lines, 1):
        if not MATCH.search(line):
            continue
        for m in TAG.finditer(line):
            seen[m.group(1)].add(f'{s}:{i}')
k = sorted(t for t in seen if t in known)
u = sorted(t for t in seen if t not in known)
print('MATCHED & KNOWN (%d):' % len(k), ' '.join(k))
print('MATCHED & NOT IN TABLE (%d):' % len(u))
for t in u:
    print('  ', t, sorted(seen[t])[:5])
