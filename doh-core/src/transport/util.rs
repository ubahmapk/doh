use crate::error::DohError;

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
}
