"""Cut the converter's test fixture from the real mapdb.

Rooms are chosen for what they exercise, and their exits are trimmed to each
other so the cut is self-contained (no dangling exits) and small.
"""
import json

SRC = 'E:/Cena/reference/mapdb/map-1789942730.json'
OUT = 'E:/Cena/crates/cena-mapdb-convert/tests/fixtures/mapdb_cut.json'
d = {r['id']: r for r in json.load(open(SRC, encoding='utf-8'))}

def first(pred):
    return next(r['id'] for r in d.values() if pred(r))

picks = [
    0,        # inn tables: many scripted crossings of ONE shape, plain numeric costs
    7,        # the urchin gate: a scripted COST on a `;e true` crossing
    30714,    # the virtual urchin hub: no uid, plain `urchin guide …` commands
    4136,     # negative uids
    18011,    # fifty uids on one room
    first(lambda r: r.get('location') is False and r['wayto']),
    first(lambda r: r.get('check_location') and r['wayto']),
    first(lambda r: r.get('unique_loot')),
    first(lambda r: r.get('image') and any(v in ('up', 'down') for v in r['wayto'].values())),
]
# pull in a few plain neighbours so exits have somewhere to go
keep = set(picks)
for i in picks:
    plain = [int(k) for k, v in d[i]['wayto'].items() if not v.startswith(';e')][:2]
    keep.update(plain)
    # the vertical pick must keep its vertical exit, or it exercises nothing
    keep.update(int(k) for k, v in d[i]['wayto'].items() if v in ('up', 'down'))
keep.update([1, 2])  # two of room 0's table rooms, so its scripted exits survive the trim

out = []
for i in sorted(keep):
    r = json.loads(json.dumps(d[i]))
    r['wayto'] = {k: v for k, v in r['wayto'].items() if int(k) in keep}
    r['timeto'] = {k: v for k, v in r['timeto'].items() if int(k) in keep}
    r['tags'] = [t for t in r.get('tags', []) if not t.startswith('meta:forage-sensed:')][:8]
    if len(r.get('uid', [])) > 6:
        r['uid'] = r['uid'][:6]
    out.append(r)
json.dump(out, open(OUT, 'w', encoding='utf-8'), indent=1, ensure_ascii=False)
print(len(out), 'rooms', sum(len(r['wayto']) for r in out), 'exits', 'picks', picks)
