# doh-core

A Rust DNS client library for secure DNS transports: DNS-over-HTTPS
([RFC 8484]), DNS-over-TLS ([RFC 7858]), and DNS-over-QUIC ([RFC 9250]).

**This library never falls back to classic plaintext UDP/TCP DNS.** If a
secure transport fails, it returns a clear, actionable error.

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

`doh-core` exposes the `Transport` trait, implemented by `DohTransport`,
`DotTransport`, and `DoqTransport`:

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
  failure the library returns a clear, actionable error.
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

## More

Full project docs, the `doh` CLI built on this library, and the
changelog: <https://github.com/ubahmapk/doh>.
