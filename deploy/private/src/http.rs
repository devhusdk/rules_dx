use std::time::Duration;

use base64::Engine as _;
use percent_encoding::{AsciiSet, NON_ALPHANUMERIC, utf8_percent_encode};

/// Settings for publish and upload calls: plain http for loopback registries, a stall
/// timeout instead of an overall deadline, and redirects returned to the caller.
fn settings() -> dx_http::Settings {
    dx_http::Settings {
        https_only: false,
        idle_timeout: Duration::from_secs(300),
        follow_redirects: false,
        ..dx_http::Settings::default()
    }
}

/// Builds the client for publish and upload calls with proxy and CA from the environment.
pub fn client() -> Result<dx_http::Client, String> {
    dx_http::Client::new(settings()).map_err(|error| error.to_string())
}

/// A client that ignores the environment and tries each request once.
#[cfg(test)]
pub fn test_client() -> dx_http::Client {
    let settings = dx_http::Settings {
        attempts: 1,
        ..settings()
    };
    dx_http::Client::with_env(settings, |_| None).expect("http client")
}

/// Encodes one file as a single-part multipart/form-data body and returns its content type.
pub fn multipart_single_file(field: &str, filename: &str, bytes: &[u8]) -> (String, Vec<u8>) {
    multipart_form(&[], field, filename, bytes)
}

/// Encodes text fields followed by one file as a multipart/form-data body and returns its content type.
pub fn multipart_form(
    fields: &[(&str, &str)],
    file_field: &str,
    filename: &str,
    bytes: &[u8],
) -> (String, Vec<u8>) {
    use rand::Rng as _;
    let boundary = loop {
        let token: u128 = rand::thread_rng().r#gen();
        let candidate = format!("dx-boundary-{token:032x}");
        let clash = |hay: &[u8]| {
            hay.windows(candidate.len())
                .any(|window| window == candidate.as_bytes())
        };
        if !clash(bytes) && !fields.iter().any(|(_, value)| clash(value.as_bytes())) {
            break candidate;
        }
    };
    let mut body = Vec::new();
    for (name, value) in fields {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"{name}\"\r\n\r\n{value}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"{file_field}\"; filename=\"{filename}\"\r\nContent-Type: application/octet-stream\r\n\r\n"
        )
        .as_bytes(),
    );
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

const UNRESERVED: &AsciiSet = &NON_ALPHANUMERIC
    .remove(b'-')
    .remove(b'.')
    .remove(b'_')
    .remove(b'~');

/// Percent-encodes one query value, keeping only unreserved characters.
pub fn query_escape(value: &str) -> String {
    utf8_percent_encode(value, UNRESERVED).to_string()
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
        assert!(
            !payload
                .windows(boundary.len())
                .any(|window| window == boundary.as_bytes())
        );
        let head = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"bundle\"; filename=\"bundle.zip\"\r\nContent-Type: application/octet-stream\r\n\r\n"
        );
        let tail = format!("\r\n--{boundary}--\r\n");
        assert!(body.starts_with(head.as_bytes()));
        assert!(body.ends_with(tail.as_bytes()));
        assert_eq!(&body[head.len()..body.len() - tail.len()], &payload[..]);
    }

    #[test]
    fn http_multipart_form_puts_fields_before_the_file() {
        let (content_type, body) =
            multipart_form(&[("a", "1"), ("b", "")], "file", "f.bin", b"payload");
        let boundary = content_type
            .strip_prefix("multipart/form-data; boundary=")
            .expect("boundary");
        let want = format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"a\"\r\n\r\n1\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"b\"\r\n\r\n\r\n--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"f.bin\"\r\nContent-Type: application/octet-stream\r\n\r\npayload\r\n--{boundary}--\r\n"
        );
        assert_eq!(String::from_utf8(body).expect("utf8"), want);
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
