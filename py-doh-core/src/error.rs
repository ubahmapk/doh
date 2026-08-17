use pyo3::create_exception;
use pyo3::exceptions::PyException;

create_exception!(
    py_doh_core,
    DohError,
    PyException,
    "Raised for any doh-core transport/DNS failure. str(exc) is the same \
     message the Rust DohError produces (bad server URL, connection \
     failure, malformed response, SERVFAIL/REFUSED, ...). There is no \
     fallback to classic plaintext DNS, matching the Rust library's own \
     behavior."
);
