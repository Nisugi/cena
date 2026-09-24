"""Build another town through the same pinned native exporter and browser UI."""
import argparse
from collections import Counter, defaultdict
import json
from pathlib import Path
import subprocess
from common import components, read, write, sha
from embedded_assignments import embedded_assignments

PROFILES = {
    'mist-harbor': dict(title='Mist Harbor', region='Mist Harbor / Four Winds Isle',
        town='isle-of-the-four-winds-mist-harbor-town',
        underground='isle-of-the-four-winds-western-harbor-sewers',
        underground_label='Western Harbor sewers', start_room=3668, layout_policy='native'),
    'icemule': dict(title='Icemule Trace', region='Icemule Trace',
        town='icemule-trace-town', underground='icemule-trace-town-tunnels',
        underground_label='Town tunnels', start_room=2300, layout_policy='reference'),
}


def area_label(area, profile):
    # Drop the region namespace for display, never for identity or ownership.
    prefix = profile.get('area_prefix', 'isle-of-the-four-winds-' if profile['title'] == 'Mist Harbor' else 'icemule-trace-' if profile['title'] == 'Icemule Trace' else '')
    return area.removeprefix(prefix).replace('-', ' ').title()


def build(key, profile, graph, map_path, binary, work, output, engine_revision):
    out = work / key
    out.mkdir(parents=True, exist_ok=True)
    assignments = embedded_assignments(graph)
    if profile.get('underground') and not any(r['area'] == profile['underground'] for r in assignments.values()):
        # An area removed/merged upstream is not recreated by a stale UI profile.
        profile = {**profile, 'underground': None}
    eligible = {rid for rid, r in assignments.items() if r['status'] in ('live', 'closed')}
    region_ids = {rid for rid in eligible if assignments[rid]['region'] == profile['region']}
    area_names = {assignments[rid]['area'] for rid in region_ids} - {''}
    selection, specs = {}, {}
    for area in sorted(area_names):
        # Complete assigned areas even if a few members have another/absent
        # region label. Retain both original identities; report this context.
        ids = {rid for rid in eligible if assignments[rid]['area'] == area}
        scene_key = 'town' if area == profile['town'] else 'catacombs' if area == profile.get('underground') else 'area-' + assignments[min(ids)]['area_key']
        selection[scene_key] = sorted(ids)
        label = profile.get('primary_label', profile['title']) if scene_key == 'town' else profile['underground_label'] if scene_key == 'catacombs' else area_label(area, profile)
        specs[scene_key] = dict(label=label, slug=area, area_key=assignments[min(ids)]['area_key'], assignment_kind='native-area', region=profile['region'])
    included = set().union(*(set(ids) for ids in selection.values()))
    for group in components(region_ids - included, graph):
        scene_key = 'unassigned-' + str(group[0])
        names = Counter((graph[rid].get('title') or [f'Room #{rid}'])[0].strip('[]').split(',')[0] for rid in group)
        selection[scene_key] = group
        specs[scene_key] = dict(label=f'{names.most_common(1)[0][0]} · unassigned group #{group[0]}', assignment_kind='region-only', region=profile['region'])
    write(out / 'selection.json', selection)
    manifest = dict(map=sha(map_path), assignment_source='native-meta',
        selection=sha(out / 'selection.json'), binary=sha(binary), engine_revision=engine_revision)
    native_path = out / 'native-scenes.json'
    if native_path.exists():
        if read(out / 'native-manifest.json') != manifest:
            raise ValueError('Stale native export: preserve this pilot and choose a new output directory')
    else:
        subprocess.run([str(binary), str(map_path), str(out / 'selection.json'), str(native_path)], check=True)
        write(out / 'native-manifest.json', manifest)
    native = read(native_path)
    scenes, rooms = {}, {}
    for scene_key, blob in native.items():
        scenes[scene_key] = {**blob['scene'], **specs[scene_key]}
        for record in blob['rooms']:
            rid = record['id']
            if record != graph[rid] or str(rid) in rooms:
                raise ValueError('Native record changed or duplicate membership')
            assignment = assignments[rid]
            rooms[str(rid)] = {**record, 'exits': record.get('exits', []), 'area': scene_key,
                'assignment': assignment, 'status': assignment['status'],
                'ownership_source': 'Native map metadata; unassigned display groups are not new areas'}
    assert region_ids <= {int(rid) for rid in rooms}
    assert rooms[str(profile['start_room'])]['area'] == 'town'
    underground = set(selection.get('catacombs', []))
    boundary = []
    for rid, room in graph.items():
        for ordinal, edge in enumerate(room.get('exits', [])):
            if (rid in underground) != (edge['to'] in underground):
                boundary.append(dict(from_=rid, to=edge['to'], ordinal=ordinal, edge=edge,
                    external_endpoint=edge['to'] if rid in underground else rid))
    for edge in boundary:
        edge['from'] = edge.pop('from_')
    outside_ids = {e['to'] for room in rooms.values() for e in room['exits'] if str(e['to']) not in rooms}
    outside_ids.update(e['external_endpoint'] for e in boundary if str(e['external_endpoint']) not in rooms)
    outside = {str(rid): {'id': rid, 'title': graph.get(rid, {}).get('title', []),
        'location': graph.get(rid, {}).get('location'), 'assignment': assignments.get(rid), 'bundled': False} for rid in outside_ids}
    stats = dict(region_rooms=len(region_ids), region_unassigned=sum(not assignments[r]['area'] for r in region_ids),
        bundled_rooms=len(rooms), frames=len(scenes), closed=sum(r['status']=='closed' for r in rooms.values()),
        nonregion_assigned_context=sorted(int(rid) for rid in rooms if int(rid) not in region_ids),
        reference_rooms=sum(bool(r.get('image', {}).get('rect')) for r in rooms.values()),
        underground_boundary_edges=len(boundary), underground_unbundled_endpoints=sorted(outside_ids & {e['external_endpoint'] for e in boundary}))
    tours = [{'room': profile['start_room'], 'name': profile.get('start_label', 'Town center')}]
    for scene_key, scene in scenes.items():
        if scene_key == 'catacombs' or any(s in scene.get('slug', '') for s in ['playershops', 'private-homes', 'south-gate-wilds', 'ranger-guild']):
            tours.append({'room': selection[scene_key][0], 'name': scene['label']})
    config = {**profile, 'key': key, 'tours': tours[:8], 'town_focus': profile.get('town_focus', 'Town streets'), 'primary_tabs': ['town'] + (['catacombs'] if underground else [])}
    data = dict(scenes=scenes, rooms=rooms, outside=outside, reviewed={}, presentation=config,
        catacomb_boundary=dict(catacomb_rooms=sorted(underground), external_endpoints=sorted({e['external_endpoint'] for e in boundary}), directed_boundary_edges=boundary),
        assignment_import=stats, provenance=dict(engine_revision=engine_revision,
            hashes=manifest, map_mutated=False, native_export=key+'/native-scenes.json', limitations=[
                'Area and region read directly from native map metadata. Missing assignments remain missing; no separate assignment file.',
                'Native positions regenerated with the pinned upstream engine. Automated area placements are not human-reviewed boundaries.',
                'Region-only groups are provisional connected previews, not approved areas, floors or hunting selections.',
                'Native mode preserves exported positions; artwork comparison is a separate optional display transform.',
                'Missing artwork positions are not evidence that native geometry is incorrect. Service/title annotations need visual review.',
                'Closed rooms remain visible but cannot be used in routes. Missing destinations remain explicit unbundled links.']))
    public = output / key
    public.mkdir(parents=True, exist_ok=True)
    write(public / 'data.json', data)
    write(out / 'IMPORT-REPORT.json', stats)
    print(json.dumps({'region': key, 'rooms': len(rooms), 'frames': len(scenes), 'area_unassigned': stats['region_unassigned']}))
