"""Collect element names the pile's scripts match or emit, and diff against Cena's KNOWN_WIRE_TAGS."""
import re, collections
D = 'C:/Users/shawn/AppData/Local/Temp/claude/e--Cena/a4e2b7d3-05c6-411f-9da3-d545e41fa29c/scratchpad/survey/tmp-9-ui-social-util/'
L = 'E:/Cena/reference/lich_repo_mirror/lib/'
known = set(open(D + 'cena_tags.txt', encoding='utf-8').read().split())
pile = [l.strip() for l in open(D + 'pile.txt', encoding='utf-8') if l.strip()]
# element name after '<' or '<\/' or '<\\/' (escaped in regex), followed by space, '/', '>' or '\'
TAG = re.compile(r"<\\?/?([A-Za-z][A-Za-z0-9_]*)(?=[\s/>\\'\"(]|\\s|$)")
REGEX_LINE = re.compile(r"=~|match|when\s+/|%r\{|Regexp|scan\(|include\?|waitfor|matchtimeout|start_with|sub!?\(|gsub!?\(")
hits = collections.defaultdict(lambda: collections.defaultdict(list))  # tag -> script -> [(line, kind)]
for s in pile:
    try:
        lines = open(L + s, 'rb').read().decode('utf-8', 'replace').splitlines()
    except FileNotFoundError:
        continue
    for i, line in enumerate(lines, 1):
        kind = 'match' if REGEX_LINE.search(line) else 'emit'
        for m in TAG.finditer(line):
            hits[m.group(1)][s].append((i, kind))
unknown = sorted(t for t in hits if t not in known)
print('distinct names seen:', len(hits), ' known:', len([t for t in hits if t in known]), ' not in Cena table:', len(unknown))
for t in unknown:
    scripts = hits[t]
    kinds = collections.Counter(k for v in scripts.values() for _, k in v)
    sample = '; '.join(f"{s}:{v[0][0]}" for s, v in list(scripts.items())[:4])
    print(f"{t}\t{dict(kinds)}\t{len(scripts)} scripts\t{sample}")
