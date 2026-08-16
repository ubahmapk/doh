use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hickory_proto::rr::RecordType;
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use reqwest::Client;

use crate::error::DohError;
use crate::message::{build_query, encode_query, parse_response, Answer};
use crate::transport::Transport;

/// application/dns-message, per RFC 8484 section 4.
const DNS_MESSAGE_MIME: &str = "application/dns-message";

/// HTTP method used to send the query, per RFC 8484 section 4.1 (POST) and
/// section 4.1.1 (GET).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HttpMethod {
    Get,
    Post,
}

/// A DNS-over-HTTPS transport (RFC 8484) bound to a single DoH server URL.
pub struct DohTransport {
    client: Client,
    server_url: String,
    method: HttpMethod,
}

impl DohTransport {
    /// Build a transport for `server_url` (e.g.
    /// `https://dns.google/dns-query`), validating it's a well-formed
    /// HTTPS URL up front rather than failing lazily on first query.
    pub fn new(server_url: impl Into<String>, method: HttpMethod) -> Result<Self, DohError> {
        let server_url = server_url.into();
        let parsed = reqwest::Url::parse(&server_url)
            .map_err(|e| DohError::invalid_server_url(&server_url, e))?;

        if parsed.scheme() != "https" {
            return Err(DohError::invalid_server_url(
                &server_url,
                "DoH server URL must use https://",
            ));
        }

        Ok(Self {
            client: Client::new(),
            server_url,
            method,
        })
    }
}

#[async_trait]
impl Transport for DohTransport {
    async fn resolve(&self, name: &str, record_type: RecordType) -> Result<Vec<Answer>, DohError> {
        let query = build_query(name, record_type)?;
        let wire = encode_query(&query)?;

        let response = match self.method {
            HttpMethod::Post => {
                self.client
                    .post(&self.server_url)
                    .header(CONTENT_TYPE, DNS_MESSAGE_MIME)
                    .header(ACCEPT, DNS_MESSAGE_MIME)
                    .body(wire)
                    .send()
                    .await
            }
            HttpMethod::Get => {
                let encoded = URL_SAFE_NO_PAD.encode(&wire);
                self.client
                    .get(&self.server_url)
                    .query(&[("dns", encoded)])
                    .header(ACCEPT, DNS_MESSAGE_MIME)
                    .send()
                    .await
            }
        }
        .map_err(|source| DohError::TransportFailed {
            url: self.server_url.clone(),
            source,
        })?;

        let status = response.status();
        if !status.is_success() {
            let body = response.text().await.unwrap_or_default();
            return Err(DohError::HttpStatus {
                url: self.server_url.clone(),
                status: status.as_u16(),
                body,
            });
        }

        let bytes = response
            .bytes()
            .await
            .map_err(|source| DohError::TransportFailed {
                url: self.server_url.clone(),
                source,
            })?;

        parse_response(&self.server_url, &bytes)
    }
}
