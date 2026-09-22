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
            report = root / 'docs/reports/pack.html'
            report.parent.mkdir(parents=True)
            before = '<!doctype html>\n<main\n id="content">Released evidence &amp; links</main>'
            report.write_text(before)
            alias = report.with_name('README.html')
            alias.write_text('<a href="index.html">Old index alias</a>')
            unrelated = root / 'index.html'
            unrelated.write_text('<main>Released landing</main>')
            composer.link_latest_evaluations(root)
            after = report.read_text()
            self.assertIn('Released evidence &amp; links</main>', after)
            # Removing only the inserted aside recovers every original chapter byte.
            start = after.index('\n<aside ')
            end = after.index('</aside>\n', start) + len('</aside>\n')
            self.assertEqual(after[:start] + after[end:], before)
            self.assertEqual(unrelated.read_text(), '<main>Released landing</main>')
            self.assertEqual(alias.read_text(), '<a href="index.html">Old index alias</a>')
            self.assertEqual(current.read_text(), '<main>Current evidence</main>')
            for page in (report, root / 'evaluations/index.html'):
                links = Links(page.read_text()).hrefs
                self.assertEqual(len(links), 1)
                self.assertEqual((page.parent / links[0]).resolve(), current.resolve())

    def test_refuses_broken_latest_target_before_mutation(self):
        with tempfile.TemporaryDirectory() as directory:
            root = Path(directory)
            with self.assertRaisesRegex(ValueError, 'development report index'):
                composer.link_latest_evaluations(root)
            self.assertFalse((root / 'evaluations').exists())


if __name__ == '__main__':
    unittest.main()
