# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `DotTransport`: DNS-over-TLS ([RFC 7858]) transport, sharing the
  `Transport` trait with `DohTransport`. `doh-cli --server tls://host[:port]`
  selects it (default port 853). Opens a new TCP+TLS connection per query,
  10s connect/query timeout, 64 KiB message cap (RFC 1035 §4.2.2 stream
  framing), OS trust store via `rustls-native-certs`, no connection pooling.
- `DoqTransport`: DNS-over-QUIC ([RFC 9250]) transport, via `quinn`.
  `doh-cli --server quic://host[:port]` selects it (default port 853,
  shared with DoT). Unlike `DotTransport`, pools and reuses one QUIC
  connection per transport instance across queries (reconnecting
  transparently if it closes) rather than opening a new one per query —
  the whole point of choosing QUIC. Same 10s timeout and 64 KiB message
  cap as DoT; ALPN `"doq"`, TLS 1.3 only, and the DNS message ID zeroed on
  the wire per RFC 9250 §4.2.1/§4.1/§4.2 respectively.
- CI bumped `actions/checkout` to v7 (resolves a Node 20 deprecation
  warning; v7 targets Node 24 natively).

### Security

- CI: pinned `actions/checkout`, `dtolnay/rust-toolchain`, and
  `Swatinem/rust-cache` to full commit SHAs instead of mutable tags/branches
  (semgrep `github-actions-mutable-action-tag`), to prevent a repointed
  tag from silently changing what code CI executes. Added
  `.github/dependabot.yml` (`github-actions` ecosystem) so the pins still
  get automated update PRs.

### Fixed

- DNS response codes other than `NOERROR`/`NXDOMAIN` (e.g. `SERVFAIL`,
  `REFUSED`) are now surfaced as `DohError::Dns` instead of being silently
  reported as "no records" with a zero exit code.
- `doh-cli` reports `NXDOMAIN` distinctly from a genuinely empty answer set.

### Added

- Request timeout (10s) on the DoH HTTP client; previously unset, so a
  slow/black-holing server could hang indefinitely.
- Response bodies (success and HTTP-error paths) are capped at 64 KiB to
  bound memory use against an oversized response.
- `Answer` now carries typed `hickory_proto` values (`Name`, `RData`)
  instead of pre-formatted strings.

### Changed

- HTTP client no longer follows redirects (`redirect::Policy::none()`), to
  prevent a compromised/hostile DoH endpoint from silently redirecting
  queries elsewhere.
- Switched TLS root store from bundled `webpki-roots` to
  `rustls-native-certs` (OS trust store).
- `DohError` is now `#[non_exhaustive]`.
- Server-controlled error text is stripped of control characters before
  being printed, to prevent terminal escape sequence injection.

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
[RFC 7858]: https://www.rfc-editor.org/rfc/rfc7858
[RFC 9250]: https://www.rfc-editor.org/rfc/rfc9250

[Unreleased]: https://github.com/ubahmapk/doh/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/ubahmapk/doh/releases/tag/v0.1.0
