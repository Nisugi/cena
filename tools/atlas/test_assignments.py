import copy
import unittest
from embedded_assignments import embedded_assignments, area_key


class EmbeddedTests(unittest.TestCase):
    def room(self, meta=None):
        return {1: dict(id=1, title=['Example'], uid=[10, 11],
                        location='Misleading location', meta=meta or [])}

    def test_missing_stays_missing(self):
        row = embedded_assignments(self.room())[1]
        self.assertEqual((row['area'], row['area_key'], row['region'], row['region_src']), ('', '', '', ''))
        self.assertEqual(row['status'], 'live')

    def test_native_authority_and_identity(self):
        graph = self.room(['area:town-guild', 'region:Town', 'map:status:closed'])
        before = copy.deepcopy(graph)
        row = embedded_assignments(graph)[1]
        self.assertEqual((row['area'], row['area_key'], row['region']), ('town-guild', 'town.guild', 'Town'))
        self.assertEqual(row['region_src'], 'native-meta')
        self.assertEqual(row['uid'], '10')
        self.assertEqual(graph, before)

    def test_rejects_ambiguous_or_unknown_data(self):
        for meta in (['area:a', 'area:b'], ['region:a', 'region:a'], ['area:'],
                     ['area: a'], ['map:status:unknown'], ['map:status:live', 'map:status:closed']):
            with self.subTest(meta=meta), self.assertRaises(ValueError):
                embedded_assignments(self.room(meta))

    def test_status(self):
        for meta, expected in [(['map:status:gone'], 'gone'),
                               (['map:virtual room', 'map:status:closed'], 'virtual')]:
            self.assertEqual(embedded_assignments(self.room(meta))[1]['status'], expected)

    def test_key_collision_refused(self):
        graph = self.room(['area:a-b'])
        graph[2] = dict(id=2, meta=['area:a b'])
        with self.assertRaisesRegex(ValueError, 'collision'):
            embedded_assignments(graph)


if __name__ == '__main__':
    unittest.main()
