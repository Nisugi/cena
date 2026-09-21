"""How do Lich ids and Simu uids relate in the mapdb? Can uid be the sole key?"""
import json, collections

d = json.load(open('E:/Cena/reference/mapdb/map-1789942730.json', encoding='utf-8'))
by = {r['id']: r for r in d}
uid_to_ids = collections.defaultdict(list)
for r in d:
    for u in r.get('uid') or []:
        uid_to_ids[u].append(r['id'])

no_uid = [r for r in d if not r.get('uid')]
multi_uid_rooms = [r for r in d if len(r.get('uid') or []) > 1]
shared = {u: ids for u, ids in uid_to_ids.items() if len(ids) > 1}
print('rooms', len(d), 'no-uid', len(no_uid), 'rooms with >1 uid', len(multi_uid_rooms))
print('distinct uids', len(uid_to_ids), 'uids shared by >1 room', len(shared),
      'rooms involved', len({i for ids in shared.values() for i in ids}))
print('largest shared:', sorted(((len(v), u) for u, v in shared.items()), reverse=True)[:5])
print('max uids on one room:', max(len(r.get('uid') or []) for r in d))

# are no-uid rooms connected (i.e. do they matter for routing)?
targets = collections.Counter()
for r in d:
    for k in r['wayto']:
        targets[int(k)] += 1
nu_ids = {r['id'] for r in no_uid}
print('no-uid rooms with outgoing edges', sum(1 for r in no_uid if r['wayto']),
      'with incoming', sum(1 for i in nu_ids if targets[i]),
      'isolated', sum(1 for r in no_uid if not r['wayto'] and not targets[r['id']]))
edges = sum(len(r['wayto']) for r in d)
touch = sum(1 for r in d for k in r['wayto'] if r['id'] in nu_ids or int(k) in nu_ids)
print('edges touching a no-uid room', touch, 'of', edges)

# what are the no-uid rooms?
loc = collections.Counter(str(r.get('location')) for r in no_uid)
print('no-uid by location:', loc.most_common(12))
tg = collections.Counter(t for r in no_uid for t in (r.get('tags') or []) if t.startswith('meta:') and 'forage' not in t)
print('no-uid meta tags:', tg.most_common(10))
print('no-uid without title', sum(1 for r in no_uid if not r.get('title')),
      'without description', sum(1 for r in no_uid if not r.get('description')))
# uid magnitude / negative / synthetic
us = list(uid_to_ids)
print('uid range', min(us), max(us), 'negative', sum(1 for u in us if u < 0), '>2^32', sum(1 for u in us if u > 2**32))
# tags that steer multi-uid handling
for t in ('meta:map:multi-uid', 'meta:map:latest-only', 'meta:playershop'):
    print(t, sum(1 for r in d if t in (r.get('tags') or [])))
# dangling edge targets
print('edges to nonexistent ids', sum(1 for r in d for k in r['wayto'] if int(k) not in by))
