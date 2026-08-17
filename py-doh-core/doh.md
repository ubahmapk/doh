# py-doh-core

Python bindings for [`doh-core`](../doh-core), via
[PyO3](https://pyo3.rs)/[maturin](https://www.maturin.rs). Not published to
PyPI.

`DohTransport` (DoH, GET or POST), `DotTransport` (DoT), and
`DoqTransport` (DoQ) are all bound, each with a blocking `resolve()` and
an `async def`-compatible `aresolve()`, plus `resolve_many()`/
`aresolve_many()` for querying several record types against one name in
a single call (mirroring `doh-cli`'s variadic `[record_types...]`
argument -- see [Multiple record types](#multiple-record-types) below).
Responses come back as typed `ParsedResponse`/`Answer` objects (see
`py_doh_core.pyi` for the full shape) mirroring every field of
`doh_core::ParsedResponse`, not plain dicts. `op_code`/`response_code`
are `OpCode`/`ResponseCode` enums (e.g. `response.response_code ==
doh.ResponseCode.NXDOMAIN`), not magic strings.

This crate is intentionally **excluded** from the main Cargo workspace
(see the root `Cargo.toml`): it's a PyO3 `cdylib` extension module, which
needs `maturin`'s linker setup to resolve Python symbols at import time —
plain `cargo build`/`cargo test --workspace` can't link it.

## Build / try it locally

```sh
cd py-doh-core
python3 -m venv .venv
source .venv/bin/activate
pip install maturin
maturin develop
```

The convention used here and throughout `tests/test_resolve.py` is to
import the module as `doh`:

```python
import asyncio
import py_doh_core as doh

transport = doh.DohTransport("https://dns.google/dns-query")
response = transport.resolve("example.com", "A")
assert response.response_code == doh.ResponseCode.NOERROR
print(response.response_code, response.answers[0].rdata)

async def main():
    dot = doh.DotTransport("dns.google")
    return await dot.aresolve("example.com", "AAAA")


print(asyncio.run(main()))
```

Errors (bad server URL, DNS failures, `SERVFAIL`/`REFUSED`, etc.) raise
`doh.DohError` with the same message `doh-core` itself produces — no
fallback to classic plaintext DNS, same as the Rust library.

## Multiple record types

`resolve_many()`/`aresolve_many()` query several record types for one
name against a single transport instance, matching `doh-cli`'s own
multi-type behavior: queries run in turn (reusing the connection --
this matters most for `DoqTransport`'s pooled connection), and one
type's failure doesn't abort the rest. Each entry in the returned list
is a `QueryResult` with `record_type`, and exactly one of
`response`/`error` set:

```python
transport = doh.DohTransport("https://dns.google/dns-query")
for result in transport.resolve_many("example.com", ["A", "AAAA", "MX"]):
    if result.error is not None:
        print(result.record_type, "failed:", result.error)
    else:
        print(result.record_type, [a.rdata for a in result.response.answers])
```

An unparseable record type string (unlike a per-query network/DNS
failure) raises `DohError` immediately, before any query is sent —
also matching `doh-cli`'s CLI-arg validation.

## Logging

Verbose/debug output uses Python's standard `logging` module -- no
separate init call needed:

```python
import logging
logging.basicConfig(level=logging.DEBUG)
```

Logger names follow the Rust module path, e.g. `doh_core.transport.doh`,
`doh_core.transport.doq`, `py_doh_core.transport`. `DEBUG` shows one line
per query (server, method, connection reuse, response codes); a small
amount of extra detail (e.g. response sizes) logs at level `5`, below
`logging.DEBUG` (10) -- pass `level=5` to see it. Scope to just this
library with `logging.getLogger("doh_core").setLevel(logging.DEBUG)`.

Only `doh_core`/`py_doh_core` targets are bridged to Python -- dependency
crates (`reqwest`, `h2`, `rustls`, `quinn`) are deliberately not, since
their logging runs on long-lived background threads that can outlive a
single `resolve()` call and, in rare cases, still be active as the
Python interpreter shuts down.

## Tests

```sh
pip install pytest pytest-asyncio
pytest
```

`tests/test_resolve.py` runs live against real public resolvers (no
mocking layer, same approach the Rust side uses). DoT/DoQ cases skip
automatically if port 853 is unreachable on the current network.
