use std::str::FromStr;
use std::sync::Arc;

use doh_core::{DohTransport, DoqTransport, DotTransport, HttpMethod, RecordType, Transport};
use pyo3::prelude::*;

use crate::error::DohError;
use crate::runtime::runtime;
use crate::types::{PyParsedResponse, PyQueryResult};

/// Shared by every transport's `resolve`/`aresolve`: parse the record type,
/// run the query, and map any failure to the `py_doh_core.DohError`
/// exception. Generic over `Transport` so the three transport pyclasses
/// don't each repeat this logic.
async fn do_resolve<T: Transport + ?Sized>(
    transport: &T,
    name: &str,
    record_type: &str,
) -> PyResult<PyParsedResponse> {
    let record_type = RecordType::from_str(&record_type.to_uppercase())
        .map_err(|_| DohError::new_err(format!("unknown record type '{record_type}'")))?;

    transport
        .resolve(name, record_type)
        .await
        .map(PyParsedResponse::from)
        .map_err(|e| DohError::new_err(e.to_string()))
}

/// Shared by every transport's `resolve_many`/`aresolve_many`: mirrors
/// doh-cli's own multi-type behavior (main.rs's `record_types` loop) --
/// every record type string is validated up front (an unknown type fails
/// the whole batch before any query is sent), then each query runs in
/// turn against the same transport, reusing its connection where that
/// matters (DoQ's pooled connection in particular). A single query's
/// failure (e.g. `SERVFAIL`) does not abort the rest; it's captured on
/// that entry's `QueryResult.error` instead.
async fn do_resolve_many<T: Transport + ?Sized>(
    transport: &T,
    name: &str,
    record_types: &[String],
) -> PyResult<Vec<PyQueryResult>> {
    let mut parsed = Vec::with_capacity(record_types.len());
    for rt in record_types {
        let record_type = RecordType::from_str(&rt.to_uppercase())
            .map_err(|_| DohError::new_err(format!("unknown record type '{rt}'")))?;
        parsed.push((rt.clone(), record_type));
    }

    let mut results = Vec::with_capacity(parsed.len());
    for (record_type_str, record_type) in parsed {
        let result = match transport.resolve(name, record_type).await {
            Ok(response) => {
                PyQueryResult::success(record_type_str, PyParsedResponse::from(response))
            }
            Err(e) => PyQueryResult::failure(record_type_str, e.to_string()),
        };
        results.push(result);
    }
    Ok(results)
}

fn parse_method(method: Option<String>) -> PyResult<HttpMethod> {
    match method.as_deref().map(str::to_ascii_lowercase).as_deref() {
        None | Some("get") => Ok(HttpMethod::Get),
        Some("post") => Ok(HttpMethod::Post),
        Some(other) => Err(DohError::new_err(format!(
            "unknown method '{other}' (expected get or post)"
        ))),
    }
}

/// A DNS-over-HTTPS transport (RFC 8484) bound to a single server URL.
#[pyclass(name = "DohTransport")]
pub struct PyDohTransport {
    inner: Arc<DohTransport>,
}

#[pymethods]
impl PyDohTransport {
    #[new]
    #[pyo3(signature = (server_url, method=None))]
    fn new(server_url: String, method: Option<String>) -> PyResult<Self> {
        let method = parse_method(method)?;
        let inner =
            DohTransport::new(server_url, method).map_err(|e| DohError::new_err(e.to_string()))?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    /// Resolve `name`/`record_type` (e.g. "A", "AAAA", "MX"), blocking the
    /// calling Python thread. The GIL is released for the duration of the
    /// network call, so other Python threads keep running.
    fn resolve(
        &self,
        py: Python<'_>,
        name: String,
        record_type: String,
    ) -> PyResult<PyParsedResponse> {
        let inner = Arc::clone(&self.inner);
        py.detach(|| runtime().block_on(do_resolve(inner.as_ref(), &name, &record_type)))
    }

    /// Resolve `name`/`record_type`, returning a Python awaitable.
    fn aresolve<'py>(
        &self,
        py: Python<'py>,
        name: String,
        record_type: String,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = Arc::clone(&self.inner);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            do_resolve(inner.as_ref(), &name, &record_type).await
        })
    }

    /// Resolve `name` against every type in `record_types` (e.g. `["A",
    /// "AAAA", "MX"]`), blocking. Queries run in turn against this same
    /// transport; one type's failure doesn't abort the rest -- see
    /// `QueryResult`.
    fn resolve_many(
        &self,
        py: Python<'_>,
        name: String,
        record_types: Vec<String>,
    ) -> PyResult<Vec<PyQueryResult>> {
        let inner = Arc::clone(&self.inner);
        py.detach(|| runtime().block_on(do_resolve_many(inner.as_ref(), &name, &record_types)))
    }

    /// Same as `resolve_many`, returning a Python awaitable.
    fn aresolve_many<'py>(
        &self,
        py: Python<'py>,
        name: String,
        record_types: Vec<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = Arc::clone(&self.inner);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            do_resolve_many(inner.as_ref(), &name, &record_types).await
        })
    }
}

/// A DNS-over-TLS transport (RFC 7858) bound to a single `host[:port]`.
#[pyclass(name = "DotTransport")]
pub struct PyDotTransport {
    inner: Arc<DotTransport>,
}

#[pymethods]
impl PyDotTransport {
    #[new]
    fn new(server_addr: String) -> PyResult<Self> {
        let inner = DotTransport::new(server_addr).map_err(|e| DohError::new_err(e.to_string()))?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    fn resolve(
        &self,
        py: Python<'_>,
        name: String,
        record_type: String,
    ) -> PyResult<PyParsedResponse> {
        let inner = Arc::clone(&self.inner);
        py.detach(|| runtime().block_on(do_resolve(inner.as_ref(), &name, &record_type)))
    }

    fn aresolve<'py>(
        &self,
        py: Python<'py>,
        name: String,
        record_type: String,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = Arc::clone(&self.inner);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            do_resolve(inner.as_ref(), &name, &record_type).await
        })
    }

    fn resolve_many(
        &self,
        py: Python<'_>,
        name: String,
        record_types: Vec<String>,
    ) -> PyResult<Vec<PyQueryResult>> {
        let inner = Arc::clone(&self.inner);
        py.detach(|| runtime().block_on(do_resolve_many(inner.as_ref(), &name, &record_types)))
    }

    fn aresolve_many<'py>(
        &self,
        py: Python<'py>,
        name: String,
        record_types: Vec<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = Arc::clone(&self.inner);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            do_resolve_many(inner.as_ref(), &name, &record_types).await
        })
    }
}

/// A DNS-over-QUIC transport (RFC 9250) bound to a single `host[:port]`.
/// The underlying connection is pooled and shared across every `resolve`/
/// `aresolve` call on this instance (and across the blocking/async paths
/// alike, via the shared `Arc`).
#[pyclass(name = "DoqTransport")]
pub struct PyDoqTransport {
    inner: Arc<DoqTransport>,
}

#[pymethods]
impl PyDoqTransport {
    #[new]
    fn new(server_addr: String) -> PyResult<Self> {
        let inner = DoqTransport::new(server_addr).map_err(|e| DohError::new_err(e.to_string()))?;
        Ok(Self {
            inner: Arc::new(inner),
        })
    }

    fn resolve(
        &self,
        py: Python<'_>,
        name: String,
        record_type: String,
    ) -> PyResult<PyParsedResponse> {
        let inner = Arc::clone(&self.inner);
        py.detach(|| runtime().block_on(do_resolve(inner.as_ref(), &name, &record_type)))
    }

    fn aresolve<'py>(
        &self,
        py: Python<'py>,
        name: String,
        record_type: String,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = Arc::clone(&self.inner);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            do_resolve(inner.as_ref(), &name, &record_type).await
        })
    }

    fn resolve_many(
        &self,
        py: Python<'_>,
        name: String,
        record_types: Vec<String>,
    ) -> PyResult<Vec<PyQueryResult>> {
        let inner = Arc::clone(&self.inner);
        py.detach(|| runtime().block_on(do_resolve_many(inner.as_ref(), &name, &record_types)))
    }

    fn aresolve_many<'py>(
        &self,
        py: Python<'py>,
        name: String,
        record_types: Vec<String>,
    ) -> PyResult<Bound<'py, PyAny>> {
        let inner = Arc::clone(&self.inner);
        pyo3_async_runtimes::tokio::future_into_py(py, async move {
            do_resolve_many(inner.as_ref(), &name, &record_types).await
        })
    }
}
