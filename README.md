# doh

A Rust DNS client library and CLI for secure DNS transports, starting with
DNS-over-HTTPS ([RFC 8484]).

**This tool never falls back to classic plaintext UDP/TCP DNS.** If a secure
transport fails, it prints a clear, actionable error and exits non-zero.

## Status

Phase 1 (this release): DoH only.

Planned, not yet implemented:

- DNS-over-TLS ([RFC 7858])
- DNS-over-QUIC ([RFC 9250])
- Oblivious DoH ([RFC 9230])
- DNSCrypt v2

[RFC 8484]: https://www.rfc-editor.org/rfc/rfc8484
[RFC 7858]: https://www.rfc-editor.org/rfc/rfc7858
[RFC 9250]: https://www.rfc-editor.org/rfc/rfc9250
[RFC 9230]: https://www.rfc-editor.org/rfc/rfc9230

## Usage

```sh
cargo run -p doh-cli -- example.com A --server https://dns.google/dns-query
```

```
example.com.	86400	A	93.184.216.34
```

Options:

- `record_type` (positional, default `A`) — e.g. `A`, `AAAA`, `CNAME`, `TXT`, `MX`
- `--server <url>` (required) — DoH server URL, must be `https://`
- `--method get|post` (default `get`) — HTTP method used per RFC 8484 §4

There is no default DoH server — you must specify one explicitly.

## Library

The `doh-core` crate exposes the `Transport` trait and `DohTransport`
implementation for use in other Rust programs:

```rust
use doh_core::{DohTransport, HttpMethod, RecordType, Transport};

let transport = DohTransport::new("https://dns.google/dns-query", HttpMethod::Get)?;
let answers = transport.resolve("example.com", RecordType::A).await?;
```

## Development

```sh
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo fmt --all --check
```
