# py-doh-core

Experimental Python bindings for [`doh-core`](../doh-core), via
[PyO3](https://pyo3.rs)/[maturin](https://www.maturin.rs). Not published to
PyPI yet.

**Status: first-pass proof of pipeline, not full parity with `doh-core`.**
Only `PyDohTransport` (DoH, GET only) and a blocking `resolve()` are
implemented. `DotTransport`/`DoqTransport`, POST, and a richer answer type
(currently plain `dict`s) are follow-ups.

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
import py_doh_core

transport = py_doh_core.PyDohTransport("https://dns.google/dns-query")
for answer in transport.resolve("example.com", "A"):
    print(answer)  # {"name": ..., "ttl": ..., "record_type": ..., "rdata": ...}
```

Errors (bad server URL, DNS failures, `SERVFAIL`/`REFUSED`, etc.) raise
`py_doh_core.DohError` with the same message `doh-core` itself produces —
no fallback to classic plaintext DNS, same as the Rust library.
