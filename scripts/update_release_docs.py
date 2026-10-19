"""Backfill whatsnew.json and CHANGELOG.md for a release that shipped without them.

Release notes should be written by hand in the release PR, because the app bundles
whatsnew.json at build time. This script only fills the gap afterwards, so the repo
docs never skip a version. Entries that already exist are left untouched.
"""
import argparse
import json
import re
from pathlib import Path

CHANGELOG_SECTIONS = ('Added', 'Changed', 'Fixed', 'Removed', 'Security')
DEFAULT_HEADLINE = 'See the full changelog for details.'


def release_bullets(body):
    """Return every bullet line of a release body, without its marker."""
    bullets = []
    for line in body.splitlines():
        match = re.match(r'^[-*] +(.+)$', line.strip())
        if match:
            bullets.append(match.group(1).strip())
    return bullets


def changelog_section(entry):
    """Pick the Keep a Changelog section for one bullet from its leading verb."""
    word = entry.split(maxsplit=1)[0].lower() if entry else ''
    if word.startswith('add') or word.startswith('new'):
        return 'Added'
    if word.startswith('fix'):
        return 'Fixed'
    if word.startswith('remov') or word.startswith('drop'):
        return 'Removed'
    if word.startswith('secur'):
        return 'Security'
    return 'Changed'


def update_whatsnew(path, version, date, bullets):
    """Prepend an entry for version unless one exists. Returns True if the file changed."""
    data = json.loads(path.read_text())
    if any(v.get('version') == version for v in data['versions']):
        return False
    data['versions'].insert(0, {
        'version': version,
        'date': date,
        'headline': DEFAULT_HEADLINE,
        'highlights': bullets[:3],
    })
    path.write_text(json.dumps(data, indent=2) + '\n')
    return True


def changelog_block(version, date, bullets, release_url):
    """Render one version block, grouped into Keep a Changelog sections."""
    grouped = {name: [] for name in CHANGELOG_SECTIONS}
    for bullet in bullets:
        grouped[changelog_section(bullet)].append(bullet)
    if not bullets:
        grouped['Changed'].append(f'See the release notes: {release_url}')
    lines = [f'## [{version}] - {date}']
    for name in CHANGELOG_SECTIONS:
        if grouped[name]:
            lines.append(f'### {name}')
            lines.extend(f'- {item}' for item in grouped[name])
    return '\n'.join(lines) + '\n'


def update_changelog(path, version, date, bullets, release_url):
    """Insert a block for version above the newest one unless it exists. Returns True if changed."""
    text = path.read_text()
    if re.search(rf'^## \[{re.escape(version)}\]', text, re.MULTILINE):
        return False
    block = changelog_block(version, date, bullets, release_url)
    first = re.search(r'^## \[', text, re.MULTILINE)
    if first:
        text = text[:first.start()] + block + '\n' + text[first.start():]
    else:
        text = text.rstrip('\n') + '\n\n' + block
    path.write_text(text)
    return True


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('version', help='version without the leading v, e.g. 0.6.0')
    parser.add_argument('date', help='release date as YYYY-MM-DD')
    parser.add_argument('body', type=Path, help='file holding the GitHub release body')
    parser.add_argument('--repo', required=True, help='owner/name, used for the release link')
    parser.add_argument('--root', type=Path, default=Path('.'), help='repository root')
    args = parser.parse_args()

    bullets = release_bullets(args.body.read_text() if args.body.exists() else '')
    release_url = f'https://github.com/{args.repo}/releases/tag/v{args.version}'
    changed_whatsnew = update_whatsnew(args.root / 'whatsnew.json', args.version, args.date, bullets)
    changed_changelog = update_changelog(
        args.root / 'CHANGELOG.md', args.version, args.date, bullets, release_url)
    print(f'whatsnew.json: {"added" if changed_whatsnew else "already has"} v{args.version}')
    print(f'CHANGELOG.md: {"added" if changed_changelog else "already has"} v{args.version}')


if __name__ == '__main__':
    main()
