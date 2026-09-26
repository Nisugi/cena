"""Build a complete, immutable explorer snapshot. Never contacts a game server.

Run from any directory. Scratch/output must be new directories on a data drive.
The explicit native exporter is built from tools/atlas-export with --locked.
"""
import argparse
from collections import Counter
from pathlib import Path
import subprocess

from common import read, write, sha
from embedded_assignments import embedded_assignments
from region import build
from regions import REGIONS
from world import world_index

ENGINE = 'e98f669e'


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--map', type=Path, required=True)
    parser.add_argument('--exporter', type=Path, required=True)
    parser.add_argument('--catalogue', type=Path, required=True)
    parser.add_argument('--work', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    args.work.mkdir(parents=True, exist_ok=False)
    args.output.mkdir(parents=True, exist_ok=False)
    public = args.work / 'bundles'
    public.mkdir()
    graph_path = args.work / 'graph.json'
    subprocess.run([str(args.exporter.resolve()), str(args.map.resolve()), '--graph', str(graph_path)], check=True)
    graph = {r['id']: r for r in read(graph_path)['rooms']}
    rows = list(embedded_assignments(graph).values())
    if set(r['region'] for r in rows) != {v[0] for v in REGIONS.values()}:
        raise ValueError('New source regions require inventory review')
    entries = []
    for slug, (region, preferred, prefix) in REGIONS.items():
        members = [r for r in rows if r['region'] == region and r['status'] in ('live', 'closed')]
        if not members:
            continue
        areas = Counter(r['area'] for r in members if r['area'])
        live_areas = {r['area'] for r in members if r['area'] and r['status'] == 'live'}
        candidates = live_areas or set(areas)
        if not candidates:
            raise ValueError(f'{slug} has no assigned primary area; add an explicit region-only profile')
        primary = preferred if preferred in candidates else sorted(candidates, key=lambda a: (-areas[a], a))[0]
        starts = [int(r['room_id']) for r in members if r['area'] == primary and r['status'] == 'live']
        start = min(starts or [int(r['room_id']) for r in members if r['area'] == primary])
        familiar = {'landing':228,'mist-harbor':3668,'icemule':2300,'tavaalor':3519}.get(slug)
        if familiar in starts:
            start = familiar
        title = region or 'No region assigned'
        label = primary.removeprefix(prefix).replace('-', ' ').title()
        underground = {'landing':'wehnimers-landing-catacombs','mist-harbor':'isle-of-the-four-winds-western-harbor-sewers','icemule':'icemule-trace-town-tunnels'}.get(slug)
        profile = dict(title=title, region=region, town=primary, primary_label=label,
            underground=underground, underground_label='Catacombs' if slug=='landing' else 'Underground',
            start_room=start, start_label=label, town_focus=label, area_prefix=prefix, layout_policy='native')
        build(slug, profile, graph, args.map, args.exporter, args.work, public, ENGINE)
        path = public / slug / 'data.json'
        data = read(path)
        owned = {int(r['room_id']) for r in members}
        bundled = {int(r) for r in data['rooms']}
        if not owned <= bundled:
            raise ValueError('Missing native region members')
        entries.append(dict(slug=slug, title=title, url=f'../{slug}/', status='automated-first-pass',
            source_rooms=len(owned), bundled_rooms=len(bundled), context_rooms=len(bundled-owned),
            unassigned_rooms=sum(not r['area'] for r in members), frames=len(data['scenes']),
            assigned_maps=sum(s.get('assignment_kind')!='region-only' for s in data['scenes'].values()), data_sha256=sha(path)))
    manifest = dict(schema='hydra-corpus-review-v1', assignment_source='native-meta',
        source_map_sha256=sha(args.map), engine_revision=ENGINE, catalogue_sha256=sha(args.catalogue),
        eligible_unique_rooms=sum(r['status'] in ('live','closed') for r in rows),
        excluded_statuses=dict(Counter(r['status'] for r in rows if r['status'] not in ('live','closed'))),
        policy='Native directed exits and ownership preserved. Automated area fills need review. Habitat gaps mean unknown, not creature-free.', regions=entries)
    if sum(e['source_rooms'] for e in entries) != manifest['eligible_unique_rooms']:
        raise ValueError('Corpus coverage mismatch')
    world, search = world_index(graph, REGIONS, manifest)
    for name, value in [('manifest',manifest),('world',world),('search',search)]:
        write(public / 'corpus' / (name+'.json'), value)
    subprocess.run(['node', str(Path(__file__).with_name('context.mjs')), 'match', str(args.catalogue), str(public)], check=True)
    inventory = {}
    for path in sorted(public.glob('*/*.json')):
        relative = path.relative_to(public)
        destination = args.output / relative
        destination.parent.mkdir(parents=True, exist_ok=True)
        destination.write_bytes(path.read_bytes())
        inventory[str(relative)] = dict(sha256=sha(path), bytes=path.stat().st_size)
    write(args.output / 'inventory.json', dict(source_map_sha256=sha(args.map), files=inventory))
    print(f"Verified {manifest['eligible_unique_rooms']} unique rooms across {len(entries)} region bundles")


if __name__ == '__main__':
    main()
