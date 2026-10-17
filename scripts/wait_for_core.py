"""Wait for the exact core release before starting app release builds."""
import argparse
import json
import os
import re
import time
import tomllib
import urllib.error
import urllib.request
from pathlib import Path


def required_version(manifest_path):
    manifest = tomllib.loads(Path(manifest_path).read_text())
    requirement = manifest["dependencies"]["synapt-core"]["version"]
    if not re.fullmatch(r"=\d+\.\d+\.\d+", requirement):
        raise ValueError("App releases must pin an exact stable SynaptCore version")
    linux = manifest["target"]['cfg(target_os = "linux")']["dependencies"]["synapt-core"]
    if linux["version"] != requirement:
        raise ValueError("Linux and default SynaptCore versions differ")
    return requirement[1:]


def release_ready(release, version):
    return (release.get("tag_name") == f"v{version}"
            and release.get("draft") is False
            and release.get("prerelease") is False
            and any(asset.get("name") == "core.json" for asset in release.get("assets", [])))


def wait_for_release(version, timeout):
    url = f"https://api.github.com/repos/aatishbagal/synapt-core/releases/tags/v{version}"
    headers = {"Accept": "application/vnd.github+json", "User-Agent": "synapt-release"}
    token = os.environ.get("GH_TOKEN")
    if token:
        headers["Authorization"] = f"Bearer {token}"
    deadline = time.monotonic() + timeout
    while True:
        try:
            with urllib.request.urlopen(urllib.request.Request(url, headers=headers), timeout=15) as response:
                if release_ready(json.load(response), version):
                    return
        except urllib.error.HTTPError as error:
            if error.code not in (404, 429, 500, 502, 503, 504):
                raise
        except (urllib.error.URLError, TimeoutError):
            pass
        remaining = deadline - time.monotonic()
        if remaining <= 0:
            raise TimeoutError(f"SynaptCore v{version} is not published; app release stopped")
        print(f"Waiting for SynaptCore v{version} to be published...", flush=True)
        time.sleep(min(20, remaining))


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest")
    parser.add_argument("--timeout", type=int, default=1200)
    args = parser.parse_args()
    version = required_version(args.manifest)
    wait_for_release(version, args.timeout)
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        output.write(f"ref=refs/tags/v{version}\n")


if __name__ == "__main__":
    main()
