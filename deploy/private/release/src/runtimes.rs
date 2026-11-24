use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use dx_atomic_fs::path_io::{self, PathIoError};
use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "runtime-inventory/v1";

pub const SECTION_MARKER: &str = "=== native runtimes ===";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShippedRuntime {
    pub name: String,
    pub version: String,
    pub redistribution: String,
    pub source: String,
    #[serde(default)]
    pub files: Vec<PathBuf>,
    #[serde(default)]
    pub notice: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeFile {
    pub name: String,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeRecord {
    pub name: String,
    pub version: String,
    pub redistribution: String,
    pub source: String,
    pub files: Vec<RuntimeFile>,
    pub notice: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct Doc {
    schema: String,
    runtimes: Vec<RuntimeRecord>,
}

#[derive(Debug)]
pub enum RuntimesError {
    BuildOnly {
        name: String,
    },
    NoNotice {
        name: String,
    },
    Redistribution {
        name: String,
        redistribution: String,
    },
    FileName {
        path: PathBuf,
    },
    Hash {
        path: PathBuf,
        source: io::Error,
    },
    Notice(PathIoError),
    Json(serde_json::Error),
    Schema {
        found: String,
    },
    Io(PathIoError),
}

impl fmt::Display for RuntimesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BuildOnly { name } => {
                write!(f, "inventory: runtime {name} is build_only and cannot ship")
            }
            Self::NoNotice { name } => {
                write!(f, "inventory: app_local runtime {name} has no notice")
            }
            Self::Redistribution {
                name,
                redistribution,
            } => write!(
                f,
                "inventory: runtime {name} has unsupported redistribution {redistribution:?}"
            ),
            Self::FileName { path } => {
                write!(f, "inventory: runtime file {} has no name", path.display())
            }
            Self::Hash { path, source } => {
                write!(f, "inventory: hash {}: {source}", path.display())
            }
            Self::Notice(source) => write!(f, "inventory: {source}"),
            Self::Json(source) => write!(f, "runtimes: {source}"),
            Self::Schema { found } => write!(f, "runtimes: schema {found:?}, want {SCHEMA:?}"),
            Self::Io(source) => write!(f, "{source}"),
        }
    }
}

impl std::error::Error for RuntimesError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Hash { source, .. } => Some(source),
            Self::Notice(source) | Self::Io(source) => Some(source),
            Self::Json(source) => Some(source),
            Self::BuildOnly { .. }
            | Self::NoNotice { .. }
            | Self::Redistribution { .. }
            | Self::FileName { .. }
            | Self::Schema { .. } => None,
        }
    }
}

pub fn build(runtimes: &[ShippedRuntime]) -> Result<Vec<RuntimeRecord>, RuntimesError> {
    let mut records = Vec::with_capacity(runtimes.len());
    for runtime in runtimes {
        let name = &runtime.name;
        match runtime.redistribution.as_str() {
            "build_only" => {
                return Err(RuntimesError::BuildOnly { name: name.clone() });
            }
            "app_local" if runtime.notice.is_none() => {
                return Err(RuntimesError::NoNotice { name: name.clone() });
            }
            "app_local" | "os_supplied" => {}
            other => {
                return Err(RuntimesError::Redistribution {
                    name: name.clone(),
                    redistribution: other.to_owned(),
                });
            }
        }
        let mut files = Vec::with_capacity(runtime.files.len());
        for path in &runtime.files {
            let member = path
                .file_name()
                .and_then(|member| member.to_str())
                .ok_or_else(|| RuntimesError::FileName { path: path.clone() })?;
            let sha256 =
                dx_digest::sha256_file_hex(path).map_err(|source| RuntimesError::Hash {
                    path: path.clone(),
                    source,
                })?;
            files.push(RuntimeFile {
                name: member.to_owned(),
                sha256,
            });
        }
        files.sort_by(|a, b| a.name.cmp(&b.name));
        let notice = match &runtime.notice {
            Some(path) => Some(path_io::read_to_string(path).map_err(RuntimesError::Notice)?),
            None => None,
        };
        records.push(RuntimeRecord {
            name: runtime.name.clone(),
            version: runtime.version.clone(),
            redistribution: runtime.redistribution.clone(),
            source: runtime.source.clone(),
            files,
            notice,
        });
    }
    records.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(records)
}

#[must_use]
pub fn render_records(records: &[RuntimeRecord]) -> String {
    let doc = Doc {
        schema: SCHEMA.to_owned(),
        runtimes: records.to_vec(),
    };
    let mut text = serde_json::to_string_pretty(&doc).unwrap_or_default();
    text.push('\n');
    text
}

pub fn parse_records(text: &str) -> Result<Vec<RuntimeRecord>, RuntimesError> {
    let doc: Doc = serde_json::from_str(text).map_err(RuntimesError::Json)?;
    if doc.schema != SCHEMA {
        return Err(RuntimesError::Schema { found: doc.schema });
    }
    Ok(doc.runtimes)
}

#[must_use]
pub fn render_section(records: &[RuntimeRecord]) -> String {
    if records.is_empty() {
        return String::new();
    }
    let mut sorted: Vec<&RuntimeRecord> = records.iter().collect();
    sorted.sort_by(|a, b| a.name.cmp(&b.name));
    let mut out = format!("{SECTION_MARKER}\n");
    for record in sorted {
        match (&record.notice, record.redistribution.as_str()) {
            (Some(notice), "app_local") => {
                out.push_str(&format!(
                    "=== {} {} (app_local) [runtime] ===\n{}\n",
                    record.name,
                    record.version,
                    notice.trim_end_matches('\n')
                ));
            }
            _ => {
                out.push_str(&format!(
                    "{} {} ({})\n",
                    record.name, record.version, record.redistribution
                ));
            }
        }
    }
    out
}

#[must_use]
pub fn strip_section(notice: &str) -> &str {
    let mut offset = 0;
    for line in notice.split_inclusive('\n') {
        if line.trim_end_matches('\n').trim_end_matches('\r') == SECTION_MARKER {
            return notice.get(..offset).unwrap_or(notice);
        }
        offset += line.len();
    }
    notice
}

pub fn read_records(path: &Path) -> Result<Vec<RuntimeRecord>, RuntimesError> {
    parse_records(&path_io::read_to_string(path).map_err(RuntimesError::Io)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn runtime(dir: &Path, name: &str, redistribution: &str, with_notice: bool) -> ShippedRuntime {
        let lib = dir.join(format!("{name}.so"));
        std::fs::write(&lib, format!("bytes of {name}")).expect("lib");
        let notice = dir.join(format!("{name}.NOTICE"));
        std::fs::write(&notice, "Runtime words.\n\n").expect("notice");
        ShippedRuntime {
            name: name.to_owned(),
            version: "1.2".to_owned(),
            redistribution: redistribution.to_owned(),
            source: "toolchain".to_owned(),
            files: if redistribution == "app_local" {
                vec![lib]
            } else {
                Vec::new()
            },
            notice: with_notice.then_some(notice),
        }
    }

    #[test]
    fn hashes_are_the_hashes_of_the_shipped_bytes() {
        let dir = tempfile::tempdir().expect("dir");
        let shipped = runtime(dir.path(), "vc", "app_local", true);
        let records = build(std::slice::from_ref(&shipped)).expect("builds");
        let bytes = std::fs::read(&shipped.files[0]).expect("read");
        assert_eq!(records[0].files[0].name, "vc.so");
        assert_eq!(records[0].files[0].sha256, dx_digest::sha256_hex(&bytes));
        assert_eq!(
            parse_records(&render_records(&records)).expect("parses"),
            records
        );
    }

    #[test]
    fn runtimes_that_cannot_ship_fail() {
        let dir = tempfile::tempdir().expect("dir");
        let build_only = runtime(dir.path(), "sdk", "build_only", false);
        let error = build(&[build_only]).expect_err("build_only");
        assert!(
            matches!(error, RuntimesError::BuildOnly { .. }),
            "{error:?}"
        );
        assert_eq!(
            error.to_string(),
            "inventory: runtime sdk is build_only and cannot ship"
        );
        let no_notice = runtime(dir.path(), "vc", "app_local", false);
        let error = build(&[no_notice]).expect_err("no notice");
        assert!(matches!(error, RuntimesError::NoNotice { .. }), "{error:?}");
        assert_eq!(
            error.to_string(),
            "inventory: app_local runtime vc has no notice"
        );
        let mut missing_notice = runtime(dir.path(), "crt", "app_local", true);
        missing_notice.notice = Some(dir.path().join("absent.NOTICE"));
        let error = build(&[missing_notice]).expect_err("missing notice");
        assert!(matches!(error, RuntimesError::Notice(_)), "{error:?}");
        assert!(
            error.to_string().starts_with("inventory: cannot read "),
            "{error}"
        );
    }

    #[test]
    fn bad_record_documents_keep_their_source() {
        use std::error::Error as _;
        let error = parse_records("not json").expect_err("json");
        assert!(matches!(error, RuntimesError::Json(_)), "{error:?}");
        assert!(error.to_string().starts_with("runtimes: "), "{error}");
        assert!(error.source().is_some());
        let error = parse_records(r#"{"schema":"runtime-inventory/v0","runtimes":[]}"#)
            .expect_err("schema");
        assert!(matches!(error, RuntimesError::Schema { .. }), "{error:?}");
        assert_eq!(
            error.to_string(),
            "runtimes: schema \"runtime-inventory/v0\", want \"runtime-inventory/v1\""
        );
        let dir = tempfile::tempdir().expect("dir");
        let error = read_records(&dir.path().join("absent.json")).expect_err("missing");
        assert!(matches!(error, RuntimesError::Io(_)), "{error:?}");
        assert!(error.to_string().starts_with("cannot read "), "{error}");
    }

    #[test]
    fn section_lists_runtimes_by_name_and_strips_cleanly() {
        let dir = tempfile::tempdir().expect("dir");
        let records = build(&[
            runtime(dir.path(), "zeta", "app_local", true),
            runtime(dir.path(), "alpha", "os_supplied", false),
        ])
        .expect("builds");
        let section = render_section(&records);
        assert_eq!(
            section,
            format!(
                "{SECTION_MARKER}\nalpha 1.2 (os_supplied)\n=== zeta 1.2 (app_local) [runtime] ===\nRuntime words.\n"
            )
        );
        let head = "NOTICE for //a:b\n\n";
        assert_eq!(strip_section(&format!("{head}{section}")), head);
        assert_eq!(strip_section(head), head);
        assert_eq!(render_section(&[]), "");
    }
}
