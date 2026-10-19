import json
from pathlib import Path
import tempfile
import unittest

import update_release_docs as docs

HEADER = (
    '# Changelog\n'
    'All notable changes to this project are documented here.\n'
    'Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).\n\n'
)
URL = 'https://github.com/owner/repo/releases/tag/v0.6.0'


class ReleaseDocsTests(unittest.TestCase):
    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = Path(self.tmp.name)
        (self.root / 'whatsnew.json').write_text(json.dumps({'versions': [
            {'version': '0.5.4', 'date': '2026-10-17', 'headline': 'h', 'highlights': []},
        ]}))
        (self.root / 'CHANGELOG.md').write_text(HEADER + '## [0.5.4] - 2026-10-17\n### Fixed\n- x\n')

    def tearDown(self):
        self.tmp.cleanup()

    def test_bullets_are_read_from_release_body(self):
        body = 'Intro line\n- Added one\n* Fixed two\n  - Nested three\nnot a bullet\n'
        self.assertEqual(docs.release_bullets(body), ['Added one', 'Fixed two', 'Nested three'])

    def test_new_version_is_prepended_to_whatsnew_with_three_highlights(self):
        path = self.root / 'whatsnew.json'
        self.assertTrue(docs.update_whatsnew(path, '0.6.0', '2026-11-01', ['a', 'b', 'c', 'd']))
        versions = json.loads(path.read_text())['versions']
        self.assertEqual([v['version'] for v in versions], ['0.6.0', '0.5.4'])
        self.assertEqual(versions[0]['highlights'], ['a', 'b', 'c'])

    def test_existing_whatsnew_entry_is_left_alone(self):
        path = self.root / 'whatsnew.json'
        before = path.read_text()
        self.assertFalse(docs.update_whatsnew(path, '0.5.4', '2026-11-01', ['new']))
        self.assertEqual(path.read_text(), before)

    def test_changelog_block_goes_below_header_and_above_newest_version(self):
        path = self.root / 'CHANGELOG.md'
        self.assertTrue(docs.update_changelog(
            path, '0.6.0', '2026-11-01', ['Added thing', 'Fixed bug', 'Tweaked UI'], URL))
        text = path.read_text()
        self.assertTrue(text.startswith(HEADER + '## [0.6.0] - 2026-11-01\n'))
        self.assertLess(text.index('## [0.6.0]'), text.index('## [0.5.4]'))
        block = text[text.index('## [0.6.0]'):text.index('## [0.5.4]')]
        self.assertEqual(block, (
            '## [0.6.0] - 2026-11-01\n### Added\n- Added thing\n'
            '### Changed\n- Tweaked UI\n### Fixed\n- Fixed bug\n\n'))

    def test_empty_release_body_links_to_release_notes(self):
        block = docs.changelog_block('0.6.0', '2026-11-01', [], URL)
        self.assertEqual(block, f'## [0.6.0] - 2026-11-01\n### Changed\n- See the release notes: {URL}\n')

    def test_existing_changelog_version_is_left_alone(self):
        path = self.root / 'CHANGELOG.md'
        before = path.read_text()
        self.assertFalse(docs.update_changelog(path, '0.5.4', '2026-11-01', ['x'], URL))
        self.assertEqual(path.read_text(), before)

    def test_real_repo_files_are_parseable(self):
        root = Path(__file__).resolve().parents[1]
        versions = json.loads((root / 'whatsnew.json').read_text())['versions']
        self.assertTrue(versions)
        self.assertIn(f"## [{versions[0]['version']}]", (root / 'CHANGELOG.md').read_text())


if __name__ == '__main__':
    unittest.main()
