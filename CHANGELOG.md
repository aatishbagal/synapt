# Changelog
All notable changes to this project are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.6.0] - 2026-10-19
### Added
- Design system (DESIGN.md) with 8px grid, Radix/shadcn aesthetic, and token standardisation
- Motion (Framer Motion) spring animations for overlay entrance, panels, and dropdowns
- CSS cubic-bezier micro-interaction curves on all interactive elements
- First-launch onboarding flow for both apps
- Crash detection popup with opt-in mailto email report
- What is new modal shown once per version after an update
- Experimental UI flag in Settings with app restart prompt
### Fixed
- All Nielsen usability heuristic and WCAG 2.1 AA contrast findings
- Inconsistent spacing, colour usage, and interactive states across components

## [0.5.4] - 2026-10-17
### Added
- Updates section shows the bundled SynaptCore version, compatibility warnings, and check errors
### Changed
- Bundles SynaptCore 0.2.1; release builds wait for the matching SynaptCore release
### Fixed
- Global hotkey now fires correctly on GNOME Wayland via the xdg-desktop-portal GlobalShortcuts interface
- Clipboard transfer from macOS to Linux no longer fails with access denied

## [0.5.3] - 2026-10-16
### Added
- Manual device pairing via IP address entry
- Invite code generation and redemption for device pairing
### Fixed
- /v1/peers endpoint now returns only trusted and online peers

## [0.5.2] - 2026-08-28
### Added
- Auto-updater with signed update endpoint for all platforms
- Automatic update check preference and reworked Updates section in Settings
- Crash log written to platform data directory on panic
### Fixed
- Reduced memory usage by packing the Bloom filter into a bit vector instead of Vec<bool>
- Reduced memory usage by storing Trie children in a sorted Vec instead of a HashMap
- Reduced tantivy writer heap from 50MB to 15MB
- Decoded app icons are no longer leaked; cached icons are reused until the app bundle changes
- Incoming pair request system notification fires even when the overlay is hidden

## [0.5.1] - 2026-08-25
### Added
- NSAlert confirmation dialog when quitting from the tray on macOS
- PRIVACY.md
- CONTRIBUTING.md, SECURITY.md, and GitHub issue templates
### Fixed
- Cmd+Q and the close button are suppressed on macOS; quit is only available from the tray menu
- Apache 2.0 copyright format corrected to 'Copyright 2026 Aatish Bagal'

## [0.5.0] - 2026-08-21
### Added
- SynaptClip integration API on port 57321 (GET /v1/health, GET /v1/peers, POST /v1/clips/send)
- Outbound webhook delivers received clips to SynaptClip on port 57322
- SynaptClip bridge status in Settings and the overlay header
- macOS universal build with signing and notarization support
- macOS application icons extracted via NSWorkspace
- New app icon built with Icon Composer; tray icon stays transparent
### Changed
- SynaptClip status dot replaced with connected-state text
### Fixed
- Transparent webview corners on macOS

## [0.4.2] - 2026-06-05
### Added
- @ device picker in the overlay replaces the bottom devices tab
- Remote search against the device selected via @
- Remote folder search
- Transfer progress card and loading bar below the search bar
- Right arrow on a file result opens an actions panel (open, reveal, send to devices)
- Multi-device file send with device multi-select
- Remote application launch with keyboard confirmation
- Remote application results show their icons
- Transfer history page with open-in-files
- Idle tips in the overlay
### Changed
- Startup skips redundant index rescans
### Fixed
- Arrow key navigation now moves through all results instead of stopping at the second
- Keyboard navigation auto-scrolls the selected result into view
- Application icon extraction on Windows

## [0.4.1] - 2026-06-04
### Added
- Indexing progress banner with progress bar during file and app scan
- App icons displayed alongside application results in the overlay
### Changed
- Indexing indicator is now a looping underline loader
- Theme selector removed from the overlay
### Fixed
- begin_pairing_cmd argument casing mismatch between frontend and backend
- Duplicate tray icon on Windows
- Theme dropdown uses a themed Select component instead of the native control

## [0.4.0] - 2026-06-04
### Added
- Complete Settings page with all sections, hotkey rebinding, and file pickers
- Dark, light, and system theme with CSS variables and runtime switching
- System notifications for transfer complete, peer paired, and peer online
- Auto-start on login for Linux, Windows, and macOS
- Install scripts (install.sh and install.ps1)
- IPC server stub on port 57321 for SynaptClip integration
- Application indexing on Linux, Windows, and macOS
- Substring search via tantivy regex queries
- Star syntax (*query*) for explicit contains search
### Changed
- Overlay dismisses on focus loss, matching SynaptClip

## [0.3.1] - 2026-06-03
### Added
- Single system tray icon shared with SynaptClip, hosted by whichever app launches first
- Automated release workflow
### Changed
- Synapt UI standardised to match SynaptClip visual style
### Fixed
- File indexer correctly populates SQLite and the tantivy index
- Search pipeline wired end-to-end so results appear in the overlay
- Tray icon uses the correct Synapt logo

## [0.3.0]
### Added
- Chunked file transfer with 64KB chunk size
- Transfer resume on reconnect
- Multi-file batch transfer with sequential streaming and per-file progress
- Transfer queue with in-memory state tracking and a queue panel in Settings
- macOS build with system tray and global hotkey

## [0.2.0]
### Added
- Local file search with tantivy full-text index
- Custom Trie with frequency weighting and autocompletion
- Custom Bloom filter as pre-filter gate in the search pipeline
- Jaro-Winkler and Levenshtein fuzzy matcher
- LRU cache for recent query results
- Overlay input modes: bare query, /folder, @device, and arithmetic calculator
- Remote search sends an encrypted query to a trusted peer
- Windows build
- README, Apache 2.0 license, and logo assets

## [0.1.0]
### Added
- Tauri project scaffold with React and TypeScript frontend
- UDP multicast peer discovery on 239.255.42.99:42099
- X25519 ECDH pairing ceremony with 8-digit code verification
- ChaCha20-Poly1305 encrypted sessions for all peer communication
- Basic TCP file transfer with length-prefixed framing
- SQLite schema with local device identity, trusted peers, and shared directories
- synapt-core shared types crate
- Platform detection for X11, Wayland GNOME, Wayland wlroots, Windows, and macOS
- System tray icon with show/hide
- Settings page with device name, trusted devices, and shared and indexed directories
