# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-08-15

### Added

- `doh-core` library crate: DNS-over-HTTPS ([RFC 8484]) transport supporting
  GET and POST, built on `reqwest` and `hickory-proto`.
- `Transport` trait as the common interface for future protocol transports
  (DoT, DoQ, ODoH, DNSCrypt).
- `doh-cli` binary: resolve a name/record type against a given DoH server URL.
- No fallback to classic UDP/TCP DNS: transport failures surface as clear,
  actionable errors instead.
- GitHub Actions CI: build, test, clippy, fmt.

[RFC 8484]: https://www.rfc-editor.org/rfc/rfc8484

[Unreleased]: https://github.com/ubahmapk/doh/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ubahmapk/doh/releases/tag/v0.1.0
