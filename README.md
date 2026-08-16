# doh

A Rust DNS client library and CLI for secure DNS transports, starting with
DNS-over-HTTPS ([RFC 8484]).

**This tool never falls back to classic plaintext UDP/TCP DNS.** If a secure
transport fails, it prints a clear, actionable error and exits non-zero.

## Status

Implemented: DNS-over-HTTPS ([RFC 8484]), DNS-over-TLS ([RFC 7858]).

Planned, not yet implemented:

- DNS-over-QUIC ([RFC 9250])
- Oblivious DoH ([RFC 9230])
- DNSCrypt v2

[RFC 8484]: https://www.rfc-editor.org/rfc/rfc8484
[RFC 7858]: https://www.rfc-editor.org/rfc/rfc7858
[RFC 9250]: https://www.rfc-editor.org/rfc/rfc9250
[RFC 9230]: https://www.rfc-editor.org/rfc/rfc9230

## Usage

```sh
# DoH: https:// selects DNS-over-HTTPS
cargo run -p doh-cli -- example.com A --server https://dns.google/dns-query

# DoT: tls:// selects DNS-over-TLS, host[:port], default port 853
cargo run -p doh-cli -- example.com A --server tls://dns.google
```

```
example.com.	86400	A	93.184.216.34
```

Options:

- `record_type` (positional, default `A`) — e.g. `A`, `AAAA`, `CNAME`, `TXT`, `MX`
- `--server <addr>` (required) — `https://host/path` for DoH, `tls://host[:port]` for DoT
- `--method get|post` (default `get`) — HTTP method used per RFC 8484 §4; ignored for DoT

There is no default server — you must specify one explicitly.

For DoT, the same hostname is used both to resolve the connection address
via the OS resolver and to validate the server's TLS certificate. Finding
the DoT server's IP therefore isn't itself protected by DoT — only the
queries sent to it, once connected, are. Each query opens a new TCP+TLS
connection; connections are not pooled or pipelined.

## Library

The `doh-core` crate exposes the `Transport` trait, implemented by
`DohTransport` and `DotTransport`, for use in other Rust programs:

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
