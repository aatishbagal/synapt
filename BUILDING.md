# Building Synapt

## Prerequisites

- Rust stable (install via rustup)
- Node.js 18+
- Tauri CLI: `cargo install tauri-cli --version '^2'`
- Linux only: `libwebkit2gtk-4.1-dev`, `libayatana-appindicator3-dev`, `librsvg2-dev`

## Setup

```bash
bash install.sh
cd synapt
npm install
```

## Development

```bash
cargo tauri dev
```

## Release build

```bash
cargo tauri build
```

The bundled installer is written to `src-tauri/target/release/bundle/`.

## First run

On first launch, Synapt creates its database at the platform data directory:
- Linux: `~/.local/share/synapt/synapt.db`
- Windows: `%APPDATA%\synapt\synapt.db`
- macOS: `~/Library/Application Support/synapt/synapt.db`

Add at least one directory to **Indexed Directories** in Settings before searching.
