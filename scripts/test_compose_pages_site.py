#!/usr/bin/env python3
"""Behavioral checks for latest-report navigation around released snapshots."""
import importlib.util
import tempfile
import unittest
from html.parser import HTMLParser
from pathlib import Path

SPEC = importlib.util.spec_from_file_location('compose_pages_site', Path(__file__).with_name('compose-pages-site.py'))
assert SPEC and SPEC.loader
composer = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(composer)


class Links(HTMLParser):
    def __init__(self, text):
        super().__init__()
        self.hrefs = []
        self.feed(text)

    def handle_starttag(self, tag, attrs):
        if tag == 'a':
            self.hrefs.extend(value for key, value in attrs if key == 'href')


class LatestEvaluationsTests(unittest.TestCase):
    def test_latest_route_and_notice_preserve_release_chapter(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            current = root / 'dev/docs/reports/index.html'
            current.parent.mkdir(parents=True)
            current.write_text('<main>Current evidence</main>')
            reports_dir = root / 'docs/reports'
            reports_dir.mkdir(parents=True)
            originals = {
                reports_dir / 'pack.html': '<!doctype html>\n<main\n id="content">Released evidence &amp; links</main>',
                reports_dir / 'unicode-pack.html': '<html><nav>one\u2028two\u2029three\x85four\vfifth\fsixth</nav>\n<main\n id="content">Unicode separator evidence.</main></html>',
                reports_dir / 'second-pack.html': '<html><main class="chapter"><h1>Second report</h1><p>Different release findings.</p></main></html>',
            }
            for report, before in originals.items():
                report.write_text(before, encoding="utf-8")
            alias = reports_dir / 'README.html'
            alias.write_text('<a href="index.html">Old index alias</a>')
            unrelated = root / 'index.html'
            unrelated.write_text('<main>Released landing</main>')
            composer.link_latest_evaluations(root)
            for report, before in originals.items():
                after = report.read_text(encoding="utf-8")
                self.assertEqual(after.count('<aside class="warning"'), 1)
                self.assertIn('This is a release snapshot.', after)
                self.assertIn('>\n<aside class="warning"', after)
                self.assertEqual(after.index('\n<aside '), before.index('>', before.index('<main')) + 1)
                # Removing only the inserted aside recovers every original chapter byte.
                start = after.index('\n<aside ')
                end = after.index('</aside>\n', start) + len('</aside>\n')
                self.assertEqual(after[:start] + after[end:], before)
                links = Links(after).hrefs
                self.assertEqual(len(links), 1)
                self.assertEqual((report.parent / links[0]).resolve(), current.resolve())
            self.assertEqual(unrelated.read_text(), '<main>Released landing</main>')
            self.assertEqual(alias.read_text(), '<a href="index.html">Old index alias</a>')
            self.assertEqual(current.read_text(), '<main>Current evidence</main>')
            latest = root / 'evaluations/index.html'
            links = Links(latest.read_text()).hrefs
            self.assertEqual(len(links), 1)
            self.assertEqual((latest.parent / links[0]).resolve(), current.resolve())

    def test_reserved_route_collision_preserves_release_content(self):
        for kind in ('file', 'directory', 'dangling-symlink'):
            with self.subTest(kind=kind), tempfile.TemporaryDirectory() as directory:
                root = Path(directory)
                current = root / 'dev/docs/reports/index.html'
                current.parent.mkdir(parents=True)
                current.write_text('<main>Current evidence</main>')
                report = root / 'docs/reports/pack.html'
                report.parent.mkdir(parents=True)
                original = b'<main>Released evidence</main>'
                report.write_bytes(original)
                route = root / 'evaluations'
                if kind == 'directory':
                    route.mkdir()
                    retained = route / 'retained.html'
                    retained.write_bytes(b'Released route')
                elif kind == 'file':
                    route.write_bytes(b'Released route')
                else:
                    try:
                        route.symlink_to(root / 'missing')
                    except OSError:
                        continue  # Windows may not grant symlink creation privileges.
                with self.assertRaisesRegex(ValueError, 'reserved latest-evaluations route already exists'):
                    composer.link_latest_evaluations(root)
                self.assertEqual(report.read_bytes(), original)
                if kind == 'directory':
                    self.assertEqual(retained.read_bytes(), b'Released route')
                elif kind == 'file':
                    self.assertEqual(route.read_bytes(), b'Released route')
                else:
                    self.assertTrue(route.is_symlink())
                    self.assertEqual(route.readlink(), root / 'missing')

    def test_refuses_broken_latest_target_before_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaisesRegex(ValueError, 'development report index'):
                composer.link_latest_evaluations(root)
            self.assertFalse((root / 'evaluations').exists())


if __name__ == '__main__':
    unittest.main()
