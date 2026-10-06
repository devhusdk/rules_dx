//! One path as a `file://` URI, and the local path one names.

use std::path::{Path, PathBuf};

use url::Url;

const VERBATIM_PREFIX: &str = r"\\?\";

/// Why a path and a file URI could not be spelled for each other.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum UriError {
    #[error("path {path:?} is not absolute: a file URI names an absolute path")]
    NotAbsolute { path: String },
    #[error("path {path:?} has no file URI that reads back as the same path")]
    NotEncodable { path: String },
    #[error("{uri:?} is not a URI")]
    Malformed { uri: String },
    #[error("{uri:?} is not a file URI")]
    NotAFileUri { uri: String },
    #[error("file URI {uri:?} carries a query or fragment: it does not name one path")]
    NotOnePath { uri: String },
    #[error("file URI {uri:?} does not name a local path")]
    NotLocalPath { uri: String },
}

/// Returns one absolute path as a `file://` URI.
///
/// A relative path has no file URI; read one with `native` or `posix` instead.
/// A path a `file://` URI would not read back as the same path has none either.
pub fn uri(path: &Path) -> Result<String, UriError> {
    if !path.is_absolute() {
        return Err(UriError::NotAbsolute {
            path: path.to_string_lossy().into_owned(),
        });
    }
    let encoded: Option<String> = Url::from_file_path(path).map(Url::into).ok();
    let read_back = match path.strip_prefix(VERBATIM_PREFIX) {
        Ok(without) => without,
        Err(_) => path,
    };
    match encoded {
        Some(uri) if uri_to_path(&uri).ok().as_deref() == Some(read_back) => Ok(uri),
        _ => Err(UriError::NotEncodable {
            path: path.to_string_lossy().into_owned(),
        }),
    }
}

/// Returns the local path one `file://` URI names.
pub fn uri_to_path(uri: &str) -> Result<PathBuf, UriError> {
    let parsed = Url::parse(uri).map_err(|_| UriError::Malformed {
        uri: uri.to_owned(),
    })?;
    if parsed.scheme() != "file" {
        return Err(UriError::NotAFileUri {
            uri: uri.to_owned(),
        });
    }
    let rooted = uri
        .split_once(':')
        .is_some_and(|(_, rest)| rest.starts_with('/'));
    if !rooted || parsed.query().is_some() || parsed.fragment().is_some() {
        return Err(UriError::NotLocalPath {
            uri: uri.to_owned(),
        });
    }
    parsed.to_file_path().map_err(|()| UriError::NotLocalPath {
        uri: uri.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A file URI for one path this host spells absolutely.
    fn uri_of(text: &str) -> String {
        uri(Path::new(text)).expect("absolute")
    }

    /// A directory this host spells absolutely, with forward slashes for URIs.
    fn root() -> (&'static str, &'static str) {
        if cfg!(windows) {
            (r"C:\tmp", "/C:/tmp")
        } else {
            ("/tmp", "/tmp")
        }
    }

    /// Filenames a temp directory holds on this host, with the URI each one spells as.
    fn escapes() -> &'static [(&'static str, &'static str)] {
        if cfg!(windows) {
            &[
                ("a b.pb", "a%20b.pb"),
                ("a%20b.pb", "a%2520b.pb"),
                ("täst.pb", "t%C3%A4st.pb"),
            ]
        } else {
            &[
                ("a b.pb", "a%20b.pb"),
                ("a%20b.pb", "a%2520b.pb"),
                ("a#b.pb", "a%23b.pb"),
                ("a?b.pb", "a%3Fb.pb"),
                ("täst.pb", "t%C3%A4st.pb"),
            ]
        }
    }

    #[test]
    fn uri_escapes_each_character_a_reader_has_to_decode() {
        if cfg!(windows) {
            assert_eq!(uri_of(r"C:\tmp\a b.py"), "file:///C:/tmp/a%20b.py");
            assert_eq!(uri_of(r"\\?\C:\tmp\a b.py"), "file:///C:/tmp/a%20b.py");
            assert_eq!(
                uri_of(r"\\?\UNC\srv\share\a#b.py"),
                "file://srv/share/a%23b.py"
            );
            assert_eq!(uri_of(r"\\srv\share\a b.py"), "file://srv/share/a%20b.py");
        } else {
            for (path, want) in [
                ("/", "file:///"),
                ("/tmp/a.py", "file:///tmp/a.py"),
                ("/tmp/a b.py", "file:///tmp/a%20b.py"),
                ("/tmp/a%20b.py", "file:///tmp/a%2520b.py"),
                ("/tmp/a#b.py", "file:///tmp/a%23b.py"),
                ("/tmp/a?b.py", "file:///tmp/a%3Fb.py"),
                ("/tmp/täst.py", "file:///tmp/t%C3%A4st.py"),
                ("/tmp/日本.py", "file:///tmp/%E6%97%A5%E6%9C%AC.py"),
            ] {
                assert_eq!(uri_of(path), want, "path: {path:?}");
            }
        }
    }

    #[test]
    fn uri_keeps_every_component_it_was_given() {
        let (native, spelled) = root();
        let path = Path::new(native).join(".").join("a.py");
        let uri = uri(&path).expect("absolute");
        assert_eq!(uri, format!("file://{spelled}/a.py"));
        assert_eq!(
            uri_to_path(&uri).expect("local"),
            Path::new(native).join("a.py")
        );
    }

    #[test]
    fn uri_has_no_spelling_for_a_path_a_reader_resolves_differently() {
        let (native, _) = root();
        for text in [
            format!("{native}/../a.py"),
            format!("{native}/a/../../a.py"),
        ] {
            assert_eq!(
                uri(Path::new(&text)).expect_err(&text),
                UriError::NotEncodable { path: text.clone() },
                "path: {text:?}"
            );
        }
    }

    #[test]
    fn uri_has_no_spelling_for_a_relative_path() {
        for text in ["", "a.py", "./a.py", "../a.py", "sub/a.py", "C:a.py"] {
            assert_eq!(
                uri(Path::new(text)).expect_err(text),
                UriError::NotAbsolute {
                    path: text.to_owned()
                },
                "path: {text:?}"
            );
        }
    }

    #[test]
    fn uri_to_path_decodes_the_escapes_uri_writes() {
        if cfg!(windows) {
            for (uri, want) in [
                ("file:///C:/tmp/a.py", r"C:\tmp\a.py"),
                ("file:///C:/tmp/a%20b.py", r"C:\tmp\a b.py"),
                ("file:///C:/tmp/a%2520b.py", r"C:\tmp\a%20b.py"),
                ("file:///C%3A/tmp/a%20b.py", r"C:\tmp\a b.py"),
                ("file:///C:/tmp/t%C3%A4st.py", r"C:\tmp\täst.py"),
                ("file://localhost/C:/tmp/a.py", r"C:\tmp\a.py"),
                ("file://srv/share/a%23b.py", r"\\srv\share\a#b.py"),
            ] {
                assert_eq!(
                    uri_to_path(uri).expect(uri),
                    PathBuf::from(want),
                    "uri: {uri:?}"
                );
            }
        } else {
            for (uri, want) in [
                ("file:///", "/"),
                ("file:/tmp/a.py", "/tmp/a.py"),
                ("file:///tmp/a.py", "/tmp/a.py"),
                ("file:///tmp/a%20b.py", "/tmp/a b.py"),
                ("file:///tmp/a%2520b.py", "/tmp/a%20b.py"),
                ("file:///tmp/a%23b.py", "/tmp/a#b.py"),
                ("file:///tmp/a%3Fb.py", "/tmp/a?b.py"),
                ("file:///tmp/t%C3%A4st.py", "/tmp/täst.py"),
                ("file://localhost/tmp/a.py", "/tmp/a.py"),
                ("file:///C:/tmp/a.py", "/C:/tmp/a.py"),
            ] {
                assert_eq!(
                    uri_to_path(uri).expect(uri),
                    PathBuf::from(want),
                    "uri: {uri:?}"
                );
            }
        }
    }

    #[test]
    fn uri_to_path_rejects_uris_that_name_no_local_path() {
        for uri in ["", "/tmp/a.py", "a.py"] {
            assert_eq!(
                uri_to_path(uri).expect_err(uri),
                UriError::Malformed {
                    uri: uri.to_owned()
                },
                "uri: {uri:?}"
            );
        }
        for uri in ["bytestream://remote/cache/a.pb", "https://example.com/a.pb"] {
            assert_eq!(
                uri_to_path(uri).expect_err(uri),
                UriError::NotAFileUri {
                    uri: uri.to_owned()
                },
                "uri: {uri:?}"
            );
        }
        for uri in [
            "file:a.py",
            "file:tmp/a.py",
            "file:///tmp/a.py?x=1",
            "file:///tmp/a%23b.py#frag",
        ] {
            assert_eq!(
                uri_to_path(uri).expect_err(uri),
                UriError::NotLocalPath {
                    uri: uri.to_owned()
                },
                "uri: {uri:?}"
            );
        }
        if !cfg!(windows) {
            assert_eq!(
                uri_to_path("file://otherhost/out/a.pb").expect_err("remote host"),
                UriError::NotLocalPath {
                    uri: "file://otherhost/out/a.pb".to_owned()
                }
            );
        }
    }

    #[test]
    fn uri_messages_name_what_each_uri_lacks() {
        for (err, want) in [
            (
                UriError::NotAbsolute {
                    path: "a.py".to_owned(),
                },
                "path \"a.py\" is not absolute: a file URI names an absolute path",
            ),
            (
                UriError::NotEncodable {
                    path: "C:\\tmp\\..\\a.py".to_owned(),
                },
                "path \"C:\\\\tmp\\\\..\\\\a.py\" has no file URI that reads back as the same path",
            ),
            (
                UriError::Malformed {
                    uri: "x".to_owned(),
                },
                "\"x\" is not a URI",
            ),
            (
                UriError::NotAFileUri {
                    uri: "bytestream://x".to_owned(),
                },
                "\"bytestream://x\" is not a file URI",
            ),
            (
                UriError::NotLocalPath {
                    uri: "file://host/a".to_owned(),
                },
                "file URI \"file://host/a\" does not name a local path",
            ),
        ] {
            assert_eq!(err.to_string(), want);
        }
    }

    #[test]
    fn uri_round_trips_literal_filenames_through_the_filesystem() {
        let dir = tempfile::TempDir::new().expect("scratch");
        for (name, escaped) in escapes() {
            let path = dir.path().join(name);
            std::fs::write(&path, name.as_bytes()).expect("write");
            let uri = uri(&path).expect("absolute");
            assert!(uri.ends_with(&format!("/{escaped}")), "uri: {uri}");
            let round_tripped = uri_to_path(&uri).expect("local");
            assert_eq!(round_tripped, path, "name: {name:?}");
            assert_eq!(
                std::fs::read(&round_tripped).expect("read"),
                name.as_bytes(),
                "name: {name:?}"
            );
        }
    }
}
