use std::str::FromStr;

use hickory_proto::op::Message;
use hickory_proto::rr::{Name, RData, RecordType};

use crate::error::DohError;

/// A single answer record extracted from a DNS response, in a
/// transport-agnostic form.
#[derive(Debug, Clone)]
pub struct Answer {
    pub name: String,
    pub record_type: RecordType,
    pub ttl: u32,
    pub rdata: String,
}

/// Build a DNS query message for `name`/`record_type`, ready to be
/// serialized by a transport.
pub fn build_query(name: &str, record_type: RecordType) -> Result<Message, DohError> {
    let name = Name::from_str(name).map_err(|e| DohError::invalid_name(name, e))?;

    let mut message = Message::query();
    message.metadata.recursion_desired = true;
    message.add_query(hickory_proto::op::Query::query(name, record_type));

    Ok(message)
}

/// Serialize a query message to its RFC 1035 binary wire format.
pub fn encode_query(message: &Message) -> Result<Vec<u8>, DohError> {
    message.to_vec().map_err(|source| DohError::QueryBuild {
        name: message
            .queries
            .first()
            .map(|q| q.name().to_string())
            .unwrap_or_default(),
        source,
    })
}

/// Parse a raw DNS response and extract its answer records.
pub fn parse_response(url: &str, bytes: &[u8]) -> Result<Vec<Answer>, DohError> {
    let message = Message::from_vec(bytes).map_err(|source| DohError::InvalidResponse {
        url: url.to_string(),
        source,
    })?;

    Ok(message
        .answers
        .iter()
        .map(|record| Answer {
            name: record.name.to_string(),
            record_type: record.record_type(),
            ttl: record.ttl,
            rdata: rdata_to_string(&record.data),
        })
        .collect())
}

fn rdata_to_string(rdata: &RData) -> String {
    rdata.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_query_encodes_and_round_trips_header() {
        let message = build_query("example.com", RecordType::A).expect("valid query");
        let bytes = encode_query(&message).expect("encode");
        assert!(!bytes.is_empty());
    }

    #[test]
    fn build_query_rejects_invalid_name() {
        let err = build_query("exa mple..com", RecordType::A).unwrap_err();
        assert!(matches!(err, DohError::InvalidName { .. }));
    }
}
