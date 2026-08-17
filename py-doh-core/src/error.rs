use pyo3::create_exception;
use pyo3::exceptions::PyException;

// Raised for any `doh-core` transport/DNS failure, carrying the same
// message the Rust `DohError` produces. There is no fallback to classic
// plaintext DNS, matching the Rust library's own behavior.
create_exception!(py_doh_core, DohError, PyException);
