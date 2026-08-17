use doh_core::OpCode;
use pyo3::prelude::*;

/// One resource record from an answer, authority, or additional section.
#[pyclass(skip_from_py_object)]
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

/// A successfully-received and parsed DNS response, mirroring every field
/// of `doh_core::ParsedResponse`. `response_code` is `"NOERROR"` or
/// `"NXDOMAIN"` on any response returned here -- every other response code
/// (`SERVFAIL`, `REFUSED`, ...) is raised as a `DohError` instead, matching
/// the Rust library's own behavior.
#[pyclass]
pub struct PyParsedResponse {
    #[pyo3(get)]
    id: u16,
    #[pyo3(get)]
    op_code: String,
    #[pyo3(get)]
    response_code: String,
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
            "ParsedResponse(id={}, op_code={:?}, response_code={:?}, question_name={:?}, \
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

/// Same stringification `doh-cli` already uses for its own JSON/text output
/// (`doh-cli/src/output.rs`'s `opcode_str`): `OpCode` has no useful
/// `Display`, so the known variants are named explicitly.
fn opcode_str(op: OpCode) -> &'static str {
    match op {
        OpCode::Query => "QUERY",
        OpCode::Status => "STATUS",
        OpCode::Notify => "NOTIFY",
        OpCode::Update => "UPDATE",
        _ => "UNKNOWN",
    }
}

impl From<doh_core::ParsedResponse> for PyParsedResponse {
    fn from(response: doh_core::ParsedResponse) -> Self {
        Self {
            id: response.id,
            op_code: opcode_str(response.op_code).to_string(),
            response_code: response.response_code.to_string(),
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
