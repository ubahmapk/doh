mod error;
mod runtime;
mod transport;
mod types;

use pyo3::prelude::*;

use error::DohError;
use runtime::runtime;
use transport::{PyDohTransport, PyDoqTransport, PyDotTransport};
use types::{PyAnswer, PyOpCode, PyParsedResponse, PyResponseCode};

#[pymodule]
fn py_doh_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
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
    m.add("DohError", m.py().get_type::<DohError>())?;
    Ok(())
}
