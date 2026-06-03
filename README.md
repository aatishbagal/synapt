# Synapt

<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="./assets/images/logo/png/SynaptV2_White_PNG_512sq.png">
    <img src="./assets/images/logo/png/SynaptV2_Black_PNG_512sq.png" alt="Synapt" width="120">
  </picture>
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

## Security

Synapt uses X25519 Elliptic Curve Diffie-Hellman for key exchange, ChaCha20-Poly1305 for all network traffic, and HKDF-SHA256 for key derivation. Device pairing uses a code-verification ceremony to detect man-in-the-middle attacks. Trusted peers can only access directories you explicitly add to the shared list. Trust can be revoked at any time from Settings.

## Related

- [synapt-clip](https://github.com/aatishbagal/synapt-clip) — clipboard manager with optional cross-device clipboard sharing
- [synapt-core](https://github.com/aatishbagal/synapt-core) — shared type library used across the Synapt apps

## License

Apache License 2.0 — see [LICENSE](./LICENSE).
