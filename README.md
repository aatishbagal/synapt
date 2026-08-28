# Synapt

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./assets/images/logo/png/SynaptV2_White_PNG_512sq.png">
    <img src="./assets/images/logo/png/SynaptV2_Black_PNG_512sq.png" alt="Synapt" width="120">
  </picture>
</p>

<p align="center">
  <a href="https://github.com/aatishbagal/synapt/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/aatishbagal/synapt?label=release&color=3b82f6"></a>
  <a href="https://github.com/aatishbagal/synapt/actions/workflows/ci.yml"><img alt="CI status" src="https://img.shields.io/github/actions/workflow/status/aatishbagal/synapt/ci.yml?label=CI"></a>
  <a href="https://github.com/aatishbagal/synapt/releases"><img alt="Total downloads" src="https://img.shields.io/github/downloads/aatishbagal/synapt/total?label=downloads&color=3b82f6"></a>
  <a href="./LICENSE"><img alt="License" src="https://img.shields.io/github/license/aatishbagal/synapt?label=license"></a>
</p>

Synapt is a Spotlight-style launcher and LAN file utility — summon it with a global hotkey, search your files, and transfer them to trusted devices on your local network.

## Features

- Global hotkey overlay (default Ctrl+Space) — transparent, always-on-top launcher
- Local file search: prefix matching, full-text, and fuzzy fallback
- Remote search across trusted peer devices on the same network
- Encrypted peer-to-peer file transfer — no cloud, no account required
- Device pairing with code verification (TOFU trust model)
- Input modes: bare query (file search), /prefix (folders only), @device (remote search), @settings (open settings), arithmetic (inline calculator)
- Shared directory allow-list — peers only access what you explicitly share
- Persistent file index in SQLite, tantivy full-text index rebuilt on startup
- System tray icon with show/hide
- SynaptClip integration — cross-device clipboard sync via local API when SynaptClip is installed
- Linux (X11, Wayland GNOME, Wayland wlroots), Windows, macOS

## How It Works

**Discovery and pairing.** Synapt uses UDP multicast to find other devices running Synapt on the local network. First contact requires a pairing ceremony: both devices perform an X25519 Diffie-Hellman key exchange and display a verification code derived from the shared secret. If the codes match on both screens, the devices are paired and their long-term public keys are stored. All subsequent communication is encrypted with ChaCha20-Poly1305.

**Search.** The overlay indexes your local file system into SQLite and a tantivy full-text index on first run. Queries pass through a Bloom filter pre-check, a frequency-weighted prefix Trie, tantivy full-text search, and a Jaro-Winkler/Levenshtein fuzzy fallback. Remote search sends an encrypted query to a trusted peer and shows their results inline, tagged with the device name.

**Transfer.** File requests go over an encrypted TCP channel. The receiving device verifies the requested path is inside a shared directory before streaming. Transfers are chunked, progress-tracked, and resumable on reconnect.

## Usage

| Input | Result |
|---|---|
| `report` | Search local files for "report" |
| `/documents` | Search within directories only |
| `@alice-laptop report` | Search alice-laptop's shared files for "report" |
| `@settings` | Open settings |
| `2 * (3 + 4)` | Evaluate arithmetic inline |
| Ctrl+Space | Show / hide the overlay |
| Escape | Dismiss the overlay |
| Up / Down | Navigate results |
| Enter | Open or copy selected result |

## Installation

### Requirements

- Linux: libwebkit2gtk, libayatana-appindicator (system tray on GNOME)
- Windows: WebView2 runtime (bundled in installer)
- macOS: macOS 13+

### Build from source

```bash
git clone https://github.com/aatishbagal/synapt.git
cd synapt
bash install.sh   # clones synapt-core into the correct relative path

# Development
cargo tauri dev

# Release build
cargo tauri build
```

### GNOME Wayland — system tray

GNOME does not support system trays by default. Install the [AppIndicator and KStatusNotifierItem Support](https://extensions.gnome.org/extension/615/appindicator-support/) GNOME Shell extension.

### GNOME Wayland — global hotkey

The global hotkey (Ctrl+Space) does not fire under GNOME on Wayland. The shortcut registers without error, but GNOME's Wayland compositor does not deliver globally-grabbed key combinations to applications, so the keypress never reaches Synapt. This is a limitation of the underlying global-shortcut implementation, not a configuration issue.

Until native Wayland support lands (planned for v0.5, via the XDG Desktop Portal GlobalShortcuts interface), use a **GNOME on Xorg** session, where the hotkey works normally. Pick "GNOME on Xorg" from the gear menu on the GDM login screen. The hotkey also works as expected on Windows, macOS, and X11 Linux sessions.

## Security

Synapt uses X25519 Elliptic Curve Diffie-Hellman for key exchange, ChaCha20-Poly1305 for all network traffic, and HKDF-SHA256 for key derivation. Device pairing uses a code-verification ceremony to detect man-in-the-middle attacks. Trusted peers can only access directories you explicitly add to the shared list. Trust can be revoked at any time from Settings.

## SynaptClip Integration

When SynaptClip is installed and running on the same machine, Synapt exposes a local HTTP API on port 57321 that enables cross-device clipboard sync.

### What it enables

- Send clipboard entries from SynaptClip to any trusted peer device
- Receive clipboard entries from a peer's SynaptClip and have them appear in the local clip history
- SynaptClip's panel shows a Devices section listing trusted peers when Synapt is running

### How it works

Synapt acts as the network transport. SynaptClip acts as the clipboard UI. Neither app depends on the other at build time. If Synapt is not running, SynaptClip works normally with no error. If SynaptClip is not installed, Synapt's file transfer and search features are unaffected.

When SynaptClip sends a clip to a peer, it calls Synapt's local API. Synapt wraps the content as a temporary file and transfers it using its existing encrypted P2P transfer layer. When the clip arrives on the peer device, Synapt forwards it to the local SynaptClip instance via a webhook on port 57322.

### Setup

1. Install and run Synapt on both devices.
2. Pair the devices using the @ device picker in the Synapt overlay.
3. Install SynaptClip on both devices.
4. Launch SynaptClip. It detects Synapt automatically.

No additional configuration is needed.

### API

The integration API is documented in [api-contract.md](https://github.com/aatishbagal/synapt-clip/blob/main/references/api-contract.md).

## Related

- [synapt-clip](https://github.com/aatishbagal/synapt-clip) — clipboard manager with optional cross-device clipboard sharing
- [synapt-core](https://github.com/aatishbagal/synapt-core) — shared type library used across the Synapt apps

## License

Copyright 2026 Aatish Bagal

Licensed under the Apache License, Version 2.0. See [LICENSE](./LICENSE) for the full text.
