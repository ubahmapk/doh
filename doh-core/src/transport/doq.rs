use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use hickory_proto::op::ResponseCode;
use hickory_proto::rr::RecordType;
use quinn::crypto::rustls::QuicClientConfig;
use tokio::sync::Mutex;
use tokio::time::timeout;

use crate::error::DohError;
use crate::message::{build_query, encode_query_doq, parse_response, ParsedResponse};
use crate::transport::util::{native_root_store, parse_host_port};
use crate::transport::Transport;

/// Default DNS-over-QUIC port, per RFC 9250 section 4.1.1 (shared with DoT).
const DEFAULT_PORT: u16 = 853;

/// ALPN token identifying DoQ during the QUIC/TLS handshake, per RFC 9250
/// section 4.1 / section 8.1.
const ALPN_DOQ: &[u8] = b"doq";

/// Connection + query timeout for a single DoQ round trip. There is no
/// fallback transport, so a slow/black-holing server must fail cleanly
/// rather than hang the caller forever.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// Cap on how much of a response we'll read into memory. RFC 9250 section
/// 4.2 reuses RFC 1035 section 4.2.2's 2-byte length-prefixed framing, so
/// any single message is capped at 64 KiB the same way DoT is.
const MAX_RESPONSE_BYTES: usize = 64 * 1024;

/// Lazily-created QUIC endpoint plus the currently-live connection (if
/// any), guarded together so concurrent `resolve()` calls don't race to
/// create two endpoints or two connections.
#[derive(Default)]
struct State {
    endpoint: Option<quinn::Endpoint>,
    connection: Option<quinn::Connection>,
}

/// A DNS-over-QUIC transport (RFC 9250) bound to a single `host:port`.
///
/// Unlike [`DotTransport`](crate::DotTransport), which opens a new
/// connection per query, `DoqTransport` reuses one pooled QUIC connection
/// across calls to `resolve()` — reconnecting transparently if the cached
/// connection has closed. This is the whole point of using QUIC over DoT:
/// cheap multiplexed reuse.
pub struct DoqTransport {
    host: String,
    port: u16,
    client_config: quinn::ClientConfig,
    state: Mutex<State>,
}

impl DoqTransport {
    fn addr_label(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn new(server_addr: impl AsRef<str>) -> Result<Self, DohError> {
        let (host, port) = parse_host_port(server_addr.as_ref(), DEFAULT_PORT);
        let root_store = native_root_store()?;

        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let mut tls_config = rustls::ClientConfig::builder_with_provider(provider)
            // QUIC requires TLS 1.3; unlike DoT, TLS 1.2 is not an option.
            .with_protocol_versions(&[&rustls::version::TLS13])
            .expect("aws-lc-rs provider supports TLS 1.3")
            .with_root_certificates(root_store)
            .with_no_client_auth();
        tls_config.alpn_protocols = vec![ALPN_DOQ.to_vec()];

        let quic_crypto = QuicClientConfig::try_from(tls_config)
            .map_err(|e| DohError::quic(format!("{host}:{port}"), e))?;
        let client_config = quinn::ClientConfig::new(Arc::new(quic_crypto));

        Ok(Self {
            host,
            port,
            client_config,
            state: Mutex::new(State::default()),
        })
    }

    /// Return the pooled connection if it's still open, otherwise establish
    /// a new one (creating the underlying `quinn::Endpoint` on first use)
    /// and cache it for subsequent calls.
    async fn get_or_connect(&self) -> Result<quinn::Connection, DohError> {
        let mut state = self.state.lock().await;

        if let Some(conn) = &state.connection {
            if conn.close_reason().is_none() {
                return Ok(conn.clone());
            }
        }

        let addr_label = self.addr_label();
        let socket_addr = self.resolve_addr().await?;

        if state.endpoint.is_none() {
            let bind_addr: SocketAddr = if socket_addr.is_ipv4() {
                "0.0.0.0:0".parse().expect("valid IPv4 wildcard")
            } else {
                "[::]:0".parse().expect("valid IPv6 wildcard")
            };
            let mut endpoint =
                quinn::Endpoint::client(bind_addr).map_err(|source| DohError::Io {
                    addr: addr_label.clone(),
                    source,
                })?;
            endpoint.set_default_client_config(self.client_config.clone());
            state.endpoint = Some(endpoint);
        }

        let connecting = state
            .endpoint
            .as_ref()
            .expect("endpoint just initialized above")
            .connect(socket_addr, &self.host)
            .map_err(|e| DohError::quic(&addr_label, e))?;
        let connection = connecting
            .await
            .map_err(|e| DohError::quic(&addr_label, e))?;

        state.connection = Some(connection.clone());
        Ok(connection)
    }

    async fn resolve_addr(&self) -> Result<SocketAddr, DohError> {
        let addr_label = self.addr_label();
        tokio::net::lookup_host((self.host.as_str(), self.port))
            .await
            .map_err(|source| DohError::Io {
                addr: addr_label.clone(),
                source,
            })?
            .next()
            .ok_or_else(|| DohError::invalid_server_address(&addr_label, "no addresses found"))
    }
}

#[async_trait]
impl Transport for DoqTransport {
    async fn resolve(
        &self,
        name: &str,
        record_type: RecordType,
    ) -> Result<ParsedResponse, DohError> {
        let query = build_query(name, record_type)?;
        // RFC 9250 section 4.2.1: the DNS message ID MUST be 0 on the wire.
        let wire = encode_query_doq(&query)?;

        let response_bytes = timeout(REQUEST_TIMEOUT, self.query_over_quic(&wire))
            .await
            .map_err(|_| DohError::Timeout {
                addr: self.addr_label(),
            })??;

        let parsed = parse_response(&self.addr_label(), &response_bytes)?;

        match parsed.response_code {
            ResponseCode::NoError | ResponseCode::NXDomain => Ok(parsed),
            code => Err(DohError::Dns {
                url: self.addr_label(),
                code,
            }),
        }
    }
}

impl DoqTransport {
    async fn query_over_quic(&self, wire: &[u8]) -> Result<Vec<u8>, DohError> {
        let addr_label = self.addr_label();
        let connection = self.get_or_connect().await?;

        // RFC 9250 section 4.2: each query goes on its own new
        // client-initiated bidirectional stream.
        let (mut send, mut recv) = connection
            .open_bi()
            .await
            .map_err(|e| DohError::quic(&addr_label, e))?;

        // Same 2-byte length-prefixed framing as DoT (RFC 9250 section 4.2).
        let len = u16::try_from(wire.len()).map_err(|_| DohError::MessageTooLarge {
            addr: addr_label.clone(),
            reason: format!("query is {} bytes", wire.len()),
        })?;
        let mut framed = Vec::with_capacity(2 + wire.len());
        framed.extend_from_slice(&len.to_be_bytes());
        framed.extend_from_slice(wire);
        send.write_all(&framed)
            .await
            .map_err(|e| DohError::quic(&addr_label, e))?;
        // The client MUST signal STREAM FIN after sending the query; the
        // server won't respond until it sees this.
        send.finish().map_err(|e| DohError::quic(&addr_label, e))?;

        let mut len_buf = [0u8; 2];
        recv.read_exact(&mut len_buf)
            .await
            .map_err(|e| DohError::quic(&addr_label, e))?;
        let response_len = u16::from_be_bytes(len_buf) as usize;
        if response_len > MAX_RESPONSE_BYTES {
            return Err(DohError::MessageTooLarge {
                addr: addr_label,
                reason: format!("server announced a {response_len}-byte response"),
            });
        }

        let mut response_buf = vec![0u8; response_len];
        recv.read_exact(&mut response_buf)
            .await
            .map_err(|e| DohError::quic(&addr_label, e))?;

        Ok(response_buf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Live network test against a real public DoQ resolver — `#[ignore]`d
    /// so CI (which doesn't opt into `--ignored`) doesn't depend on
    /// network access. Run manually with
    /// `cargo test -p doh-core --lib transport::doq -- --ignored`.
    ///
    /// Confirms two sequential `resolve()` calls reuse the same pooled
    /// QUIC connection (via `stable_id()`, a per-connection identifier)
    /// rather than reconnecting each time — the whole point of choosing a
    /// pooled connection model over DoT's per-query one.
    #[tokio::test]
    #[ignore]
    async fn resolve_reuses_the_same_connection_across_calls() {
        let transport = DoqTransport::new("dns.adguard.com").expect("valid transport");

        transport
            .resolve("example.com", RecordType::A)
            .await
            .expect("first query succeeds");
        let first_id = transport
            .state
            .lock()
            .await
            .connection
            .as_ref()
            .expect("connection cached after first query")
            .stable_id();

        transport
            .resolve("example.org", RecordType::A)
            .await
            .expect("second query succeeds");
        let second_id = transport
            .state
            .lock()
            .await
            .connection
            .as_ref()
            .expect("connection cached after second query")
            .stable_id();

        assert_eq!(
            first_id, second_id,
            "connection should be reused, not reopened"
        );
    }
}
