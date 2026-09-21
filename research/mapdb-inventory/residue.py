"""Cluster Vellum's 147 uncrossable edges and tally what each family needs."""
import collections, re, sys

BS = chr(92)
Q = re.compile('"(?:[^"' + BS*2 + ']|' + BS*2 + '.)*"|' + "'(?:[^'" + BS*2 + "]|" + BS*2 + ".)*'")
NUM = re.compile(BS + 'd+(?:' + BS + '.' + BS + 'd+)?')
WS = re.compile(BS + 's+')

rows = []
cur = None
for line in open('E:/Cena/research/mapdb-inventory/vellum_residue.txt', encoding='utf-8'):
    parts = line.rstrip('\n').split('\t', 2)
    if len(parts) == 3 and parts[0].isdigit() and parts[1].isdigit():
        cur = [parts[0], parts[1], parts[2]]
        rows.append(cur)
    elif cur is not None:
        cur[2] += '\n' + line.rstrip('\n')

def norm(s):
    return WS.sub(' ', NUM.sub('N', Q.sub('S', s))).strip()

fam = collections.OrderedDict()
for a, b, src in rows:
    fam.setdefault(norm(src), []).append((a, b, src))

# what capability does each family lean on?
NEEDS = [
    ('cast/spell', r'Spell\[|\.cast|cast\(|affordable|\.known\?'),
    ('npc/escort', r'GameObj\.npcs|bounty\?|Society\.task'),
    ('loot/object scan', r'GameObj\.loot|checkloot'),
    ('inventory/hands', r'GameObj\.inv|checkleft|checkright|GameObj\.(right|left)_hand|empty_hand|fill_hand'),
    ('read a line + branch', r'\bget\b|matchwait|matchtimeout|=~ /|\.match\('),
    ('random exit', r'rand\('),
    ('checkpaths', r'checkpaths'),
    ('uservar list', r'UserVars\.\w+\.each'),
    ('group', r'group_members|Group\.'),
    ('room id loop', r'Room\.current\.id|Map\.current\.id'),
    ('delegate', r'\.wayto\[.*\]\.call'),
    ('run script', r'Script\.run|start_script'),
    ('silver/bank', r'silver|withdraw|deposit|Go2\.'),
    ('stats', r'Stats\.|Skills\.|Char\.'),
    ('time', r'Time\.now'),
]
tally = collections.Counter(); ftally = collections.Counter()
out = []
for i, (k, es) in enumerate(sorted(fam.items(), key=lambda kv: -len(kv[1])), 1):
    src = es[0][2]
    needs = [n for n, pat in NEEDS if re.search(pat, src)]
    for n in needs:
        tally[n] += len(es); ftally[n] += 1
    out.append(f"{i}\t{len(es)}\t{len(src)}\t{es[0][0]}->{es[0][1]}\t{','.join(needs) or '-'}\t{WS.sub(' ', src)[:260]}")
open('E:/Cena/research/mapdb-inventory/residue_families.tsv', 'w', encoding='utf-8').write('\n'.join(out))
print('edges', len(rows), 'families', len(fam))
print('capability -> edges / families')
for n, c in tally.most_common():
    print(f'  {n}: {c} / {ftally[n]}')
lens = sorted(len(es[0][2]) for es in fam.values())
print('family source length: median', lens[len(lens)//2], 'over 500 chars:', sum(1 for l in lens if l > 500), 'over 1500:', sum(1 for l in lens if l > 1500))
