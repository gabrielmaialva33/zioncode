# Security Policy

## Supported Versions

`zioncode` is pre-1.0. Security fixes are made on the active default branch unless a release branch is explicitly announced.

| Version | Supported   |
|---------|-------------|
| `0.0.x` | Best effort |

## Reporting a Vulnerability

Do not publish exploit details, malicious samples, or crash-triggering corpora in a public issue.

Preferred reporting path:

1. Use GitHub private vulnerability reporting if it is enabled for this repository.
2. If private reporting is not available, open a minimal public issue asking for a secure maintainer contact. Do not include the exploit payload or detailed reproduction steps in that public issue.

Useful report contents:

- Affected commit, branch, or version.
- Impact summary.
- Minimal reproduction steps.
- Whether the issue requires malformed `.zbin` input, crafted CLI arguments, or another vector.
- Crash output, sanitizer output, or fuzz artifact metadata when available.

## Security-Relevant Areas

Treat these areas as security-sensitive:

- `zion-codec/src/format/`: binary parsing and serialization.
- `zion-codec/src/ecc.rs`: Reed-Solomon and interleaving behavior.
- `zion-codec/src/decode.rs`: symbol decoding.
- `zion-codec/src/reassemble.rs`: multi-symbol validation and recovery.
- `fuzz/`: parser and decoder fuzz targets.

Parser and decoder changes should include tests or fuzz coverage for malformed lengths, invalid headers, CRC mismatches, duplicate symbols, divergent metadata, corrupted payloads, and boundary sizes.
