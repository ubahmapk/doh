use std::time::Duration;

use async_trait::async_trait;
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hickory_proto::op::ResponseCode;
use hickory_proto::rr::RecordType;
use reqwest::header::{ACCEPT, CONTENT_TYPE};
use reqwest::{redirect, Client, Url};

use crate::error::DohError;
use crate::message::{build_query, encode_query, parse_response, ParsedResponse};
use crate::transport::Transport;

/// application/dns-message, per RFC 8484 section 4.
const DNS_MESSAGE_MIME: &str = "application/dns-message";

/// Request timeout for a single DoH round trip. There is no fallback
/// transport, so a slow/black-holing server must fail cleanly rather than
/// hang the caller forever.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Cap on how much of a response body we'll read into memory. RFC 8484
/// doesn't define a size limit; classic DNS-over-TCP framing caps a message
/// at 64 KiB, so we reuse that as a deliberate ceiling against a
/// malicious/compromised server sending an oversized body.
const MAX_RESPONSE_BYTES: u64 = 64 * 1024;

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
    server_url: Url,
    method: HttpMethod,
}

impl DohTransport {
    /// Build a transport for `server_url` (e.g.
    /// `https://dns.google/dns-query`), validating it's a well-formed
    /// HTTPS URL up front rather than failing lazily on first query.
    ///
    /// The underlying HTTP client never follows redirects (a compromised or
    /// hostile DoH endpoint could otherwise silently bounce queries to a
    /// third-party host the caller never chose) and applies a fixed request
    /// timeout.
    pub fn new(server_url: impl AsRef<str>, method: HttpMethod) -> Result<Self, DohError> {
        let server_url = server_url.as_ref();
        let parsed =
            Url::parse(server_url).map_err(|e| DohError::invalid_server_url(server_url, e))?;

        if parsed.scheme() != "https" {
            return Err(DohError::invalid_server_url(
                server_url,
                "DoH server URL must use https://",
            ));
        }

        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(redirect::Policy::none())
            .build()
            .map_err(|source| DohError::ClientBuild { source })?;

        Ok(Self {
            client,
            server_url: parsed,
            method,
        })
    }

    /// Test-only constructor that skips the `https://` requirement, so unit
    /// tests can point at a local [`wiremock`](https://docs.rs/wiremock)
    /// server (which speaks plain HTTP). Not exposed outside this crate.
    #[cfg(test)]
    pub(crate) fn new_for_test(server_url: &str, method: HttpMethod) -> Self {
        let client = Client::builder()
            .timeout(REQUEST_TIMEOUT)
            .redirect(redirect::Policy::none())
            .build()
            .expect("client build");

        Self {
            client,
            server_url: Url::parse(server_url).expect("valid test URL"),
            method,
        }
    }
}

#[async_trait]
impl Transport for DohTransport {
    async fn resolve(
        &self,
        name: &str,
        record_type: RecordType,
    ) -> Result<ParsedResponse, DohError> {
        log::debug!(
            "doh: querying {name} {record_type} via {} {}",
            match self.method {
                HttpMethod::Get => "GET",
                HttpMethod::Post => "POST",
            },
            self.server_url
        );

        let query = build_query(name, record_type)?;
        let wire = encode_query(&query)?;

        let response = match self.method {
            HttpMethod::Post => {
                self.client
                    .post(self.server_url.clone())
                    .header(CONTENT_TYPE, DNS_MESSAGE_MIME)
                    .header(ACCEPT, DNS_MESSAGE_MIME)
                    .body(wire)
                    .send()
                    .await
            }
            HttpMethod::Get => {
                let encoded = URL_SAFE_NO_PAD.encode(&wire);
                self.client
                    .get(self.server_url.clone())
                    .query(&[("dns", encoded)])
                    .header(ACCEPT, DNS_MESSAGE_MIME)
                    .send()
                    .await
            }
        }
        .map_err(|source| {
            log::debug!("doh: request to {} failed: {source}", self.server_url);
            DohError::TransportFailed {
                url: self.server_url.to_string(),
                source,
            }
        })?;

        let status = response.status();
        if !status.is_success() {
            let body = read_capped_text(response).await;
            log::debug!("doh: {} returned HTTP {status}", self.server_url);
            return Err(DohError::HttpStatus {
                url: self.server_url.to_string(),
                status: status.as_u16(),
                body,
            });
        }

        let bytes =
            read_capped_bytes(response)
                .await
                .map_err(|source| DohError::TransportFailed {
                    url: self.server_url.to_string(),
                    source,
                })?;
        log::trace!("doh: received {} response bytes", bytes.len());

        let parsed = parse_response(self.server_url.as_str(), &bytes)?;

        match parsed.response_code {
            ResponseCode::NoError | ResponseCode::NXDomain => Ok(parsed),
            code => {
                log::debug!("doh: {} answered with {code:?}", self.server_url);
                Err(DohError::Dns {
                    url: self.server_url.to_string(),
                    code,
                })
            }
        }
    }
}

/// Read up to `MAX_RESPONSE_BYTES` of a response body. Truncates rather
/// than erroring on an oversized body, since a truncated DNS message will
/// fail to parse cleanly on its own and produce a meaningful error either
/// way; this just bounds how much we buffer in memory to get there.
async fn read_capped_bytes(response: reqwest::Response) -> reqwest::Result<Vec<u8>> {
    use futures_util::StreamExt;

    let mut stream = response.bytes_stream();
    let mut buf = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk?;
        let remaining = MAX_RESPONSE_BYTES.saturating_sub(buf.len() as u64) as usize;
        if remaining == 0 {
            break;
        }
        let take = remaining.min(chunk.len());
        buf.extend_from_slice(&chunk[..take]);
        if take < chunk.len() {
            break;
        }
    }
    Ok(buf)
}

/// Same capping as [`read_capped_bytes`], for error-path bodies that are
/// displayed as text rather than parsed as DNS wire format. Untrusted
/// control characters (e.g. terminal escape sequences) are stripped before
/// the caller ever sees this text.
async fn read_capped_text(response: reqwest::Response) -> String {
    let bytes = read_capped_bytes(response).await.unwrap_or_default();
    let text = String::from_utf8_lossy(&bytes);
    sanitize_for_terminal(&text)
}

fn sanitize_for_terminal(input: &str) -> String {
    input
        .chars()
        .filter(|c| !c.is_control() || *c == '\n' || *c == '\t')
        .collect()
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;
    use std::str::FromStr;

    use hickory_proto::op::{Message, ResponseCode};
    use hickory_proto::rr::rdata::A;
    use hickory_proto::rr::{Name, RData, Record};
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    use super::*;

    /// Build a wire-format DNS response with one A answer for `example.com`.
    fn a_response_bytes() -> Vec<u8> {
        let query = build_query("example.com", RecordType::A).unwrap();
        let mut response = Message::response(query.metadata.id, query.metadata.op_code);
        response.add_query(query.queries[0].clone());
        response.add_answer(Record::from_rdata(
            Name::from_str("example.com.").unwrap(),
            300,
            RData::A(A(Ipv4Addr::new(93, 184, 216, 34))),
        ));
        response.to_vec().unwrap()
    }

    #[tokio::test]
    async fn get_request_encodes_query_as_base64url_with_no_body() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/dns-query"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", DNS_MESSAGE_MIME)
                    .set_body_bytes(a_response_bytes()),
            )
            .mount(&mock_server)
            .await;

        let transport = DohTransport::new_for_test(
            &format!("{}/dns-query", mock_server.uri()),
            HttpMethod::Get,
        );
        let response = transport
            .resolve("example.com", RecordType::A)
            .await
            .unwrap();
        assert_eq!(response.response_code, ResponseCode::NoError);
        assert_eq!(response.answers.len(), 1);

        let requests = mock_server.received_requests().await.unwrap();
        assert_eq!(requests.len(), 1);
        let request = &requests[0];
        assert!(request.body.is_empty(), "GET must not send a body");
        let (_, encoded) = request
            .url
            .query_pairs()
            .find(|(k, _)| k == "dns")
            .expect("dns query param present");
        let decoded = URL_SAFE_NO_PAD
            .decode(encoded.as_bytes())
            .expect("valid base64url");
        assert!(
            Message::from_vec(&decoded).is_ok(),
            "dns param decodes to a valid query"
        );
    }

    #[tokio::test]
    async fn post_request_sends_binary_body_with_content_type() {
        let mock_server = MockServer::start().await;
        Mock::given(method("POST"))
            .and(path("/dns-query"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", DNS_MESSAGE_MIME)
                    .set_body_bytes(a_response_bytes()),
            )
            .mount(&mock_server)
            .await;

        let transport = DohTransport::new_for_test(
            &format!("{}/dns-query", mock_server.uri()),
            HttpMethod::Post,
        );
        let response = transport
            .resolve("example.com", RecordType::A)
            .await
            .unwrap();
        assert_eq!(response.answers.len(), 1);

        let requests = mock_server.received_requests().await.unwrap();
        let request = &requests[0];
        assert!(
            !request.body.is_empty(),
            "POST must send the query as the body"
        );
        assert!(
            Message::from_vec(&request.body).is_ok(),
            "body is a valid DNS query"
        );
        assert_eq!(
            request.headers.get("content-type").unwrap(),
            DNS_MESSAGE_MIME
        );
    }

    #[tokio::test]
    async fn non_2xx_status_maps_to_http_status_error() {
        let mock_server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/dns-query"))
            .respond_with(ResponseTemplate::new(500).set_body_string("internal error"))
            .mount(&mock_server)
            .await;

        let transport = DohTransport::new_for_test(
            &format!("{}/dns-query", mock_server.uri()),
            HttpMethod::Get,
        );
        let err = transport
            .resolve("example.com", RecordType::A)
            .await
            .unwrap_err();
        match err {
            DohError::HttpStatus { status, body, .. } => {
                assert_eq!(status, 500);
                assert_eq!(body, "internal error");
            }
            other => panic!("expected HttpStatus, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn non_success_rcode_maps_to_dns_error() {
        let mock_server = MockServer::start().await;
        let query = build_query("example.com", RecordType::A).unwrap();
        let mut response = Message::response(query.metadata.id, query.metadata.op_code);
        response.metadata.response_code = ResponseCode::ServFail;
        let bytes = response.to_vec().unwrap();

        Mock::given(method("GET"))
            .and(path("/dns-query"))
            .respond_with(
                ResponseTemplate::new(200)
                    .insert_header("content-type", DNS_MESSAGE_MIME)
                    .set_body_bytes(bytes),
            )
            .mount(&mock_server)
            .await;

        let transport = DohTransport::new_for_test(
            &format!("{}/dns-query", mock_server.uri()),
            HttpMethod::Get,
        );
        let err = transport
            .resolve("example.com", RecordType::A)
            .await
            .unwrap_err();
        assert!(matches!(
            err,
            DohError::Dns {
                code: ResponseCode::ServFail,
                ..
            }
        ));
    }
}
