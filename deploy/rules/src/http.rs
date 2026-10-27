use std::time::Duration;

use base64::Engine as _;

/// Builds the HTTPS agent for publish uploads with the proxy from the environment.
pub fn publish_agent() -> ureq::Agent {
    publish_agent_with_proxy(ureq::Proxy::try_from_env())
}

/// Builds the HTTPS agent for publish uploads with an explicit proxy.
pub fn publish_agent_with_proxy(proxy: Option<ureq::Proxy>) -> ureq::Agent {
    agent_config(proxy)
        .timeout_global(Some(Duration::from_secs(600)))
        .build()
        .into()
}

/// Builds the HTTPS agent for large blob uploads with the proxy from the environment.
pub fn upload_agent() -> ureq::Agent {
    upload_agent_with_proxy(ureq::Proxy::try_from_env())
}

/// Builds the HTTPS agent for large blob uploads: no global deadline, only stall timeouts.
pub fn upload_agent_with_proxy(proxy: Option<ureq::Proxy>) -> ureq::Agent {
    agent_config(proxy)
        .timeout_per_read(Some(Duration::from_secs(300)))
        .timeout_per_write(Some(Duration::from_secs(300)))
        .build()
        .into()
}

fn agent_config(
    proxy: Option<ureq::Proxy>,
) -> ureq::config::ConfigBuilder<ureq::typestate::AgentScope> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    let tls = ureq::tls::TlsConfig::builder()
        .provider(ureq::tls::TlsProvider::Rustls)
        .root_certs(ureq::tls::RootCerts::PlatformVerifier)
        .build();
    ureq::Agent::config_builder()
        .tls_config(tls)
        .max_redirects(0)
        .http_status_as_error(false)
        .timeout_connect(Some(Duration::from_secs(30)))
        .proxy(proxy)
}

/// Encodes one file as a single-part multipart/form-data body and returns its content type.
pub fn multipart_single_file(field: &str, filename: &str, bytes: &[u8]) -> (String, Vec<u8>) {
    use rand::Rng as _;
    let boundary = loop {
        let token: u128 = rand::thread_rng().gen();
        let candidate = format!("dx-boundary-{token:032x}");
        if !bytes
            .windows(candidate.len())
            .any(|window| window == candidate.as_bytes())
        {
            break candidate;
        }
    };
    let mut body = format!(
        "--{boundary}\r\nContent-Disposition: form-data; name=\"{field}\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
    )
    .into_bytes();
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    (format!("multipart/form-data; boundary={boundary}"), body)
}

/// Returns the Authorization value `Bearer base64(user:pass)`.
pub fn bearer_basic(user: &str, pass: &str) -> String {
    let token = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
    format!("Bearer {token}")
}

/// Returns the Authorization value `Basic base64(user:pass)`.
pub fn basic_auth(user: &str, pass: &str) -> String {
    let token = base64::engine::general_purpose::STANDARD.encode(format!("{user}:{pass}"));
    format!("Basic {token}")
}

/// Percent-encodes one query value, keeping only unreserved characters.
pub fn query_escape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            out.push(char::from(byte));
        } else {
            out.push_str(&format!("%{byte:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn http_multipart_boundary_absent_from_payload() {
        let payload = b"dx-boundary-0000 payload bytes \r\n--".to_vec();
        let (content_type, body) = multipart_single_file("bundle", "bundle.zip", &payload);
        let boundary = content_type
            .strip_prefix("multipart/form-data; boundary=")
            .expect("boundary");
        assert!(!payload
            .windows(boundary.len())
            .any(|window| window == boundary.as_bytes()));
        let head = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"bundle\"; filename=\"bundle.zip\"\r\nContent-Type: application/octet-stream\r\n\r\n"
        );
        let tail = format!("\r\n--{boundary}--\r\n");
        assert!(body.starts_with(head.as_bytes()));
        assert!(body.ends_with(tail.as_bytes()));
        assert_eq!(&body[head.len()..body.len() - tail.len()], &payload[..]);
    }

    #[test]
    fn http_bearer_basic_known_vector() {
        assert_eq!(bearer_basic("user", "pass"), "Bearer dXNlcjpwYXNz");
    }

    #[test]
    fn http_basic_auth_known_vector() {
        assert_eq!(basic_auth("user", "pass"), "Basic dXNlcjpwYXNz");
    }

    #[test]
    fn http_query_escape_keeps_unreserved() {
        assert_eq!(
            query_escape("com.example:demo:1.0+b"),
            "com.example%3Ademo%3A1.0%2Bb"
        );
    }
}
