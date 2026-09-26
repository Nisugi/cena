"""World navigation indexes. Source-region ownership, never bundle context copies."""
from pathlib import Path

from embedded_assignments import embedded_assignments
from common import read, write

NON_GEOGRAPHIC = {'unassigned-region', 'consultation', 'events', 'instanced', 'transport', 'crystal'}


def world_index(graph, regions, manifest):
    assignments = embedded_assignments(graph)
    slugs = {region: slug for slug, (region, *_rest) in regions.items()}
    eligible = {rid: row for rid, row in assignments.items() if row['status'] in ('live', 'closed')}
    nodes = [dict(id=e['slug'], name=e['title'], rooms=e['source_rooms'],
                  url=e.get('url'), maps=e.get('assigned_maps', 0),
                  unassigned=e.get('unassigned_rooms', 0),
                  geographic=e['slug'] not in NON_GEOGRAPHIC)
             for e in manifest['regions']]
    pairs, unknown = {}, []
    for rid, row in sorted(eligible.items()):
        source = slugs[row['region']]
        for ordinal, edge in enumerate(graph[rid].get('exits', [])):
            target_row = eligible.get(edge['to'])
            if target_row is None:
                unknown.append(dict(from_room=rid, ordinal=ordinal, to=edge['to']))
                continue
            target = slugs[target_row['region']]
            if source == target:
                continue
            a, b = sorted((source, target))
            key = a + '|' + b
            pair = pairs.setdefault(key, dict(id=key, a=a, b=b, records=[]))
            pair['records'].append(dict(source=source, target=target, **{'from': rid},
                to=edge['to'], ordinal=ordinal, edge=edge,
                closed=row['status'] == 'closed' or target_row['status'] == 'closed',
                special=not edge.get('cmd') or edge.get('kind') == 'scripted'
                        or 'pass' in edge or bool(edge.get('routine'))))
    search = [dict(id=rid, title=row['title'], region=slugs[row['region']],
                   area=row['area'], status=row['status']) for rid, row in sorted(eligible.items())]
    return dict(schema='hydra-world-navigation-v1', source_map_sha256=manifest['source_map_sha256'],
                nodes=nodes, links=sorted(pairs.values(), key=lambda p: p['id']),
                excluded_endpoint_records=unknown), search
