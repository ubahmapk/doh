mod error;
mod runtime;
mod transport;
mod types;

use pyo3::prelude::*;

use error::DohError;
use runtime::runtime;
use transport::{PyDohTransport, PyDoqTransport, PyDotTransport};
use types::{PyAnswer, PyOpCode, PyParsedResponse, PyQueryResult, PyResponseCode};

/// Python bindings for `doh-core`: `DohTransport`, `DotTransport`, and
/// `DoqTransport` (DNS-over-HTTPS, -TLS, and -QUIC), each with a blocking
/// `resolve()` and an `async def`-compatible `aresolve()`, plus
/// `resolve_many()`/`aresolve_many()` for multiple record types in one
/// call. No fallback to classic plaintext DNS.
#[pymodule]
fn py_doh_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    // Bridge Rust `log` records to Python's stdlib `logging` module, so
    // verbose/debug output is controlled the normal Python way, e.g.
    // `logging.basicConfig(level=logging.DEBUG)`. Logger names follow the
    // Rust module path (e.g. "doh_core.transport.doh", "py_doh_core.transport").
    //
    // Only our own targets are bridged -- dependency crates (reqwest, h2,
    // rustls, quinn) log from long-lived tokio worker threads that outlive
    // a single `resolve()` call and can still be running as the Python
    // interpreter finalizes at process exit. Attaching to Python from one
    // of those threads at that point segfaults (PyGILState_Ensure into a
    // finalizing interpreter). Scoping the filter means those crates'
    // targets are rejected before any `Python::attach` is attempted.
    let _ = pyo3_log::Logger::default()
        .filter(log::LevelFilter::Off)
        .filter_target("doh_core".to_owned(), log::LevelFilter::Trace)
        .filter_target("py_doh_core".to_owned(), log::LevelFilter::Trace)
        .install();

    // Share one tokio runtime between the blocking (`.block_on`) and async
    // (`aresolve`, via pyo3-async-runtimes) call paths.
    pyo3_async_runtimes::tokio::init_with_runtime(runtime())
        .expect("py_doh_core module initialized more than once");

    m.add_class::<PyDohTransport>()?;
    m.add_class::<PyDotTransport>()?;
    m.add_class::<PyDoqTransport>()?;
    m.add_class::<PyAnswer>()?;
    m.add_class::<PyParsedResponse>()?;
    m.add_class::<PyOpCode>()?;
    m.add_class::<PyResponseCode>()?;
    m.add_class::<PyQueryResult>()?;
    m.add("DohError", m.py().get_type::<DohError>())?;
    Ok(())
}
