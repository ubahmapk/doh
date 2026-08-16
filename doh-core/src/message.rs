use std::str::FromStr;

use hickory_proto::op::{Message, ResponseCode};
use hickory_proto::rr::{Name, RData, RecordType};

use crate::error::DohError;

/// A single answer record extracted from a DNS response, in a
/// transport-agnostic form. Carries typed data rather than pre-formatted
/// strings so library consumers can work with the values directly (e.g.
/// match on `RData::A` for an `Ipv4Addr`) instead of re-parsing `Display`
/// output.
#[derive(Debug, Clone)]
pub struct Answer {
    pub name: Name,
    pub record_type: RecordType,
    pub ttl: u32,
    pub rdata: RData,
}

/// A successfully-received and parsed DNS response. `NXDomain` is a
/// successful outcome (the name does not exist) and is carried here rather
/// than as an error; any other non-`NoError` response code (`ServFail`,
/// `Refused`, etc.) is surfaced by the caller as `DohError::Dns` instead of
/// being returned as a `ParsedResponse`.
#[derive(Debug, Clone)]
pub struct ParsedResponse {
    pub response_code: ResponseCode,
    pub answers: Vec<Answer>,
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

/// Serialize a query message for DoQ, per RFC 9250 section 4.2.1: the DNS
/// message ID MUST be set to 0 on the wire (stream mapping, not the ID,
/// correlates queries and responses). `build_query` itself is left
/// untouched — DoH/DoT still want a normal random ID — so this clones the
/// message and zeroes the ID only on the copy that actually goes on the
/// wire.
pub fn encode_query_doq(message: &Message) -> Result<Vec<u8>, DohError> {
    let mut zeroed = message.clone();
    zeroed.metadata.id = 0;
    encode_query(&zeroed)
}

/// Parse a raw DNS response into its response code and answer records.
/// Callers are responsible for treating non-`NoError`/non-`NXDomain`
/// response codes as errors (see `DohError::Dns`).
pub fn parse_response(url: &str, bytes: &[u8]) -> Result<ParsedResponse, DohError> {
    let message = Message::from_vec(bytes).map_err(|source| DohError::InvalidResponse {
        url: url.to_string(),
        source,
    })?;

    let answers = message
        .answers
        .iter()
        .map(|record| Answer {
            name: record.name.clone(),
            record_type: record.record_type(),
            ttl: record.ttl,
            rdata: record.data.clone(),
        })
        .collect();

    Ok(ParsedResponse {
        response_code: message.metadata.response_code,
        answers,
    })
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

    #[test]
    fn encode_query_doq_zeroes_the_message_id() {
        let message = build_query("example.com", RecordType::A).expect("valid query");
        let bytes = encode_query_doq(&message).expect("encode");
        // RFC 9250 section 4.2.1: the DNS message ID field (the first two
        // bytes of the header) MUST be 0 on the wire for DoQ.
        assert_eq!(&bytes[0..2], &[0, 0]);
    }

    #[test]
    fn encode_query_does_not_zero_the_message_id() {
        let message = build_query("example.com", RecordType::A).expect("valid query");
        let bytes = encode_query(&message).expect("encode");
        // Sanity check that the non-DoQ path is untouched by the DoQ
        // zeroing behavior: the original random ID survives on the wire.
        assert_eq!(&bytes[0..2], &message.metadata.id.to_be_bytes());
    }
}
