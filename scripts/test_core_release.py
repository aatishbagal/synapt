import io
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch
import urllib.error

import wait_for_core


class CoreReleaseTests(unittest.TestCase):
    def published(self, version='0.2.0'):
        return {'tag_name': f'v{version}', 'draft': False, 'prerelease': False,
                'assets': [{'name': 'core.json'}]}

    def test_current_manifest_pins_expected_core_for_both_targets(self):
        manifest = Path(__file__).resolve().parents[1] / 'src-tauri/Cargo.toml'
        self.assertEqual(wait_for_core.required_version(manifest), '0.2.0')

    def test_unpinned_or_mismatched_dependencies_are_rejected(self):
        source = (Path(__file__).resolve().parents[1] / 'src-tauri/Cargo.toml').read_text()
        for replacement in ['^0.2.0', '=0.3.0']:
            with tempfile.TemporaryDirectory() as tmp:
                manifest = Path(tmp) / 'Cargo.toml'
                manifest.write_text(source.replace('version = "=0.2.0"', f'version = "{replacement}"', 1))
                with self.assertRaises(ValueError):
                    wait_for_core.required_version(manifest)

    def test_draft_prerelease_wrong_tag_or_missing_metadata_cannot_start_app_release(self):
        self.assertTrue(wait_for_core.release_ready(self.published(), '0.2.0'))
        for field, value in [('draft', True), ('prerelease', True), ('tag_name', 'v0.1.0'), ('assets', [])]:
            release = self.published()
            release[field] = value
            self.assertFalse(wait_for_core.release_ready(release, '0.2.0'))

    @patch('wait_for_core.time.sleep')
    @patch('wait_for_core.urllib.request.urlopen')
    def test_waits_for_core_publication_when_main_merges_race(self, request, sleep):
        request.side_effect = [urllib.error.HTTPError('url', 404, 'not yet', {}, None),
                               io.BytesIO(json.dumps(self.published()).encode())]
        wait_for_core.wait_for_release('0.2.0', 60)
        self.assertEqual(request.call_count, 2)
        sleep.assert_called_once()

    @patch('wait_for_core.urllib.request.urlopen')
    def test_timeout_stops_release_instead_of_using_an_old_core(self, request):
        request.return_value = io.BytesIO(json.dumps(self.published('0.1.0')).encode())
        with self.assertRaises(TimeoutError):
            wait_for_core.wait_for_release('0.2.0', 0)

    @patch('wait_for_core.urllib.request.urlopen')
    def test_authentication_failure_is_reported_immediately(self, request):
        request.side_effect = urllib.error.HTTPError('url', 403, 'forbidden', {}, None)
        with self.assertRaises(urllib.error.HTTPError):
            wait_for_core.wait_for_release('0.2.0', 60)


if __name__ == '__main__':
    unittest.main()
