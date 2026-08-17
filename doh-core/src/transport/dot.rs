use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use hickory_proto::op::ResponseCode;
use hickory_proto::rr::RecordType;
use rustls_pki_types::ServerName;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_rustls::TlsConnector;

use crate::error::DohError;
use crate::message::{build_query, encode_query, parse_response, ParsedResponse};
use crate::transport::util::{
    check_response_size, frame_message, native_root_store, parse_host_port,
};
use crate::transport::Transport;

/// Default DNS-over-TLS port, per RFC 7858 section 3.1.
const DEFAULT_PORT: u16 = 853;

/// Connection + query timeout for a single DoT round trip. There is no
/// fallback transport, so a slow/black-holing server must fail cleanly
/// rather than hang the caller forever.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(10);

/// A DNS-over-TLS transport (RFC 7858) bound to a single `host:port`.
///
/// The server is specified as `hostname:port` (or bare `hostname`, which
/// defaults to port 853): the same hostname is used both to resolve the
/// connection address via the OS resolver and to validate the server's TLS
/// certificate. This means finding the DoT server's IP isn't itself
/// protected by DoT (the OS resolver is trusted for that step) — only the
/// DNS queries sent *to* that server are.
///
/// Opens a new TCP+TLS connection per query; it does not pipeline multiple
/// queries over one connection.
pub struct DotTransport {
    host: String,
    port: u16,
    tls_config: Arc<rustls::ClientConfig>,
}

impl DotTransport {
    fn addr_label(&self) -> String {
        format!("{}:{}", self.host, self.port)
    }

    pub fn new(server_addr: impl AsRef<str>) -> Result<Self, DohError> {
        let (host, port) = parse_host_port(server_addr.as_ref(), DEFAULT_PORT);
        let root_store = native_root_store()?;

        let provider = Arc::new(rustls::crypto::aws_lc_rs::default_provider());
        let tls_config = rustls::ClientConfig::builder_with_provider(provider)
            .with_safe_default_protocol_versions()
            .expect("aws-lc-rs provider supports the default TLS protocol versions")
            .with_root_certificates(root_store)
            .with_no_client_auth();

        Ok(Self {
            host,
            port,
            tls_config: Arc::new(tls_config),
        })
    }
}

#[async_trait]
impl Transport for DotTransport {
    async fn resolve(
        &self,
        name: &str,
        record_type: RecordType,
    ) -> Result<ParsedResponse, DohError> {
        log::debug!("dot: querying {name} {record_type} via {}", self.addr_label());

        let query = build_query(name, record_type)?;
        let wire = encode_query(&query)?;

        let response_bytes = timeout(REQUEST_TIMEOUT, self.query_over_tls(&wire))
            .await
            .map_err(|_| {
                log::debug!("dot: {} timed out", self.addr_label());
                DohError::Timeout {
                    addr: self.addr_label(),
                }
            })??;
        log::trace!("dot: received {} response bytes", response_bytes.len());

        let parsed = parse_response(&self.addr_label(), &response_bytes)?;

        match parsed.response_code {
            ResponseCode::NoError | ResponseCode::NXDomain => Ok(parsed),
            code => {
                log::debug!("dot: {} answered with {code:?}", self.addr_label());
                Err(DohError::Dns {
                    url: self.addr_label(),
                    code,
                })
            }
        }
    }
}

impl DotTransport {
    async fn query_over_tls(&self, wire: &[u8]) -> Result<Vec<u8>, DohError> {
        let addr_label = self.addr_label();
        let io_err = |source: std::io::Error| DohError::Io {
            addr: addr_label.clone(),
            source,
        };

        let tcp = TcpStream::connect((self.host.as_str(), self.port))
            .await
            .map_err(io_err)?;

        let server_name = ServerName::try_from(self.host.clone())
            .map_err(|e| DohError::invalid_server_address(&addr_label, e))?;
        let connector = TlsConnector::from(self.tls_config.clone());
        let mut tls = connector.connect(server_name, tcp).await.map_err(io_err)?;

        // RFC 1035 section 4.2.2 / RFC 7858 section 3.3: each DNS message
        // over a stream transport is prefixed with a 2-byte length.
        let framed = frame_message(wire, &addr_label)?;
        tls.write_all(&framed).await.map_err(io_err)?;
        tls.flush().await.map_err(io_err)?;

        let mut len_buf = [0u8; 2];
        tls.read_exact(&mut len_buf).await.map_err(io_err)?;
        let response_len = u16::from_be_bytes(len_buf) as usize;
        check_response_size(response_len, &addr_label)?;

        let mut response_buf = vec![0u8; response_len];
        tls.read_exact(&mut response_buf).await.map_err(io_err)?;

        Ok(response_buf)
    }
}
