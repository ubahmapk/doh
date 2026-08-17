# py-doh-core

Python bindings for [`doh-core`](../doh-core), via
[PyO3](https://pyo3.rs)/[maturin](https://www.maturin.rs). Not published to
PyPI.

`DohTransport` (DoH, GET or POST), `DotTransport` (DoT), and
`DoqTransport` (DoQ) are all bound, each with a blocking `resolve()` and
an `async def`-compatible `aresolve()`. Responses come back as typed
`ParsedResponse`/`Answer` objects (see `py_doh_core.pyi` for the full
shape) mirroring every field of `doh_core::ParsedResponse`, not plain
dicts. `op_code`/`response_code` are `OpCode`/`ResponseCode` enums
(e.g. `response.response_code == doh.ResponseCode.NXDOMAIN`),
not magic strings.

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

## Tests

```sh
pip install pytest pytest-asyncio
pytest
```

`tests/test_resolve.py` runs live against real public resolvers (no
mocking layer, same approach the Rust side uses). DoT/DoQ cases skip
automatically if port 853 is unreachable on the current network.
