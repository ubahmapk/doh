# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.3.1] - 2026-08-17

### Fixed

- `doh-core` and `doh-cli`'s crates.io pages rendered no README, since
  neither `Cargo.toml` set a `readme` field and the single project
  README lives at the workspace root (outside either crate's own
  directory, so `cargo package` didn't include it). Added focused
  `README.md` files inside each crate directory — `doh-core`'s covers
  library usage, `doh-cli`'s covers CLI usage/configuration — and set
  `readme = "README.md"` in both. Packaging-only fix, no functional
  change; the root `README.md` remains the canonical full project doc.

## [0.3.0] - 2026-08-16

### Added

- `doh-cli` now supports a TOML config file for default values, so common
  invocations can drop repeated flags (most usefully a default
  `--server`). Location follows OS convention via the `directories` crate
  (`~/.config/doh/config.toml` on Linux, etc.), overridable with
  `--config <path>`. Precedence is CLI flag > config file > built-in
  default; every boolean flag now has a paired `--no-<flag>` so a CLI
  invocation can always override a config value back off. A missing
  config file is not an error; a malformed one is, reported clearly. See
  the README's new "Configuration" section for the full set of keys.
- `doh --init-config` creates a fully commented-out config template at
  the config path (`--config <path>` if given, else the OS default),
  ready to uncomment and edit. Won't overwrite an existing file — warns
  to stderr and exits non-zero instead.

### Changed

- Deduplicated the DoT/DoQ stream-framing logic (RFC 1035 §4.2.2
  length-prefixing and the 64 KiB response-size check, previously
  inlined identically in both `dot.rs` and `doq.rs`) into shared,
  directly unit-tested helpers in `transport/util.rs`. No behavior
  change; improves test coverage of the one part of those transports
  that's both pure and risky to get wrong.

### Fixed

- `doh-cli` no longer panics with a broken-pipe backtrace when its output
  is piped into a command that closes its stdin early (e.g. `doh ... |
  head`) on Unix. SIGPIPE is now reset to its default disposition at
  process start, so the OS terminates the process silently instead, the
  same way `dig` and most Unix CLI tools behave. ([#1])

[#1]: https://github.com/ubahmapk/doh/issues/1

## [0.2.0] - 2026-08-16

### Added

- `doh-cli` output now closely follows [`q`](https://github.com/natesales/q):
  `-f/--format <pretty|column|json|yaml|raw>` (default `pretty`); section
  flags `--question`/`--answer` (default on)/`--authority`/`--additional`/
  `--all`; `-S/--stats` (query time, response size, opcode/status/id/flags,
  section counts); `-r/--short` (values only); TTL display flags
  `--pretty-ttls`/`--short-ttls` (both default on)/`--round-ttls`; and
  `--color`/`--no-color` with q's exact color scheme (name=purple,
  ttl=green, type=magenta), auto TTY detection, and `NO_COLOR` support.
- Record type(s) are now trailing positional args (`doh example.com MX
  SOA ...`) instead of a single type, defaulting to `A, AAAA, NS, MX,
  TXT, CNAME` (six types) when none are given — matching q's
  `--default-rr-types` default. Every requested type is queried against
  one transport instance (reusing DoQ's pooled connection); unlike q,
  which aborts on the first failing type, `doh` queries all of them and
  exits non-zero only if any failed — a deliberate improvement, not an
  oversight.
- `doh-core`'s `ParsedResponse` now also carries the authority and
  additional sections (`authorities`, `additionals`, same `Answer`
  shape as `answers`), header metadata needed for the new `--stats`
  output (`id`, `op_code`, `authoritative`, `truncated`,
  `recursion_desired`, `recursion_available`, `authentic_data`,
  `checking_disabled`), the echoed question (`question_name`,
  `question_type`), and the raw response size (`wire_size`).
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
- CI/README: MSRV declared as `rust-version = "1.88"` (the actual floor —
  the highest `rust-version` among current dependencies, `hickory-proto`),
  enforced by a new CI `msrv` job that pins the toolchain to exactly 1.88
  and runs `cargo check --workspace`. Also added a `coverage` CI job
  (`cargo-llvm-cov` + Codecov upload) and CI/license/MSRV/codecov badges
  to the README.
- Request timeout (10s) on the DoH HTTP client; previously unset, so a
  slow/black-holing server could hang indefinitely.
- Response bodies (success and HTTP-error paths) are capped at 64 KiB to
  bound memory use against an oversized response.

### Changed

- **Breaking (`doh-core`)**: `ParsedResponse` and `Answer` gained fields
  (see above) and are now `#[non_exhaustive]`, matching `DohError`.
  `Answer` carries typed `hickory_proto` values (`Name`, `RData`)
  instead of pre-formatted strings.
- HTTP client no longer follows redirects (`redirect::Policy::none()`), to
  prevent a compromised/hostile DoH endpoint from silently redirecting
  queries elsewhere.
- Switched TLS root store from bundled `webpki-roots` to
  `rustls-native-certs` (OS trust store).
- Server-controlled error text is stripped of control characters before
  being printed, to prevent terminal escape sequence injection.

### Fixed

- DNS response codes other than `NOERROR`/`NXDOMAIN` (e.g. `SERVFAIL`,
  `REFUSED`) are now surfaced as `DohError::Dns` instead of being silently
  reported as "no records" with a zero exit code.

### Security

- CI: pinned `actions/checkout`, `dtolnay/rust-toolchain`, and
  `Swatinem/rust-cache` to full commit SHAs instead of mutable tags/branches
  (semgrep `github-actions-mutable-action-tag`), to prevent a repointed
  tag from silently changing what code CI executes. Added
  `.github/dependabot.yml` (`github-actions` ecosystem) so the pins still
  get automated update PRs.

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

[Unreleased]: https://github.com/ubahmapk/doh/compare/v0.3.1...HEAD
[0.3.1]: https://github.com/ubahmapk/doh/compare/v0.3.0...v0.3.1
[0.3.0]: https://github.com/ubahmapk/doh/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/ubahmapk/doh/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/ubahmapk/doh/releases/tag/v0.1.0
