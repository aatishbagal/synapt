# Privacy Policy - Synapt

Last updated: August 2026

## What Synapt collects

Synapt does not collect any personal data. The application has no telemetry, no analytics, no crash reporting service, and no account system. Nothing is sent to any server operated by the developers.

## Data stored on your device

Synapt stores the following data locally in a SQLite database on your device:

- Your device name and a locally generated device identifier (UUID)
- A cryptographic key pair, including the private key, used to establish encrypted connections with paired devices
- The list of devices you have paired with, including their public keys, fingerprints, and display names
- An index of file names, paths, types, and sizes from directories you choose to add
- An index of the applications installed on your device, used by the launcher
- Transfer history (file names, sizes, peer device identifiers, and timestamps)
- Application settings

This database is stored unencrypted in your user data directory and is readable by anything running as your user account.

This data never leaves your device except as described below.

## Network activity

Synapt communicates only on your local network (LAN). It does not make any connections to the internet.

While Synapt is running it broadcasts a presence packet on your local network so other devices running Synapt can discover it. This packet contains your device name, a generated device identifier, the Synapt version, and the port used for pairing. It does not contain any personal information beyond the device name you choose.

When you pair with another device and transfer files, the file content is sent directly from your device to the paired device over an encrypted connection (X25519 key exchange with ChaCha20-Poly1305) on your local network. File content is never routed through any external server.

When SynaptClip is also installed, Synapt may receive clipboard text from a peer device and forward it to SynaptClip on the same machine over a loopback connection.

## Permissions

Synapt requests the following system permissions:

- File system access: to index directories you choose and to read files for transfer
- Network access: for local network discovery and peer-to-peer file transfer

Synapt does not require macOS Accessibility permission. Its global shortcut uses the Carbon hotkey API, which does not need it.

## Contact

For questions about this privacy policy, open an issue at https://github.com/aatishbagal/synapt

## License

Copyright 2026 Aatish Bagal. Licensed under the Apache License, Version 2.0.
