"""Checks the actual shipped corpus, including every room and habitat match."""
import hashlib
import json
from collections import Counter
from pathlib import Path
import unittest

from embedded_assignments import embedded_assignments

ROOT = Path(__file__).resolve().parents[2]
DATA = ROOT / 'crates/cena-web/atlas-data'


def read(name):
    return json.loads((DATA / name).read_text(encoding='utf-8'))


class SnapshotTests(unittest.TestCase):
    def test_inventory_coverage_identity_and_habitat(self):
        inventory = read('inventory.json')
        for name, expected in inventory['files'].items():
            raw = (DATA / name).read_bytes()
            self.assertEqual(hashlib.sha256(raw).hexdigest(), expected['sha256'], name)
            self.assertEqual(len(raw), expected['bytes'], name)
        manifest = read('corpus/manifest.json')
        canonical = {r['id']: r for r in read('corpus/search.json')}
        self.assertEqual(len(canonical), manifest['eligible_unique_rooms'])
        self.assertEqual(sum(r['source_rooms'] for r in manifest['regions']), len(canonical))
        records, seen = {}, set()
        catalogue = json.loads((ROOT / 'tools/atlas/creature-catalogue.json').read_text())
        templates = {c['id']: c['record'] for c in catalogue['entries']}
        names = Counter(c['record']['name'].strip().lower() for c in catalogue['entries'])
        for region in manifest['regions']:
            slug = region['slug']
            bundle = read(slug+'/data.json')
            self.assertEqual(bundle['provenance']['hashes']['map'], manifest['source_map_sha256'])
            self.assertFalse(bundle['provenance']['map_mutated'])
            rooms = {int(k): r for k, r in bundle['rooms'].items()}
            assignments = embedded_assignments(rooms)
            displayed = [r['id'] for s in bundle['scenes'].values() for r in s['sheet']['rooms']]
            self.assertEqual(len(displayed), len(set(displayed)))
            self.assertEqual(set(displayed), set(rooms))
            self.assertTrue({i for i,r in canonical.items() if r['region']==slug} <= set(rooms))
            for rid, record in rooms.items():
                self.assertEqual(record['assignment'], assignments[rid])
                self.assertIn(record['status'], ('live','closed'))
                native = {k:v for k,v in record.items() if k not in ('area','assignment','status','ownership_source')}
                if rid in records:
                    self.assertEqual(native, records[rid], f'Context copy drift: {rid}')
                records[rid] = native
                seen.add(rid)
            context = read(slug+'/region-data.json')
            self.assertEqual(context['data_sha256'], inventory['files'][slug+'/data.json']['sha256'])
            for creature in context['creatures']:
                for association in creature['associations']:
                    self.assertIn(association['basis'],('room_uid_match','room_tag_match'))
                    for rid in association['roomIds']:
                        self.assertEqual(rooms[int(rid)]['area'],association['group'])
                        if association['basis'] == 'room_tag_match':
                            name = templates[creature['id']]['name'].strip().lower()
                            self.assertEqual(names[name], 1)
                            self.assertIn(name, [t.strip().lower() for t in rooms[int(rid)].get('tags', [])])
                        else:
                            spans = [s for a in templates[creature['id']]['areas'] for s in a.get('uids', [])]
                            self.assertTrue(any(
                                s == uid if isinstance(s, int) else s['min'] <= uid <= s['max']
                                for uid in rooms[int(rid)].get('uid', []) for s in spans))
        self.assertEqual(seen, set(canonical))
        # Each world connection is an actual directed exit, not a guessed bridge.
        world = read('corpus/world.json')
        for link in world['links']:
            for record in link['records']:
                self.assertEqual(records[record['from']]['exits'][record['ordinal']],record['edge'])
                self.assertEqual(record['edge']['to'],record['to'])


if __name__ == '__main__':
    unittest.main()
