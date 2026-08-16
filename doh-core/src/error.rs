use std::fmt;

/// User-facing errors. Every variant renders as a clear, actionable message —
/// there is no classic-DNS fallback path, so these are the only signal the
/// caller gets when a secure transport fails.
#[derive(Debug, thiserror::Error)]
pub enum DohError {
    #[error(
        "could not reach DoH server {url}: {source}\n\
         hint: check the URL and your network connection; no classic DNS fallback is attempted"
    )]
    TransportFailed {
        url: String,
        #[source]
        source: reqwest::Error,
    },

    #[error("DoH server {url} returned HTTP {status}: {body}")]
    HttpStatus {
        url: String,
        status: u16,
        body: String,
    },

    #[error("DoH server {url} returned a malformed DNS message: {source}")]
    InvalidResponse {
        url: String,
        #[source]
        source: hickory_proto::serialize::binary::DecodeError,
    },

    #[error("failed to build DNS query for {name}: {source}")]
    QueryBuild {
        name: String,
        #[source]
        source: hickory_proto::ProtoError,
    },

    #[error("invalid DoH server URL '{url}': {reason}")]
    InvalidServerUrl { url: String, reason: String },

    #[error("invalid domain name '{name}': {reason}")]
    InvalidName { name: String, reason: String },
}

impl DohError {
    pub fn invalid_server_url(url: impl Into<String>, reason: impl fmt::Display) -> Self {
        Self::InvalidServerUrl {
            url: url.into(),
            reason: reason.to_string(),
        }
    }

    pub fn invalid_name(name: impl Into<String>, reason: impl fmt::Display) -> Self {
        Self::InvalidName {
            name: name.into(),
            reason: reason.to_string(),
        }
    }
}
