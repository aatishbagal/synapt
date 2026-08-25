# Contributing to synapt

## Development setup

### Prerequisites

- Rust stable (install via rustup: https://rustup.rs)
- Node.js 18 or later
- Tauri CLI: `cargo install tauri-cli --version '^2'`
- Linux: `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`
- macOS: Xcode Command Line Tools (`xcode-select --install`)

### Setup

`synapt` depends on the `synapt-core` crate through a relative path
(`../../synapt-core`), so the two repositories must sit side by side.
`install.sh` clones it for you if it is not already there.

```bash
git clone https://github.com/aatishbagal/synapt.git
cd synapt
bash install.sh   # clones synapt-core alongside this repo if needed
npm install
```

### Running in development

```bash
RUST_LOG=info cargo tauri dev
```

### Running tests

```bash
cargo test
cargo clippy -- -D warnings
npm run build
```

All three must pass before submitting a pull request.

## Pull request guidelines

- Open an issue before starting work on a non-trivial change
- One logical change per pull request
- All tests must pass: `cargo test`, `cargo clippy -- -D warnings`, `npm run build`
- No emojis in code, comments, strings, or documentation
- Commit messages follow the format: `type(scope): description`
  Valid types: feat, fix, chore, docs, refactor, test
- Prefer small modules. If a new module grows past roughly 300 lines, consider
  splitting it. Some existing modules are considerably larger; treat those as
  debt to be paid down, not as a pattern to copy.
- No `unwrap()` or `expect()` in non-test code

## Commit message format

```
feat(network): add peer discovery retry on startup
fix(macos): clear WKWebView background for transparent corners
chore(release): v0.5.1
docs(readme): add Linux Wayland setup instructions
```

## Architecture overview

See the README for the high-level architecture. Key modules:

- `src-tauri/src/network/` - peer discovery, pairing, transfer, search server
- `src-tauri/src/search/` - file indexer, tantivy, Trie, Bloom filter, fuzzy
- `src-tauri/src/ipc/` - SynaptClip integration API (serves loopback port 57321)
- `src-tauri/src/trust/` - device identity and trusted peer store
- `src-tauri/src/storage/` - SQLite database and migrations
- `src-tauri/src/platform/` - platform-specific setup (macOS, Linux, Windows)
- `src/` - React and TypeScript frontend
