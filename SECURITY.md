# Security Policy

## Reporting a vulnerability

Do not open a public GitHub issue for security vulnerabilities.

Report vulnerabilities privately using GitHub's security advisory system:
https://github.com/aatishbagal/synapt/security/advisories/new

Include as much detail as possible: steps to reproduce, potential impact, and any proof of concept.

We will acknowledge the report within 72 hours and aim to release a fix within 30 days depending on severity.

## Scope

The following are in scope:

- Authentication or trust bypass in the device pairing flow (synapt)
- Unauthorised file access via the transfer layer (synapt)
- Clipboard content leaking to unintended processes or network destinations (synapt-clip)
- Remote code execution via any network-facing component

The following are out of scope:

- Attacks that require physical access to the machine
- Issues in third-party dependencies (report these to the dependency maintainer)
- Social engineering
