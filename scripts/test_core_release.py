import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import urllib.error

import wait_for_core


class CoreReleaseTests(unittest.TestCase):
    def published(self, version='0.2.1'):
        return {'tag_name': f'v{version}', 'draft': False, 'prerelease': False,
                'assets': [{'name': 'core.json'}]}

    def manifest_copy(self, tmp, manifest_text=None, lock_text=None):
        src = Path(__file__).resolve().parents[1] / 'src-tauri'
        manifest = Path(tmp) / 'Cargo.toml'
        manifest.write_text(manifest_text if manifest_text is not None else (src / 'Cargo.toml').read_text())
        (Path(tmp) / 'Cargo.lock').write_text(lock_text if lock_text is not None else (src / 'Cargo.lock').read_text())
        return manifest

    def test_current_manifest_resolves_a_compatible_locked_core(self):
        manifest = Path(__file__).resolve().parents[1] / 'src-tauri/Cargo.toml'
        version = wait_for_core.release_core_version(manifest)
        self.assertTrue(wait_for_core.caret_allows('0.2.2', version), version)

    def test_exact_unbounded_or_mismatched_requirements_are_rejected(self):
        source = (Path(__file__).resolve().parents[1] / 'src-tauri/Cargo.toml').read_text()
        for replacement in ['=0.2.2', '>=0.2.2', '*', '0.2.2', '^0.3.0']:
            with tempfile.TemporaryDirectory() as tmp:
                manifest = self.manifest_copy(
                    tmp, source.replace('version = "^0.2.2"', f'version = "{replacement}"', 1))
                with self.assertRaises(ValueError):
                    wait_for_core.release_core_version(manifest)

    def test_locked_core_must_satisfy_the_requirement(self):
        lock = (Path(__file__).resolve().parents[1] / 'src-tauri/Cargo.lock').read_text()
        current = wait_for_core.release_core_version(
            Path(__file__).resolve().parents[1] / 'src-tauri/Cargo.toml')
        entry = f'name = "synapt-core"\nversion = "{current}"'
        self.assertIn(entry, lock)
        for locked in ['0.2.1', '0.3.0', '0.2.9-beta.1']:
            with tempfile.TemporaryDirectory() as tmp:
                manifest = self.manifest_copy(
                    tmp, lock_text=lock.replace(entry, f'name = "synapt-core"\nversion = "{locked}"'))
                with self.assertRaises(ValueError):
                    wait_for_core.release_core_version(manifest)
        with tempfile.TemporaryDirectory() as tmp:
            manifest = self.manifest_copy(tmp, lock_text=lock.replace(entry, 'name = "other"\nversion = "1.0.0"'))
            with self.assertRaises(ValueError):
                wait_for_core.release_core_version(manifest)

    def test_caret_ranges_follow_cargo_rules(self):
        self.assertTrue(wait_for_core.caret_allows('0.2.2', '0.2.2'))
        self.assertTrue(wait_for_core.caret_allows('0.2.2', '0.2.9'))
        self.assertFalse(wait_for_core.caret_allows('0.2.2', '0.2.1'))
        self.assertFalse(wait_for_core.caret_allows('0.2.2', '0.3.0'))
        self.assertTrue(wait_for_core.caret_allows('1.2.0', '1.9.0'))
        self.assertFalse(wait_for_core.caret_allows('1.2.0', '2.0.0'))
        self.assertFalse(wait_for_core.caret_allows('0.0.3', '0.0.4'))

    def test_draft_prerelease_wrong_tag_or_missing_metadata_cannot_start_app_release(self):
        self.assertTrue(wait_for_core.release_ready(self.published(), '0.2.1'))
        for field, value in [('draft', True), ('prerelease', True), ('tag_name', 'v0.1.0'), ('assets', [])]:
            release = self.published()
            release[field] = value
            self.assertFalse(wait_for_core.release_ready(release, '0.2.1'))

    @patch('wait_for_core.time.sleep')
    @patch('wait_for_core.urllib.request.urlopen')
    def test_waits_for_core_publication_when_main_merges_race(self, request, sleep):
        request.side_effect = [urllib.error.HTTPError('url', 404, 'not yet', {}, None),
                               io.BytesIO(json.dumps(self.published()).encode())]
        wait_for_core.wait_for_release('0.2.1', 60)
        self.assertEqual(request.call_count, 2)
        sleep.assert_called_once()

    @patch('wait_for_core.urllib.request.urlopen')
    def test_timeout_stops_release_instead_of_using_an_old_core(self, request):
        request.return_value = io.BytesIO(json.dumps(self.published('0.1.0')).encode())
        with self.assertRaises(TimeoutError):
            wait_for_core.wait_for_release('0.2.1', 0)

    @patch('wait_for_core.urllib.request.urlopen')
    def test_authentication_failure_is_reported_immediately(self, request):
        request.side_effect = urllib.error.HTTPError('url', 403, 'forbidden', {}, None)
        with self.assertRaises(urllib.error.HTTPError):
            wait_for_core.wait_for_release('0.2.1', 60)


if __name__ == '__main__':
    unittest.main()
