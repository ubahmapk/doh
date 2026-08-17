# py-doh-core

Python bindings for [`doh-core`](../doh-core), via
[PyO3](https://pyo3.rs)/[maturin](https://www.maturin.rs). Not published to
PyPI.

`PyDohTransport` (DoH, GET or POST), `PyDotTransport` (DoT), and
`PyDoqTransport` (DoQ) are all bound, each with a blocking `resolve()` and
an `async def`-compatible `aresolve()`. Responses come back as typed
`PyParsedResponse`/`PyAnswer` objects (see `py_doh_core.pyi` for the full
shape) mirroring every field of `doh_core::ParsedResponse`, not plain
dicts.

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

```python
import asyncio
import py_doh_core

transport = py_doh_core.PyDohTransport("https://dns.google/dns-query")
response = transport.resolve("example.com", "A")
print(response.response_code, response.answers[0].rdata)

dot = py_doh_core.PyDotTransport("dns.google")
response = asyncio.run(dot.aresolve("example.com", "AAAA"))
print(response)
```

Errors (bad server URL, DNS failures, `SERVFAIL`/`REFUSED`, etc.) raise
`py_doh_core.DohError` with the same message `doh-core` itself produces —
no fallback to classic plaintext DNS, same as the Rust library.

## Tests

```sh
pip install pytest pytest-asyncio
pytest
```

`tests/test_resolve.py` runs live against real public resolvers (no
mocking layer, same approach the Rust side uses). DoT/DoQ cases skip
automatically if port 853 is unreachable on the current network.
