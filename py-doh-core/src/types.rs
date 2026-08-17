use doh_core::{OpCode, ResponseCode};
use pyo3::prelude::*;

/// The DNS message opcode.
#[pyclass(
    eq,
    eq_int,
    hash,
    frozen,
    skip_from_py_object,
    name = "OpCode",
    rename_all = "SCREAMING_SNAKE_CASE"
)]
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum PyOpCode {
    Query,
    Status,
    Notify,
    Update,
    /// Any opcode not covered above -- the server's own response is
    /// simply echoed back and not otherwise validated.
    Unknown,
}

impl std::fmt::Display for PyOpCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            PyOpCode::Query => "QUERY",
            PyOpCode::Status => "STATUS",
            PyOpCode::Notify => "NOTIFY",
            PyOpCode::Update => "UPDATE",
            PyOpCode::Unknown => "UNKNOWN",
        };
        f.write_str(s)
    }
}

impl From<OpCode> for PyOpCode {
    fn from(op: OpCode) -> Self {
        match op {
            OpCode::Query => PyOpCode::Query,
            OpCode::Status => PyOpCode::Status,
            OpCode::Notify => PyOpCode::Notify,
            OpCode::Update => PyOpCode::Update,
            _ => PyOpCode::Unknown,
        }
    }
}

/// The DNS response code. `Transport::resolve()` only ever returns
/// `NOERROR` or `NXDOMAIN` here -- every other code (`SERVFAIL`,
/// `REFUSED`, ...) is raised as a `DohError` instead, matching doh-core's
/// own behavior.
#[pyclass(eq, eq_int, hash, frozen, skip_from_py_object, name = "ResponseCode")]
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy)]
pub enum PyResponseCode {
    #[pyo3(name = "NOERROR")]
    NoError,
    #[pyo3(name = "NXDOMAIN")]
    NxDomain,
}

impl std::fmt::Display for PyResponseCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let s = match self {
            PyResponseCode::NoError => "NOERROR",
            PyResponseCode::NxDomain => "NXDOMAIN",
        };
        f.write_str(s)
    }
}

/// One resource record from an answer, authority, or additional section.
///
/// Fields:
///     name (str): Owner name of the record, e.g. "example.com.".
///     record_type (str): Record type mnemonic, e.g. "A", "AAAA", "MX".
///     ttl (int): Time-to-live, in seconds.
///     rdata (str): The record data, stringified (e.g. an IP address for
///         "A"/"AAAA", a hostname for "CNAME"/"NS").
#[pyclass(skip_from_py_object, name = "Answer")]
#[derive(Clone)]
pub struct PyAnswer {
    #[pyo3(get)]
    name: String,
    #[pyo3(get)]
    record_type: String,
    #[pyo3(get)]
    ttl: u32,
    #[pyo3(get)]
    rdata: String,
}

#[pymethods]
impl PyAnswer {
    fn __repr__(&self) -> String {
        format!(
            "Answer(name={:?}, record_type={:?}, ttl={}, rdata={:?})",
            self.name, self.record_type, self.ttl, self.rdata
        )
    }
}

impl From<&doh_core::Answer> for PyAnswer {
    fn from(answer: &doh_core::Answer) -> Self {
        Self {
            name: answer.name.to_string(),
            record_type: answer.record_type.to_string(),
            ttl: answer.ttl,
            rdata: answer.rdata.to_string(),
        }
    }
}

/// A successfully-received and parsed DNS response. Only "no error" and
/// "name does not exist" responses are ever returned here -- any other
/// response code (SERVFAIL, REFUSED, ...) is raised as a DohError instead,
/// matching the Rust library's own behavior. There is no fallback to
/// classic plaintext DNS.
///
/// Fields:
///     id (int): The 16-bit DNS message ID.
///     op_code (PyOpCode): One of QUERY, STATUS, NOTIFY, UPDATE, UNKNOWN.
///     response_code (PyResponseCode): NOERROR or NXDOMAIN.
///     authoritative (bool): The "AA" header flag.
///     truncated (bool): The "TC" header flag.
///     recursion_desired (bool): The "RD" header flag.
///     recursion_available (bool): The "RA" header flag.
///     authentic_data (bool): The "AD" header flag (DNSSEC).
///     checking_disabled (bool): The "CD" header flag (DNSSEC).
///     question_name (str): The name that was queried, e.g.
///         "example.com.".
///     question_type (str): The record type that was queried, e.g. "A".
///     answers (list[PyAnswer]): Records answering the question.
///     authorities (list[PyAnswer]): Records naming authoritative servers.
///     additionals (list[PyAnswer]): Records offered as extra context
///         (e.g. glue records).
///     wire_size (int): Size of the raw response, in bytes, as received
///         on the wire.
#[pyclass(name = "ParsedResponse")]
pub struct PyParsedResponse {
    #[pyo3(get)]
    id: u16,
    #[pyo3(get)]
    op_code: PyOpCode,
    #[pyo3(get)]
    response_code: PyResponseCode,
    #[pyo3(get)]
    authoritative: bool,
    #[pyo3(get)]
    truncated: bool,
    #[pyo3(get)]
    recursion_desired: bool,
    #[pyo3(get)]
    recursion_available: bool,
    #[pyo3(get)]
    authentic_data: bool,
    #[pyo3(get)]
    checking_disabled: bool,
    #[pyo3(get)]
    question_name: String,
    #[pyo3(get)]
    question_type: String,
    #[pyo3(get)]
    answers: Vec<PyAnswer>,
    #[pyo3(get)]
    authorities: Vec<PyAnswer>,
    #[pyo3(get)]
    additionals: Vec<PyAnswer>,
    #[pyo3(get)]
    wire_size: usize,
}

#[pymethods]
impl PyParsedResponse {
    fn __repr__(&self) -> String {
        format!(
            "ParsedResponse(id={}, op_code={}, response_code={}, question_name={:?}, \
             question_type={:?}, answers={}, authorities={}, additionals={}, wire_size={})",
            self.id,
            self.op_code,
            self.response_code,
            self.question_name,
            self.question_type,
            self.answers.len(),
            self.authorities.len(),
            self.additionals.len(),
            self.wire_size,
        )
    }
}

impl From<doh_core::ParsedResponse> for PyParsedResponse {
    fn from(response: doh_core::ParsedResponse) -> Self {
        Self {
            id: response.id,
            op_code: PyOpCode::from(response.op_code),
            response_code: match response.response_code {
                ResponseCode::NoError => PyResponseCode::NoError,
                ResponseCode::NXDomain => PyResponseCode::NxDomain,
                // Every transport only ever returns `Ok` for NoError/
                // NXDomain (see e.g. doh-core/src/transport/doh.rs);
                // anything else is raised as a DohError before a
                // ParsedResponse is ever constructed.
                other => unreachable!(
                    "Transport::resolve() only returns NoError/NXDomain, got {other:?}"
                ),
            },
            authoritative: response.authoritative,
            truncated: response.truncated,
            recursion_desired: response.recursion_desired,
            recursion_available: response.recursion_available,
            authentic_data: response.authentic_data,
            checking_disabled: response.checking_disabled,
            question_name: response.question_name.to_string(),
            question_type: response.question_type.to_string(),
            answers: response.answers.iter().map(PyAnswer::from).collect(),
            authorities: response.authorities.iter().map(PyAnswer::from).collect(),
            additionals: response.additionals.iter().map(PyAnswer::from).collect(),
            wire_size: response.wire_size,
        }
    }
}
