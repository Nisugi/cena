import re, csv
src = open('E:/Cena/reference/lich_repo_mirror/lib/spellwatch.lic', 'rb').read().decode('utf-8', 'replace').splitlines()
pairs = []
cur = None
for i, l in enumerate(src, 1):
    m = re.search(r"if line =~ /(.*)/", l)
    if m:
        cur = (i, m.group(1))
        continue
    m = re.search(r'puts\("#\{fam_window_begin\}\((\d+)\) (.*?) - is no longer', l)
    if m and cur:
        pairs.append((cur[0], cur[1], int(m.group(1)), m.group(2)))
        cur = None
rows = list(csv.DictReader([l for l in open('E:/Cena/crates/cena-model/data/spells.tsv', encoding='utf-8') if not l.startswith('#')], delimiter='\t'))
bynum = {int(r['number']): r for r in rows if r['number'].isdigit()}
have = diff = miss = 0
out = []
for ln, pat, num, name in pairs:
    r = bynum.get(num)
    pat2 = pat.replace('&apos;', "'")
    plain = pat2.replace('\\', '').lstrip('^')
    if not r:
        out.append((ln, num, name, 'NO SPELL ROW', plain))
        miss += 1
        continue
    end = r['msg_end']
    ok = plain.lower() in end.lower()
    if not ok:
        try:
            ok = re.search(pat2, end) is not None
        except re.error:
            ok = False
    if ok:
        have += 1
    else:
        diff += 1
        out.append((ln, num, name, 'DIFF', repr(plain), '| cena msg_end:', repr(end[:160])))
print(len(pairs), 'pairs; match', have, 'diff', diff, 'no-row', miss)
for o in out:
    print(*o)
