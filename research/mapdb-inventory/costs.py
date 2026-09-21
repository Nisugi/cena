"""Classify every scripted timeto by the kind of gate it is."""
import json, collections, re

d = json.load(open('E:/Cena/reference/mapdb/map-1789942730.json', encoding='utf-8'))
KINDS = [  # first match wins
    ('delegate to another edge',      r'(?:Map|Room)\[\d+\]\.timeto\['),
    ('instability table',             r'\$mapdb_instability_timeto'),
    ('day pass',                      r'day_pass'),
    ('urchins',                       r'mapdb_use_urchins'),
    ('go2 running (urchin exit)',     r'Script\.list'),
    ('voln seeking',                  r'use_seeking'),
    ('posture + climate',             r'checksitting'),
    ('trip variable (origin/return)', r'UserVars\.mapdb_\w*(origin|return_room|location|from_sos|to_sos)'),
    ('spell active (cost formula)',   r'Spell\[|checkspell'),
    ('profession',                    r'Stats\.prof'),
    ('race',                          r'Stats\.race'),
    ('gender',                        r'Stats\.gender'),
    ('level',                         r'Stats\.level|XMLData\.level'),
    ('skill rank',                    r'Skills\.'),
    ('society rank',                  r'Society\.'),
    ('citizenship',                   r'citizenship'),
    ('calendar month',                r'Time\.now'),
    ('carries item',                  r'GameObj\.inv'),
    ('another script running',        r'Script\.running'),
    ('user setting (bool/string)',    r'UserVars\.'),
    ('global flag',                   r'\$\w+'),
]
edges = collections.Counter(); shapes = collections.defaultdict(set); ex = {}
for r in d:
    for k, v in r['timeto'].items():
        if not (isinstance(v, str) and v.startswith(';e')):
            continue
        for name, pat in KINDS:
            if re.search(pat, v):
                break
        else:
            name = 'UNCLASSIFIED'
        edges[name] += 1
        shapes[name].add(re.sub(r'\d+(\.\d+)?', 'N', re.sub(r"'[^']*'|\"[^\"]*\"", 'S', v)))
        ex.setdefault(name, f"{r['id']}->{k}  {v[:150]}")
tot = sum(edges.values())
print('scripted costs', tot)
for name, n in edges.most_common():
    print(f'{n:5d}  {len(shapes[name]):3d} shapes  {name}\n           e.g. {ex[name]}')
# settings actually named
names = collections.Counter()
for r in d:
    for v in r['timeto'].values():
        if isinstance(v, str):
            for m in re.findall(r'UserVars\.(\w+)', v):
                names[m] += 1
print('\nUserVars read by costs:', names.most_common())
