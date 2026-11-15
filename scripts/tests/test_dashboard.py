"""Check dashboard coverage and units against the exporter contract."""
import json
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parents[2]


class DashboardTests(unittest.TestCase):
    def setUp(self):
        self.dashboard = json.loads((ROOT / 'dashboards/truenas-exporter.json').read_text())
        self.panels = [p for p in self.dashboard['panels'] if p['type'] != 'row']

    def test_all_cataloged_metric_families_have_queries(self):
        catalog = set(re.findall(r'\btruenas_[a-z_]+', (ROOT / 'docs/metric-catalog.md').read_text()))
        expressions = '\n'.join(t['expr'] for p in self.panels for t in p['targets'])
        queried = set(re.findall(r'\btruenas_[a-z_]+', expressions))
        self.assertEqual(catalog, queried)
        self.assertNotIn('or vector(0)', expressions)

    def test_layout_has_unique_ids_and_no_overlapping_panels(self):
        panels = self.dashboard['panels']
        self.assertEqual(len(panels), len({p['id'] for p in panels}))
        for i, panel in enumerate(panels):
            box = panel['gridPos']
            self.assertGreaterEqual(box['x'], 0)
            self.assertLessEqual(box['x'] + box['w'], 24)
            for other in panels[i + 1:]:
                other = other['gridPos']
                overlaps = (box['x'] < other['x'] + other['w']
                            and other['x'] < box['x'] + box['w']
                            and box['y'] < other['y'] + other['h']
                            and other['y'] < box['y'] + box['h'])
                self.assertFalse(overlaps, panel['title'])

    def test_rate_gauges_and_scan_progress_keep_source_units(self):
        for panel in self.panels:
            for target in panel['targets']:
                expression = target['expr']
                if 'truenas_network_receive_bytes_per_second' in expression or 'truenas_network_transmit_bytes_per_second' in expression:
                    self.assertNotIn('rate(', expression)
                    self.assertEqual(panel['fieldConfig']['defaults']['unit'], 'Bps')
                if 'truenas_pool_scan_progress_ratio' in expression:
                    self.assertEqual(panel['fieldConfig']['defaults']['unit'], 'percentunit')
                    self.assertTrue(expression.startswith('truenas_pool_scan_progress_ratio{'))

    def test_memory_alpha_overview_precedes_existing_details(self):
        panels = self.dashboard['panels']
        self.assertEqual(panels[0]['title'], 'MEMORY ALPHA overview')
        self.assertEqual(panels[0]['gridPos']['y'], 0)
        overview = panels[1:7]
        self.assertEqual([p['title'] for p in overview], [
            'HW: TrueNAS', 'IP address / uptime', 'CPU', 'Memory',
            'Pool used', 'Network · RX + TX'])
        for panel in overview:
            self.assertIn('target="memory-alpha"', panel['targets'][0]['expr'])
            self.assertTrue(panel['targets'][0]['instant'])
        details = panels[7:]
        self.assertEqual({p['id'] for p in details}, set(range(1, 41)))
        self.assertTrue(all(p['gridPos']['y'] >= 7 for p in details))
        self.assertEqual(overview[1]['fieldConfig']['defaults']['displayName'], '10.10.10.7')
        self.assertEqual(overview[1]['fieldConfig']['defaults']['unit'], 'dtdhms')
        self.assertNotIn('rate(', overview[-1]['targets'][0]['expr'])

    def test_office_desk_memory_alpha_controls_use_local_exporter(self):
        dashboard = json.loads((ROOT / 'dashboards/office-desk-memory-alpha.json').read_text())
        self.assertEqual(dashboard['apiVersion'], 'dashboard.grafana.app/v2')
        self.assertEqual(dashboard['metadata']['name'], 'dbfqrjypqhwirke')
        items = {v['spec']['element']['name']: v['spec']
                 for v in dashboard['spec']['layout']['spec']['items']}
        expected = ['CPU', 'MEMORY', 'POOL USED', 'NETWORK - RX + TX']
        for index, title in enumerate(expected, start=112):
            key = f'panel-{index}'
            panel = dashboard['spec']['elements'][key]['spec']
            self.assertEqual(panel['title'], title)
            self.assertEqual(items[key]['y'], 10)
            self.assertEqual(items[key]['x'], 8 + (index - 112) * 2)
            query = panel['data']['spec']['queries'][0]['spec']['query']
            self.assertEqual(query['datasource']['name'], 'prometheus')
            self.assertTrue(query['spec']['instant'])
            self.assertIn('target="memory-alpha"', query['spec']['expr'])
            self.assertNotIn('rate(', query['spec']['expr'])
        self.assertEqual(items['panel-100']['y'], 15)
        self.assertEqual(items['panel-100']['height'], 5)
