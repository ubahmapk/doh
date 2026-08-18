# py-doh-core

Python bindings for [`doh-core`](https://github.com/ubahmapk/doh/tree/main/doh-core),
a Rust DNS resolver library that only speaks secure transports — DNS-over-HTTPS
(RFC 8484), DNS-over-TLS (RFC 7858), and DNS-over-QUIC (RFC 9250) — with **no
fallback to classic plaintext DNS**. If a query can't be answered securely, you
get a clear exception instead of a silent, unencrypted lookup.

```sh
pip install py-doh-core
```

## Quickstart

```python
import py_doh_core as doh

transport = doh.DohTransport("https://dns.google/dns-query")
response = transport.resolve("example.com", "A")

for answer in response.answers:
    print(answer.name, answer.ttl, answer.rdata)
```

Every `resolve()` has an `async def`-compatible `aresolve()` twin:

```python
import asyncio
import py_doh_core as doh


async def main() -> None:
    transport = doh.DotTransport("dns.google")
    response = await transport.aresolve("example.com", "AAAA")
    print(response.answers[0].rdata)


asyncio.run(main())
```

`resolve()` blocks the calling thread but releases the GIL for the network
call, so other Python threads keep running; `aresolve()` returns a normal
awaitable. Both forms exist on all three transports below.

## Transports

| Class | Protocol | Constructor |
| --- | --- | --- |
| `DohTransport` | DNS-over-HTTPS | `DohTransport(server_url, method=None)` — `server_url` e.g. `"https://dns.google/dns-query"` (must be `https://`); `method` is `"get"` (default) or `"post"` |
| `DotTransport` | DNS-over-TLS | `DotTransport(server_addr)` — `server_addr` is `host[:port]`, default port 853 |
| `DoqTransport` | DNS-over-QUIC | `DoqTransport(server_addr)` — same `host[:port]` form; the QUIC connection is pooled and reused across every `resolve()`/`aresolve()` call on the instance |

All three share the same API: `resolve`, `aresolve`, `resolve_many`,
`aresolve_many`. Pick a transport, not a different way of calling it.

## Responses are typed, not dicts

`resolve()`/`aresolve()` return a `ParsedResponse` with real attributes —
`response.answers[0].rdata`, not `response["answers"][0]["rdata"]` — mirroring
every field of the underlying `doh_core::ParsedResponse` (header flags,
question, answer/authority/additional sections, wire size). `op_code` and
`response_code` are `OpCode`/`ResponseCode` enums, not magic strings or ints:

```python
if response.response_code == doh.ResponseCode.NXDOMAIN:
    ...  # the name doesn't exist -- this is a successful response, not an error
```

See [`py_doh_core.pyi`](https://github.com/ubahmapk/doh/blob/main/py-doh-core/py_doh_core.pyi)
for the full type signatures.

## Handling errors

Anything that isn't a clean "no error" or "name does not exist" response —
a bad server URL, a connection failure, `SERVFAIL`, `REFUSED`, a malformed
reply — raises `doh.DohError` rather than returning a half-empty result:

```python
try:
    response = transport.resolve("dnssec-failed.org", "A")
except doh.DohError as exc:
    print("lookup failed:", exc)
```

`str(exc)` carries the same message the underlying Rust library produces.

## Multiple record types in one call

`resolve_many()`/`aresolve_many()` query several record types for one name
against a single transport instance. Queries run in turn, reusing the
connection where that matters (`DoqTransport`'s pooled connection in
particular), and one type's failure doesn't abort the rest — each entry in
the returned list is a `QueryResult` with exactly one of `response`/`error`
set:

```python
for result in transport.resolve_many("example.com", ["A", "AAAA", "MX"]):
    if result.error is not None:
        print(result.record_type, "failed:", result.error)
    else:
        print(result.record_type, [a.rdata for a in result.response.answers])
```

An unparseable record type string is the one exception: it raises `DohError`
immediately, before any query in the batch is sent.

## Logging

Verbose/debug output uses Python's standard `logging` module — no separate
init call needed:

```python
import logging

logging.basicConfig(level=logging.DEBUG)
```

Logger names follow the Rust module path, e.g. `doh_core.transport.doh`,
`doh_core.transport.doq`, `py_doh_core.transport`. `DEBUG` shows one line per
query (server, method, connection reuse, response codes); a small amount of
extra detail (e.g. response sizes) logs at level `5`, below `logging.DEBUG`
(10) — pass `level=5` to see it. Scope to just this library with
`logging.getLogger("doh_core").setLevel(logging.DEBUG)`.

Only `doh_core`/`py_doh_core` targets are bridged to Python — dependency
crates (`reqwest`, `h2`, `rustls`, `quinn`) are deliberately not, since their
logging runs on long-lived background threads that can outlive a single
`resolve()` call and, in rare cases, still be active as the Python
interpreter shuts down.

## Development

`py-doh-core` is a PyO3 `cdylib` extension module, built with
[`maturin`](https://www.maturin.rs). It's intentionally excluded from the
main Cargo workspace (see the root `Cargo.toml`) since it needs maturin's
linker setup to resolve Python symbols at import time — plain
`cargo build`/`cargo test --workspace` can't link it.

### Build from source

```sh
cd py-doh-core
python3 -m venv .venv
source .venv/bin/activate
pip install maturin
maturin develop
```

### Tests

```sh
pip install pytest pytest-asyncio
pytest
```

`tests/test_resolve.py` runs live against real public resolvers (no mocking
layer, same approach the Rust side uses). DoT/DoQ cases skip automatically
if port 853 is unreachable on the current network.
