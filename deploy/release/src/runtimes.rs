//! Native runtimes in the shipped inventory: records with packaged-byte hashes
//! and the NOTICE section they add.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

pub const SCHEMA: &str = "runtime-inventory/v1";

/// The line that starts the runtime section of a NOTICE.
pub const SECTION_MARKER: &str = "=== native runtimes ===";

/// One runtime as the rule wrote it into the shipped JSON.
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

/// One runtime record: the hashes come from the bytes the archive ships.
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

/// Hashes each runtime's files and reads its notice. Runtimes that cannot ship are errors.
pub fn build(runtimes: &[ShippedRuntime]) -> Result<Vec<RuntimeRecord>, String> {
    let mut records = Vec::with_capacity(runtimes.len());
    for runtime in runtimes {
        let name = &runtime.name;
        match runtime.redistribution.as_str() {
            "build_only" => {
                return Err(format!(
                    "inventory: runtime {name} is build_only and cannot ship"
                ));
            }
            "app_local" if runtime.notice.is_none() => {
                return Err(format!("inventory: app_local runtime {name} has no notice"));
            }
            "app_local" | "os_supplied" => {}
            other => {
                return Err(format!(
                    "inventory: runtime {name} has unsupported redistribution {other:?}"
                ));
            }
        }
        let mut files = Vec::with_capacity(runtime.files.len());
        for path in &runtime.files {
            let member = path
                .file_name()
                .and_then(|member| member.to_str())
                .ok_or_else(|| format!("inventory: runtime file {} has no name", path.display()))?;
            let sha256 = dx_digest::sha256_file_hex(path)
                .map_err(|error| format!("inventory: hash {}: {error}", path.display()))?;
            files.push(RuntimeFile {
                name: member.to_owned(),
                sha256,
            });
        }
        files.sort_by(|a, b| a.name.cmp(&b.name));
        let notice = match &runtime.notice {
            Some(path) => Some(
                std::fs::read_to_string(path)
                    .map_err(|error| format!("inventory: read {}: {error}", path.display()))?,
            ),
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

pub fn render_records(records: &[RuntimeRecord]) -> String {
    let doc = Doc {
        schema: SCHEMA.to_owned(),
        runtimes: records.to_vec(),
    };
    let mut text = serde_json::to_string_pretty(&doc).unwrap_or_default();
    text.push('\n');
    text
}

pub fn parse_records(text: &str) -> Result<Vec<RuntimeRecord>, String> {
    let doc: Doc = serde_json::from_str(text).map_err(|error| format!("runtimes: {error}"))?;
    if doc.schema != SCHEMA {
        return Err(format!(
            "runtimes: schema {:?}, want {SCHEMA:?}",
            doc.schema
        ));
    }
    Ok(doc.runtimes)
}

/// The NOTICE section for the records: nothing when empty, else the marker,
/// then each runtime by name. app_local runtimes carry their notice;
/// os_supplied ones get one line.
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
            _ => out.push_str(&format!(
                "{} {} ({})\n",
                record.name, record.version, record.redistribution
            )),
        }
    }
    out
}

/// The NOTICE text without its runtime section.
pub fn strip_section(notice: &str) -> &str {
    let mut offset = 0;
    for line in notice.split_inclusive('\n') {
        if line.trim_end_matches('\n').trim_end_matches('\r') == SECTION_MARKER {
            return &notice[..offset];
        }
        offset += line.len();
    }
    notice
}

/// Reads the sidecar file of runtime records.
pub fn read_records(path: &Path) -> std::io::Result<Vec<RuntimeRecord>> {
    parse_records(&std::fs::read_to_string(path)?).map_err(std::io::Error::other)
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
        let records = build(&[shipped.clone()]).expect("builds");
        let bytes = std::fs::read(&shipped.files[0]).expect("read");
        assert_eq!(records[0].files[0].name, "vc.so");
        assert_eq!(records[0].files[0].sha256, dx_digest::sha256_hex(&bytes));
        assert_eq!(parse_records(&render_records(&records)), Ok(records));
    }

    #[test]
    fn runtimes_that_cannot_ship_fail() {
        let dir = tempfile::tempdir().expect("dir");
        let build_only = runtime(dir.path(), "sdk", "build_only", false);
        assert_eq!(
            build(&[build_only]),
            Err("inventory: runtime sdk is build_only and cannot ship".to_owned())
        );
        let no_notice = runtime(dir.path(), "vc", "app_local", false);
        assert_eq!(
            build(&[no_notice]),
            Err("inventory: app_local runtime vc has no notice".to_owned())
        );
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
