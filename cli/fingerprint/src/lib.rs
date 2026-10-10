#![cfg_attr(
    not(test),
    deny(
        clippy::expect_used,
        clippy::unwrap_used,
        clippy::unreachable,
        clippy::todo
    )
)]

pub mod baseline;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum FingerprintError {
    #[error("fingerprint JSON serializes: {detail}")]
    Json { detail: String },
}

impl From<serde_json::Error> for FingerprintError {
    fn from(error: serde_json::Error) -> Self {
        Self::Json {
            detail: error.to_string(),
        }
    }
}

pub fn to_json<T: serde::Serialize>(view: &T) -> Result<String, FingerprintError> {
    serde_json::to_string(view).map_err(FingerprintError::from)
}

pub fn ensure_ascii(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        let code = c as u32;
        if code < 0x80 {
            out.push(c);
        } else if code <= 0xFFFF {
            out.push_str(&format!("\\u{code:04x}"));
        } else {
            let v = code - 0x10000;
            let high = 0xD800 + (v >> 10);
            let low = 0xDC00 + (v & 0x3FF);
            out.push_str(&format!("\\u{high:04x}\\u{low:04x}"));
        }
    }
    out
}

pub fn to_json_ascii_pretty(value: &serde_json::Value) -> Result<String, FingerprintError> {
    let pretty = serde_json::to_string_pretty(value).map_err(FingerprintError::from)?;
    Ok(ensure_ascii(&pretty) + "\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn renders_string_bool_vec_shapes() {
        #[derive(serde::Serialize)]
        struct View<'a> {
            name: &'a str,
            read_only: bool,
            tags: Vec<&'a str>,
        }
        let view = View {
            name: "a\"b",
            read_only: true,
            tags: vec!["x", "y"],
        };
        assert_eq!(
            to_json(&view).expect("fingerprint renders"),
            r#"{"name":"a\"b","read_only":true,"tags":["x","y"]}"#
        );
    }

    #[test]
    fn ascii_escape_matches_python_ensure_ascii() {
        assert_eq!(ensure_ascii("caf\u{e9}"), "caf\\u00e9");
        assert_eq!(ensure_ascii("a\u{1F600}b"), "a\\ud83d\\ude00b");
        assert_eq!(ensure_ascii("plain"), "plain");
        let value = serde_json::json!({"name": "caf\u{e9}"});
        let text = to_json_ascii_pretty(&value).expect("pretty ascii");
        assert!(text.contains("caf\\u00e9"));
        assert!(!text.contains("caf\u{e9}"));
        assert!(text.ends_with('\n'));
    }

    #[test]
    fn json_error_is_typed_with_display() {
        use std::collections::HashMap;
        let mut bad: HashMap<Vec<u8>, String> = HashMap::new();
        bad.insert(vec![0xff], "x".to_owned());
        let err = to_json(&bad).expect_err("non-string keys must fail");
        assert!(err.to_string().contains("fingerprint JSON serializes"));
    }
}
