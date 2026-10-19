# Privacy Policy - Synapt

Last updated: October 2026

## What Synapt collects

Synapt does not collect any personal data. The application has no telemetry, no analytics, no crash reporting service, and no account system. Nothing is sent to any server operated by the developers, and nothing reaches the developers at all unless you choose to email a crash report (see Crash reports).

## Data stored on your device

Synapt stores the following data locally in a SQLite database on your device:

- Your device name and a locally generated device identifier (UUID)
- A cryptographic key pair, including the private key, used to establish encrypted connections with paired devices
- The list of devices you have paired with, including their public keys, fingerprints, and display names
- An index of file names, paths, types, and sizes from directories you choose to add
- An index of the applications installed on your device, used by the launcher
- Transfer history (file names, sizes, peer device identifiers, and timestamps)
- Application settings
- A crash log, if Synapt has crashed, kept until you send or dismiss the crash report prompt

This database is stored unencrypted in your user data directory and is readable by anything running as your user account.

This data never leaves your device except as described below.

## Network activity

Apart from update checks (described below), Synapt communicates only on your local network (LAN).

While Synapt is running it broadcasts a presence packet on your local network so other devices running Synapt can discover it. This packet contains your device name, a generated device identifier, the Synapt version, and the port used for pairing. It does not contain any personal information beyond the device name you choose.

When you pair with another device and transfer files, the file content is sent directly from your device to the paired device over an encrypted connection (X25519 key exchange with ChaCha20-Poly1305) on your local network. File content is never routed through any external server.

When SynaptClip is also installed, Synapt may receive clipboard text from a peer device and forward it to SynaptClip on the same machine over a loopback connection.

### Update checks

Synapt checks GitHub for new versions of itself and of SynaptCore, the shared component it bundles. When automatic checks are on, a check runs 15 seconds after launch and each time you open Settings. A check also runs whenever you select Check for updates.

Each check is a standard HTTPS request to github.com, api.github.com, and raw.githubusercontent.com that identifies the app and its version. Like any web request, it reveals your IP address to GitHub. It contains no personal data, files, clipboard content, or device identifiers. When you choose to install an update, the installer is downloaded from GitHub Releases.

You can turn off automatic checks in Settings with the "Check for updates automatically" option. Synapt then contacts GitHub only when you select Check for updates.

Links in the app, such as "See all changes" in the release notes, open in your default web browser. Synapt does not fetch them itself.

## Crash reports

Synapt can detect when it has crashed during a previous session. If a crash is detected on the next launch, a prompt appears offering to send a report.

Crash reports are:

- Opt-in only: nothing is sent without your explicit action
- Sent via your own email client: Synapt opens a pre-filled draft and makes no network request of its own
- Addressed to synapt.reports@aatish.io
- Limited to the app version, the operating system, the time of the crash, the location in the code where the crash occurred, and the error message
- Shown to you in full before sending: you can read, edit, or discard the draft in your email client

Synapt does not add your files, clipboard content, or device identifiers to a crash report. The error message is produced by Synapt's own code and is not meant to contain personal data, but you can check it in the draft before you send it.

You can turn off crash report prompts at any time in Settings under About, with the "Offer to email a crash report after a crash" option. The first-launch setup also asks whether to enable them.

## Permissions

Synapt requests the following system permissions:

- File system access: to index directories you choose and to read files for transfer
- Network access: for local network discovery, peer-to-peer file transfer, and update checks

Synapt does not require macOS Accessibility permission. Its global shortcut uses the Carbon hotkey API, which does not need it.

## Contact

For questions about this privacy policy, open an issue at https://github.com/aatishbagal/synapt

## License

Copyright 2026 Aatish Bagal. Licensed under the Apache License, Version 2.0.
