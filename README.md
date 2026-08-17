# doh

[![CI](https://github.com/ubahmapk/doh/actions/workflows/ci.yml/badge.svg)](https://github.com/ubahmapk/doh/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![MSRV](https://img.shields.io/badge/MSRV-1.88-blue.svg)](Cargo.toml)
[![codecov](https://codecov.io/gh/ubahmapk/doh/branch/main/graph/badge.svg)](https://codecov.io/gh/ubahmapk/doh)

A Rust DNS client library and CLI for secure DNS transports, starting with
DNS-over-HTTPS ([RFC 8484]).

**This tool never falls back to classic plaintext UDP/TCP DNS.** If a secure
transport fails, it prints a clear, actionable error and exits non-zero.

## Status

Implemented: DNS-over-HTTPS ([RFC 8484]), DNS-over-TLS ([RFC 7858]),
DNS-over-QUIC ([RFC 9250]).

Planned, not yet implemented:

- Oblivious DoH ([RFC 9230])
- DNSCrypt v2

[RFC 8484]: https://www.rfc-editor.org/rfc/rfc8484
[RFC 7858]: https://www.rfc-editor.org/rfc/rfc7858
[RFC 9250]: https://www.rfc-editor.org/rfc/rfc9250
[RFC 9230]: https://www.rfc-editor.org/rfc/rfc9230

## Usage

Output formatting and flags closely follow [`q`](https://github.com/natesales/q):

```sh
# default: 6 common types (A, AAAA, NS, MX, TXT, CNAME), pretty output
doh example.com --server https://dns.google/dns-query

# a specific type
doh example.com MX --server https://dns.google/dns-query

# JSON or YAML
doh example.com MX --server https://dns.google/dns-query --format=json
doh example.com MX --server https://dns.google/dns-query --format=yaml

# dig-style raw output
doh example.com MX --server https://dns.google/dns-query --format=raw

# query timing and message stats
doh example.com A --server https://dns.google/dns-query --stats

# values only
doh example.com A --server https://dns.google/dns-query --short
```

```
example.com. 5m A 93.184.216.34
```

Options:

- `<name>` (positional) — name to resolve
- `[record_types...]` (positional, variadic) — e.g. `A AAAA MX`; defaults to `A AAAA NS MX TXT CNAME` if none given
- `--server <addr>` (required) — `https://host/path` for DoH, `tls://host[:port]` for DoT, `quic://host[:port]` for DoQ
- `--method get|post` (default `get`) — HTTP method per RFC 8484 §4; ignored for DoT/DoQ
- `-f, --format <pretty|column|json|yaml|raw>` (default `pretty`)
- `--question` / `--answer` (default on) / `--authority` / `--additional` / `--all` — which sections to show
- `-S, --stats` — query time, response size, opcode/status/id/flags, section counts
- `-r, --short` — record values only, no name/ttl/type columns
- `--pretty-ttls` (default on) / `--short-ttls` (default on) / `--round-ttls` — TTL display formatting (e.g. `24h0m0s` → `24h`)
- `--color` / `--no-color` — color is on by default when stdout is a terminal, off when piped, and honors `NO_COLOR`

`--server` isn't required on the command line if a config file supplies
one (see [Configuration](#configuration) below); otherwise you must
specify it explicitly. Multiple record types are queried sequentially
against one transport instance (reusing the connection for DoQ's pooled
model); if any type's query fails, `doh` still queries the rest and
reports each result, then exits non-zero — a deliberate difference from
`q`, which aborts on the first failure.

Run `doh --help` for the full flag list.

## Configuration

Most flags can be given a default in a TOML config file instead of typing
them every time. **Precedence: CLI flag > config file > built-in default.**
Every boolean flag has a paired `--no-<flag>` (e.g. `--no-stats`) so a CLI
invocation can always override a config value back off, not just on.

Default location (via the [`directories`](https://docs.rs/directories) crate):

| OS | Path |
|---|---|
| Linux | `~/.config/doh/config.toml` |
| macOS | `~/Library/Application Support/doh/config.toml` |
| Windows | `%APPDATA%\doh\config.toml` |

Override with `--config <path>`. A missing config file is not an error
(built-in defaults apply); a present-but-malformed one is — reported
clearly, not silently ignored.

Run `doh --init-config` to create a starter template at that location (or
at `--config <path>` if given). The template has every key present but
commented out, so it changes no behavior until you uncomment something.
It won't overwrite an existing file — if one's already there, `doh` warns
and exits non-zero instead.

Every key is optional; set only what you want to override:

```toml
# Connection
server = "https://dns.google/dns-query"
method = "get"                                    # "get" | "post"
default_record_types = ["A", "AAAA"]               # overrides the built-in 6-type default

# Output
format = "pretty"                                  # "pretty" | "column" | "json" | "yaml" | "raw"

# Sections
question = false
answer = true
authority = false
additional = false
all = false
stats = false
short = false

# TTL display
pretty_ttls = true
short_ttls = true
round_ttls = false

# Color: on if stdout is a terminal, off when piped/NO_COLOR is set, unless
# explicitly set here or on the CLI
color = true
```

With `server = "https://dns.google/dns-query"` set, `doh example.com`
alone is then equivalent to today's `doh example.com --server
https://dns.google/dns-query`.

For DoT and DoQ, the same hostname is used both to resolve the connection
address via the OS resolver and to validate the server's TLS certificate.
Finding the server's IP therefore isn't itself protected — only the
queries sent to it, once connected, are.

DoT opens a new TCP+TLS connection per query; connections are not pooled.
DoQ instead pools one QUIC connection per `DoqTransport`/CLI invocation,
reused across queries and transparently reconnected if it closes — this is
the main practical difference between the two: DoQ amortizes connection
setup, DoT does not.

## Library

The `doh-core` crate exposes the `Transport` trait, implemented by
`DohTransport`, `DotTransport`, and `DoqTransport`, for use in other Rust
programs:

```rust
use doh_core::{DohTransport, HttpMethod, RecordType, ResponseCode, Transport};

let transport = DohTransport::new("https://dns.google/dns-query", HttpMethod::Get)?;
let response = transport.resolve("example.com", RecordType::A).await?;
if response.response_code == ResponseCode::NXDomain {
    // name does not exist — this is a successful response, not an error
}
for answer in response.answers {
    println!("{} {} {}", answer.name, answer.ttl, answer.rdata);
}
// authority/additional sections and header metadata (id, opcode, flags,
// wire_size) are also on `response` — see ParsedResponse's docs.
```

`Transport::resolve` returns `Err(DohError::Dns { .. })` for any response
code other than `NoError`/`NXDomain` (e.g. `ServFail`, `Refused`) — a DNS
server error is never silently reported as "no records."

## Security posture

- **No fallback to classic plaintext UDP/TCP DNS**, at any layer. On
  failure the tool reports a clear, actionable error and exits non-zero.
- **10 second request timeout** — a slow or black-holing server fails
  instead of hanging the caller forever.
- **HTTP redirects are never followed** (`redirect::Policy::none()`). A
  hostile or compromised DoH endpoint cannot silently bounce queries to a
  third-party host you never chose.
- **Response bodies are capped at 64 KiB** (the same limit classic
  DNS-over-TCP framing uses) on both the success and error paths, so a
  malicious/oversized response can't be used to exhaust memory.
- **TLS certificate verification uses the OS trust store**
  (`rustls-native-certs`), so it honors corporate/custom CAs already
  installed on the machine — trust follows whatever the OS is configured
  to trust, for better or worse.
- Server-controlled error text (HTTP error bodies) has control characters
  stripped before being printed to the terminal, to avoid terminal escape
  sequence injection from a malicious server.

## Versioning

This project follows [Semantic Versioning](https://semver.org/). See
[CHANGELOG.md](CHANGELOG.md) for notable changes per release.

## Development

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```
