use crate::error::DohError;

/// Cap on how much of a response we'll read into memory, for every
/// stream-based transport (DoT, DoQ) that uses classic DNS message framing
/// (RFC 1035 section 4.2.2, reused for DoT by RFC 7858 section 3.3 and for
/// DoQ by RFC 9250 section 4.2): each message is prefixed with a 2-byte
/// length, capping any single message at 64 KiB.
pub(crate) const MAX_RESPONSE_BYTES: usize = 64 * 1024;

/// Build the 2-byte big-endian length-prefixed frame for `wire`, per RFC
/// 1035 section 4.2.2 (reused by DoT and DoQ). Errors if `wire` is too
/// large for a `u16` length field.
pub(crate) fn frame_message(wire: &[u8], addr: &str) -> Result<Vec<u8>, DohError> {
    let len = u16::try_from(wire.len()).map_err(|_| DohError::MessageTooLarge {
        addr: addr.to_string(),
        reason: format!("query is {} bytes", wire.len()),
    })?;
    let mut framed = Vec::with_capacity(2 + wire.len());
    framed.extend_from_slice(&len.to_be_bytes());
    framed.extend_from_slice(wire);
    Ok(framed)
}

/// Check a peer-announced response length against [`MAX_RESPONSE_BYTES`]
/// before reading that many bytes into memory.
pub(crate) fn check_response_size(len: usize, addr: &str) -> Result<(), DohError> {
    if len > MAX_RESPONSE_BYTES {
        return Err(DohError::MessageTooLarge {
            addr: addr.to_string(),
            reason: format!("server announced a {len}-byte response"),
        });
    }
    Ok(())
}

/// Parse `host:port` or bare `host` (defaulting to `default_port`).
/// Deliberately does not attempt to disambiguate a bracket-less IPv6
/// literal's embedded colons from a port separator; use a resolvable
/// hostname for those.
pub(crate) fn parse_host_port(addr: &str, default_port: u16) -> (String, u16) {
    match addr.rsplit_once(':') {
        Some((host, port_str)) if !host.is_empty() => match port_str.parse::<u16>() {
            Ok(port) => (host.to_string(), port),
            Err(_) => (addr.to_string(), default_port),
        },
        _ => (addr.to_string(), default_port),
    }
}

/// Load the OS trust store into a fresh [`rustls::RootCertStore`]. Shared by
/// every stream-based transport (DoT, DoQ) that builds its own
/// `rustls::ClientConfig`; `DohTransport` gets the equivalent behavior from
/// `reqwest`'s `rustls-native-certs` feature instead.
pub(crate) fn native_root_store() -> Result<rustls::RootCertStore, DohError> {
    let cert_result = rustls_native_certs::load_native_certs();
    if cert_result.certs.is_empty() {
        let reason = if cert_result.errors.is_empty() {
            "no native root certificates found".to_string()
        } else {
            format!("{:?}", cert_result.errors)
        };
        return Err(DohError::TlsConfig { reason });
    }

    let mut root_store = rustls::RootCertStore::empty();
    for cert in cert_result.certs {
        // Ignore individual malformed certs rather than failing the whole
        // store over one bad entry.
        let _ = root_store.add(cert);
    }

    Ok(root_store)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_host_port_splits_explicit_port() {
        assert_eq!(
            parse_host_port("dns.google:853", 853),
            ("dns.google".to_string(), 853)
        );
    }

    #[test]
    fn parse_host_port_defaults_when_no_port() {
        assert_eq!(
            parse_host_port("dns.google", 853),
            ("dns.google".to_string(), 853)
        );
    }

    #[test]
    fn frame_message_prefixes_a_2_byte_big_endian_length() {
        let wire = vec![0xAB; 300];
        let framed = frame_message(&wire, "test").unwrap();

        assert_eq!(framed.len(), 2 + wire.len());
        assert_eq!(&framed[0..2], &300u16.to_be_bytes());
        assert_eq!(&framed[2..], &wire[..]);
    }

    #[test]
    fn frame_message_empty_wire_is_a_valid_zero_length_frame() {
        let framed = frame_message(&[], "test").unwrap();
        assert_eq!(framed, vec![0, 0]);
    }

    #[test]
    fn frame_message_rejects_wire_larger_than_u16_max() {
        let wire = vec![0u8; u16::MAX as usize + 1];
        let err = frame_message(&wire, "test:853").unwrap_err();
        assert!(matches!(err, DohError::MessageTooLarge { .. }));
    }

    #[test]
    fn frame_message_accepts_wire_at_exactly_u16_max() {
        let wire = vec![0u8; u16::MAX as usize];
        let framed = frame_message(&wire, "test").unwrap();
        assert_eq!(&framed[0..2], &u16::MAX.to_be_bytes());
    }

    #[test]
    fn check_response_size_accepts_at_or_under_the_cap() {
        assert!(check_response_size(MAX_RESPONSE_BYTES, "test").is_ok());
        assert!(check_response_size(0, "test").is_ok());
    }

    #[test]
    fn check_response_size_rejects_over_the_cap() {
        let err = check_response_size(MAX_RESPONSE_BYTES + 1, "test:853").unwrap_err();
        assert!(matches!(err, DohError::MessageTooLarge { .. }));
    }
}
