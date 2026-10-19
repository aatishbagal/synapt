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


def caret_allows(requirement, version):
    """Whether a stable version satisfies a Cargo caret requirement like ^0.2.2."""
    want = tuple(int(part) for part in requirement.split("."))
    have = tuple(int(part) for part in version.split("."))
    if have < want:
        return False
    if want[0] > 0:
        return have[0] == want[0]
    if want[1] > 0:
        return have[:2] == want[:2]
    return have == want


def release_core_version(manifest_path):
    """The exact SynaptCore version an app release builds against.

    Cargo.toml declares a caret range so compatible core patches need no
    manifest change; Cargo.lock records the one version actually resolved,
    which is the release the app must wait for and check out.
    """
    manifest_path = Path(manifest_path)
    manifest = tomllib.loads(manifest_path.read_text())
    requirement = manifest["dependencies"]["synapt-core"]["version"]
    if not re.fullmatch(r"\^\d+\.\d+\.\d+", requirement):
        raise ValueError("SynaptCore must be required with a caret range such as ^0.2.2")
    linux = manifest["target"]['cfg(target_os = "linux")']["dependencies"]["synapt-core"]
    if linux["version"] != requirement:
        raise ValueError("Linux and default SynaptCore versions differ")

    lock = tomllib.loads((manifest_path.parent / "Cargo.lock").read_text())
    locked = [pkg["version"] for pkg in lock.get("package", [])
              if pkg.get("name") == "synapt-core" and "source" not in pkg]
    if len(locked) != 1:
        raise ValueError("Cargo.lock must contain exactly one local synapt-core entry")
    version = locked[0]
    if not re.fullmatch(r"\d+\.\d+\.\d+", version):
        raise ValueError(f"Locked SynaptCore {version} is not a stable release")
    if not caret_allows(requirement[1:], version):
        raise ValueError(f"Locked SynaptCore {version} does not satisfy {requirement}")
    return version


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
    version = release_core_version(args.manifest)
    wait_for_release(version, args.timeout)
    with open(os.environ["GITHUB_OUTPUT"], "a") as output:
        output.write(f"ref=refs/tags/v{version}\n")


if __name__ == "__main__":
    main()
