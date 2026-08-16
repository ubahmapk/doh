pub mod doh;

use async_trait::async_trait;
use hickory_proto::rr::RecordType;

use crate::error::DohError;
use crate::message::ParsedResponse;

/// A DNS transport capable of resolving a single query. Every implementation
/// (DoH today, DoT/DoQ/ODoH/DNSCrypt later) speaks a secure protocol only —
/// none of them fall back to classic plaintext UDP/TCP DNS. On failure they
/// return a `DohError` describing exactly what went wrong.
///
/// A successful `NXDOMAIN` is still an `Ok(ParsedResponse)` (with an empty
/// `answers` and `response_code: ResponseCode::NXDomain`) since the server
/// answered correctly — it just says the name doesn't exist. Any other
/// non-`NoError` response code (`ServFail`, `Refused`, ...) comes back as
/// `Err(DohError::Dns { .. })`.
#[async_trait]
pub trait Transport {
    async fn resolve(
        &self,
        name: &str,
        record_type: RecordType,
    ) -> Result<ParsedResponse, DohError>;
}
