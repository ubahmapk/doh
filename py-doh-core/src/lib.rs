use std::str::FromStr;
use std::sync::OnceLock;

use doh_core::{DohError as RustDohError, DohTransport, HttpMethod, RecordType, Transport};
use pyo3::create_exception;
use pyo3::exceptions::PyException;
use pyo3::prelude::*;
use pyo3::types::PyDict;

create_exception!(py_doh_core, DohError, PyException);

fn runtime() -> &'static tokio::runtime::Runtime {
    static RUNTIME: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RUNTIME.get_or_init(|| {
        tokio::runtime::Runtime::new().expect("failed to start the doh-core tokio runtime")
    })
}

/// A DNS-over-HTTPS transport bound to a single server URL.
///
/// First-pass binding: GET only. POST, DoT, and DoQ are follow-ups once
/// this pipeline (Rust async -> blocking Python call, error mapping,
/// answer conversion) is proven out.
#[pyclass]
struct PyDohTransport {
    inner: DohTransport,
}

#[pymethods]
impl PyDohTransport {
    #[new]
    fn new(server_url: String) -> PyResult<Self> {
        let inner = DohTransport::new(server_url, HttpMethod::Get)
            .map_err(|e| DohError::new_err(e.to_string()))?;
        Ok(Self { inner })
    }

    /// Resolve `name`/`record_type` (e.g. "A", "AAAA", "MX"), blocking the
    /// calling Python thread (the GIL is released for the duration of the
    /// network call, so other Python threads keep running).
    ///
    /// Returns a list of dicts: {"name", "ttl", "record_type", "rdata"}.
    /// Raises `py_doh_core.DohError` on any transport/DNS failure --
    /// there is no fallback to classic plaintext DNS, matching doh-core's
    /// own behavior.
    fn resolve<'py>(
        &self,
        py: Python<'py>,
        name: String,
        record_type: String,
    ) -> PyResult<Bound<'py, pyo3::types::PyList>> {
        let record_type = RecordType::from_str(&record_type.to_uppercase())
            .map_err(|_| DohError::new_err(format!("unknown record type '{record_type}'")))?;

        let transport = &self.inner;
        let result: Result<doh_core::ParsedResponse, RustDohError> =
            py.detach(|| runtime().block_on(async { transport.resolve(&name, record_type).await }));

        let response = result.map_err(|e| DohError::new_err(e.to_string()))?;

        let answers = pyo3::types::PyList::empty(py);
        for answer in &response.answers {
            let dict = PyDict::new(py);
            dict.set_item("name", answer.name.to_string())?;
            dict.set_item("ttl", answer.ttl)?;
            dict.set_item("record_type", answer.record_type.to_string())?;
            dict.set_item("rdata", answer.rdata.to_string())?;
            answers.append(dict)?;
        }
        Ok(answers)
    }
}

#[pymodule]
fn py_doh_core(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add_class::<PyDohTransport>()?;
    m.add("DohError", m.py().get_type::<DohError>())?;
    Ok(())
}
