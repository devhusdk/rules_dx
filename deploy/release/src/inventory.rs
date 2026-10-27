//! `release_tool inventory`: joins shipped crates with their lockfile licenses
//! and writes the NOTICE manifest plus one text per crate.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

pub const SCHEMA: &str = "shipped-inventory/v1";

/// The manifest set every shipped crate belongs to.
const SET: &str = "cargo";

/// The file name each crate's text is written under.
pub const TEXT_NAME: &str = "LICENSE";

/// One crate the aspect found in the runtime graph.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ShippedCrate {
    #[serde(rename = "crate")]
    pub name: String,
    pub version: String,
    pub label: String,
    #[serde(default)]
    pub texts: Vec<PathBuf>,
    #[serde(default)]
    pub manifest: Option<PathBuf>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ShippedDoc {
    schema: String,
    crates: Vec<ShippedCrate>,
}

/// Parses the aspect's shipped-inventory/v1 document, sorted by crate and version.
pub fn parse_shipped(text: &str) -> Result<Vec<ShippedCrate>, String> {
    let doc: ShippedDoc =
        serde_json::from_str(text).map_err(|error| format!("inventory: {error}"))?;
    if doc.schema != SCHEMA {
        return Err(format!(
            "inventory: schema {:?}, want {SCHEMA:?}",
            doc.schema
        ));
    }
    let mut crates = doc.crates;
    crates.sort_by(|a, b| (&a.name, &a.version).cmp(&(&b.name, &b.version)));
    for pair in crates.windows(2) {
        if (&pair[0].name, &pair[0].version) == (&pair[1].name, &pair[1].version) {
            return Err(format!(
                "inventory: {}@{} is listed twice ({} and {})",
                pair[0].name, pair[0].version, pair[0].label, pair[1].label
            ));
        }
    }
    Ok(crates)
}

/// The manifest text path of one crate: `cargo/<crate>-<version>/LICENSE`.
pub fn text_path(name: &str, version: &str) -> String {
    format!("{SET}/{name}-{version}/{TEXT_NAME}")
}

/// One crate joined with its license and the words the NOTICE carries.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JoinedCrate {
    pub name: String,
    pub version: String,
    pub license: String,
    pub words: String,
}

/// Renders the NOTICE words for one crate: every shipped text in name
/// order, each under a `--- <file> ---` line, or one line saying none ships.
fn crate_words(krate: &ShippedCrate, license: &str, texts: &[(String, String)]) -> String {
    if texts.is_empty() {
        return format!(
            "{} {} ships no license text; declared license: {license}\n",
            krate.name, krate.version
        );
    }
    let mut out = String::new();
    for (index, (name, words)) in texts.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        out.push_str(&format!("--- {name} ---\n"));
        out.push_str(words.trim_end_matches('\n'));
        out.push('\n');
    }
    out
}

/// Reads one crate's texts keyed by file name, sorted.
fn read_crate_texts(krate: &ShippedCrate) -> Result<Vec<(String, String)>, String> {
    let mut texts = BTreeMap::new();
    for path in &krate.texts {
        let name = path
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                format!(
                    "inventory: {}@{} text {} has no UTF-8 file name",
                    krate.name,
                    krate.version,
                    path.display()
                )
            })?
            .to_owned();
        let words = std::fs::read_to_string(path).map_err(|error| {
            format!(
                "inventory: {}@{} cannot read {}: {error}",
                krate.name,
                krate.version,
                path.display()
            )
        })?;
        if words.trim().is_empty() {
            return Err(format!(
                "inventory: {}@{} text {name} is empty",
                krate.name, krate.version
            ));
        }
        if texts.insert(name.clone(), words).is_some() {
            return Err(format!(
                "inventory: {}@{} lists text {name} twice",
                krate.name, krate.version
            ));
        }
    }
    Ok(texts.into_iter().collect())
}

/// Whether licenses.toml approves a crate shipping no words for `license`.
fn approved(policy: &dx_audit::license_policy::LicensePolicy, finding: &JoinedCrate) -> bool {
    let finding = dx_audit::license_policy::LicenseFinding {
        package: finding.name.clone(),
        set: SET.to_owned(),
        license: finding.license.clone(),
        version: finding.version.clone(),
    };
    policy
        .exceptions
        .iter()
        .any(|exception| dx_audit::license_policy::license_exception_covers(exception, &finding))
}

/// The license a crate outside the root crate universe declares in its own Cargo.toml.
fn manifest_license(krate: &ShippedCrate) -> Result<String, String> {
    let missing = || {
        format!(
            "inventory: {}@{} ({}) is not in cargo-bazel-lock.json and its Cargo.toml declares no license",
            krate.name, krate.version, krate.label
        )
    };
    let path = krate.manifest.as_ref().ok_or_else(missing)?;
    let text = std::fs::read_to_string(path).map_err(|error| {
        format!(
            "inventory: {}@{} cannot read {}: {error}",
            krate.name,
            krate.version,
            path.display()
        )
    })?;
    dx_audit::locks::cargo_manifest_license(&text).ok_or_else(missing)
}

/// Joins the shipped crates with the lockfile licenses and reads their texts.
///
/// Fails when a crate is missing from the lockfile, when the lockfile has no
/// license for it, or when its license needs reproduced words, it ships none
/// and licenses.toml has no exception for it.
pub fn join(
    crates: &[ShippedCrate],
    lock_text: &str,
    licenses_toml: &str,
) -> Result<Vec<JoinedCrate>, String> {
    let lock: serde_json::Value = serde_json::from_str(lock_text)
        .map_err(|error| format!("inventory: cargo-bazel-lock.json: {error}"))?;
    let locked = lock
        .get("crates")
        .and_then(serde_json::Value::as_object)
        .ok_or("inventory: cargo-bazel-lock.json has no crates table")?;
    let policy = dx_audit::license_policy::load_licenses_toml(licenses_toml)
        .map_err(|error| format!("inventory: {error}"))?;
    let packages: Vec<dx_audit::vuln::LockedPackage> = crates
        .iter()
        .map(|krate| dx_audit::vuln::LockedPackage {
            name: krate.name.clone(),
            version: krate.version.clone(),
            set: SET.to_owned(),
            is_git: false,
            is_private: false,
            location: None,
            platform: None,
        })
        .collect();
    let licensed = dx_audit::locks::cargo_licenses(lock_text, &packages);
    let mut joined = Vec::with_capacity(crates.len());
    for (krate, licensed) in crates.iter().zip(licensed) {
        let license = if locked.contains_key(&format!("{} {}", krate.name, krate.version)) {
            licensed.license
        } else {
            manifest_license(krate)?
        };
        if license == "UNKNOWN" {
            return Err(format!(
                "inventory: {}@{} has no license in cargo-bazel-lock.json",
                krate.name, krate.version
            ));
        }
        let texts = read_crate_texts(krate)?;
        let entry = JoinedCrate {
            name: krate.name.clone(),
            version: krate.version.clone(),
            words: crate_words(krate, &license, &texts),
            license,
        };
        let input = dx_audit::license_notice::NoticeInput {
            package: entry.name.clone(),
            license: entry.license.clone(),
            text_present: !texts.is_empty(),
        };
        let tier = dx_audit::license_expr::Tier::Distributed;
        let outcome = dx_audit::license_notice::evaluate_notice(&input, tier, &|name: &str| {
            name == entry.license && approved(&policy, &entry)
        });
        if dx_audit::license_notice::notice_fails(outcome, tier) {
            return Err(format!(
                "inventory: missing-notice-text: {}@{} ({}) ships no LICENSE*/COPYING*/NOTICE* file; add a licenses.toml exception or a text",
                entry.name, entry.version, entry.license
            ));
        }
        joined.push(entry);
    }
    Ok(joined)
}

/// Renders the six-field NOTICE manifest for the joined crates.
pub fn render_manifest(joined: &[JoinedCrate]) -> String {
    let mut out = String::from(
        "# Generated by release_tool inventory from the shipped crate graph; do not edit.\n",
    );
    for entry in joined {
        out.push_str(&format!(
            "{}|{SET}|{}|{}|{}|{}\n",
            entry.name,
            entry.version,
            entry.license,
            text_path(&entry.name, &entry.version),
            crate::notice_text_sha256(&entry.words)
        ));
    }
    out
}

/// Runs the join and writes the manifest plus `<texts>/cargo/<crate>-<version>/LICENSE`.
pub fn write_inventory(
    shipped: &Path,
    lock: &Path,
    licenses: &Path,
    manifest: &Path,
    texts: &Path,
) -> std::io::Result<()> {
    let crates =
        parse_shipped(&std::fs::read_to_string(shipped)?).map_err(std::io::Error::other)?;
    let joined = join(
        &crates,
        &std::fs::read_to_string(lock)?,
        &std::fs::read_to_string(licenses)?,
    )
    .map_err(std::io::Error::other)?;
    for entry in &joined {
        let path = texts.join(text_path(&entry.name, &entry.version));
        std::fs::create_dir_all(path.parent().unwrap_or(texts))?;
        std::fs::write(&path, entry.words.as_bytes())?;
    }
    std::fs::write(manifest, render_manifest(&joined).as_bytes())
}

#[cfg(test)]
mod tests {
    use super::*;

    const LOCK: &str = r#"{"crates": {
        "alpha 1.0.0": {"license": "MIT OR Apache-2.0"},
        "beta 2.0.0": {"license": "MIT"},
        "gamma 0.1.0": {"license": "Unicode-3.0"},
        "nolicense 0.1.0": {"license": ""}
    }}"#;

    fn shipped(name: &str, version: &str, texts: Vec<PathBuf>) -> ShippedCrate {
        ShippedCrate {
            name: name.to_owned(),
            version: version.to_owned(),
            label: format!("@crates//:{name}"),
            texts,
            manifest: None,
        }
    }

    fn text(dir: &Path, rel: &str, words: &str) -> PathBuf {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("dir");
        std::fs::write(&path, words).expect("text");
        path
    }

    #[test]
    fn shipped_doc_parses_sorted_and_rejects_drift() {
        let doc = r#"{"schema":"shipped-inventory/v1","crates":[
            {"crate":"beta","version":"2.0.0","label":"@b//:beta","texts":["b/LICENSE"]},
            {"crate":"alpha","version":"1.0.0","label":"@a//:alpha"}]}"#;
        let crates = parse_shipped(doc).expect("parses");
        assert_eq!(crates[0].name, "alpha");
        assert!(crates[0].texts.is_empty());
        assert_eq!(crates[1].texts, vec![PathBuf::from("b/LICENSE")]);

        let wrong = r#"{"schema":"shipped-inventory/v0","crates":[]}"#;
        assert_eq!(
            parse_shipped(wrong),
            Err(
                "inventory: schema \"shipped-inventory/v0\", want \"shipped-inventory/v1\""
                    .to_owned()
            )
        );
        assert!(
            parse_shipped("{\"schema\":\"shipped-inventory/v1\",\"crates\":[],\"x\":1}")
                .expect_err("unknown field")
                .starts_with("inventory: unknown field")
        );
        let twice = r#"{"schema":"shipped-inventory/v1","crates":[
            {"crate":"a","version":"1","label":"@x//:a"},
            {"crate":"a","version":"1","label":"@y//:a"}]}"#;
        assert_eq!(
            parse_shipped(twice),
            Err("inventory: a@1 is listed twice (@x//:a and @y//:a)".to_owned())
        );
    }

    #[test]
    fn join_reads_licenses_and_renders_exact_manifest() {
        let dir = tempfile::tempdir().expect("tempdir");
        let crates = vec![
            shipped(
                "alpha",
                "1.0.0",
                vec![
                    text(dir.path(), "a/LICENSE-MIT", "MIT words\n\n"),
                    text(dir.path(), "a/LICENSE-APACHE", "Apache words"),
                ],
            ),
            shipped("gamma", "0.1.0", vec![]),
        ];
        let joined = join(&crates, LOCK, "").expect("joins");
        assert_eq!(
            joined[0].words,
            "--- LICENSE-APACHE ---\nApache words\n\n--- LICENSE-MIT ---\nMIT words\n"
        );
        assert_eq!(
            joined[1].words,
            "gamma 0.1.0 ships no license text; declared license: Unicode-3.0\n"
        );
        let manifest = render_manifest(&joined);
        let want = format!(
            "# Generated by release_tool inventory from the shipped crate graph; do not edit.\n\
             alpha|cargo|1.0.0|MIT OR Apache-2.0|cargo/alpha-1.0.0/LICENSE|{}\n\
             gamma|cargo|0.1.0|Unicode-3.0|cargo/gamma-0.1.0/LICENSE|{}\n",
            crate::notice_text_sha256(&joined[0].words),
            crate::notice_text_sha256(&joined[1].words)
        );
        assert_eq!(manifest, want);
        assert_eq!(
            crate::parse_notice_manifest(&manifest)
                .expect("notice manifest")
                .len(),
            2
        );
    }

    #[test]
    fn same_basename_in_two_crates_stays_per_crate() {
        let dir = tempfile::tempdir().expect("tempdir");
        let crates = vec![
            shipped(
                "alpha",
                "1.0.0",
                vec![text(dir.path(), "a/LICENSE", "alpha words")],
            ),
            shipped(
                "beta",
                "2.0.0",
                vec![text(dir.path(), "b/LICENSE", "beta words")],
            ),
        ];
        let shipped_json = dir.path().join("shipped.json");
        let doc = serde_json::json!({
            "schema": SCHEMA,
            "crates": crates.iter().map(|krate| serde_json::json!({
                "crate": krate.name,
                "version": krate.version,
                "label": krate.label,
                "texts": krate.texts,
            })).collect::<Vec<_>>(),
        });
        std::fs::write(&shipped_json, doc.to_string()).expect("shipped");
        let lock = text(dir.path(), "lock.json", LOCK);
        let licenses = text(dir.path(), "licenses.toml", "\n");
        let manifest = dir.path().join("manifest.txt");
        let texts = dir.path().join("out");
        write_inventory(&shipped_json, &lock, &licenses, &manifest, &texts).expect("writes");
        let text_files = vec![
            texts.join("cargo/alpha-1.0.0/LICENSE"),
            texts.join("cargo/beta-2.0.0/LICENSE"),
        ];
        assert_eq!(
            std::fs::read_to_string(&text_files[0]).expect("alpha"),
            "--- LICENSE ---\nalpha words\n"
        );
        let notice = dir.path().join("NOTICE");
        crate::write_notice(&manifest, &notice, "//demo:root", &text_files).expect("notice");
        let words = std::fs::read_to_string(&notice).expect("notice text");
        assert!(words.contains(
            "=== alpha 1.0.0 (MIT OR Apache-2.0) [cargo] ===\n--- LICENSE ---\nalpha words\n"
        ));
        assert!(words.contains("=== beta 2.0.0 (MIT) [cargo] ===\n--- LICENSE ---\nbeta words\n"));
        crate::notice_verify_files(&notice, &manifest, &text_files).expect("verifies");
    }

    #[test]
    fn join_fails_closed() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cases = [
            (
                shipped("missing", "9.9.9", vec![]),
                "inventory: missing@9.9.9 (@crates//:missing) is not in cargo-bazel-lock.json and its Cargo.toml declares no license",
            ),
            (
                shipped("nolicense", "0.1.0", vec![]),
                "inventory: nolicense@0.1.0 has no license in cargo-bazel-lock.json",
            ),
            (
                shipped("beta", "2.0.0", vec![]),
                "inventory: missing-notice-text: beta@2.0.0 (MIT) ships no LICENSE*/COPYING*/NOTICE* file; add a licenses.toml exception or a text",
            ),
            (
                shipped("beta", "2.0.0", vec![text(dir.path(), "e/LICENSE", " \n")]),
                "inventory: beta@2.0.0 text LICENSE is empty",
            ),
            (
                shipped("beta", "2.0.0", vec![dir.path().join("absent/LICENSE")]),
                "inventory: beta@2.0.0 cannot read",
            ),
            (
                shipped("beta", "2.0.0", vec![PathBuf::from("/")]),
                "inventory: beta@2.0.0 text / has no UTF-8 file name",
            ),
            (
                shipped(
                    "beta",
                    "2.0.0",
                    vec![
                        text(dir.path(), "x/LICENSE", "x"),
                        text(dir.path(), "y/LICENSE", "y"),
                    ],
                ),
                "inventory: beta@2.0.0 lists text LICENSE twice",
            ),
        ];
        for (krate, want) in cases {
            let error = join(&[krate], LOCK, "").expect_err(want);
            assert!(error.starts_with(want), "{error}");
        }
        assert_eq!(
            join(&[], "not json", ""),
            Err("inventory: cargo-bazel-lock.json: expected ident at line 1 column 2".to_owned())
        );
        assert_eq!(
            join(&[], "{}", ""),
            Err("inventory: cargo-bazel-lock.json has no crates table".to_owned())
        );
        assert!(join(&[], LOCK, "[policy\n")
            .expect_err("bad toml")
            .starts_with("inventory: invalid licenses.toml"));
    }

    #[test]
    fn crates_outside_the_lockfile_read_their_cargo_toml() {
        let dir = tempfile::tempdir().expect("tempdir");
        let mut krate = shipped(
            "bytes",
            "1.7.1",
            vec![text(dir.path(), "b/LICENSE", "bytes words")],
        );
        krate.manifest = Some(text(
            dir.path(),
            "b/Cargo.toml",
            "[package]\nname = \"bytes\"\nlicense = \"MIT\"\n",
        ));
        let joined = join(&[krate.clone()], LOCK, "").expect("joins");
        assert_eq!(joined[0].license, "MIT");

        krate.manifest = Some(text(dir.path(), "c/Cargo.toml", "[package]\n"));
        assert_eq!(
            join(&[krate.clone()], LOCK, ""),
            Err("inventory: bytes@1.7.1 (@crates//:bytes) is not in cargo-bazel-lock.json and its Cargo.toml declares no license".to_owned())
        );
        krate.manifest = Some(dir.path().join("absent/Cargo.toml"));
        assert!(join(&[krate], LOCK, "")
            .expect_err("unreadable")
            .starts_with("inventory: bytes@1.7.1 cannot read"));
    }

    #[test]
    fn licenses_toml_exception_approves_missing_words() {
        let policy = r#"
[[exception]]
package = "beta"
set = "cargo"
license = "MIT"
versions = "=2.0.0"
reason = "upstream ships no LICENSE file"
expires = "2099-01-01"
"#;
        let joined = join(&[shipped("beta", "2.0.0", vec![])], LOCK, policy).expect("approved");
        assert_eq!(
            joined[0].words,
            "beta 2.0.0 ships no license text; declared license: MIT\n"
        );
        let other = policy.replace("\"beta\"", "\"alpha\"");
        assert!(join(&[shipped("beta", "2.0.0", vec![])], LOCK, &other).is_err());
    }

    #[test]
    fn write_inventory_reports_io_and_join_errors() {
        let dir = tempfile::tempdir().expect("tempdir");
        let missing = dir.path().join("missing.json");
        let out = dir.path().join("m.txt");
        assert!(write_inventory(&missing, &missing, &missing, &out, dir.path()).is_err());
        let bad = text(dir.path(), "bad.json", "{}");
        assert!(write_inventory(&bad, &bad, &bad, &out, dir.path()).is_err());
        let shipped_json = text(
            dir.path(),
            "s.json",
            r#"{"schema":"shipped-inventory/v1","crates":[{"crate":"zeta","version":"1","label":"@z//:zeta"}]}"#,
        );
        let lock = text(dir.path(), "lock.json", LOCK);
        assert!(write_inventory(&shipped_json, &lock, &lock, &out, dir.path()).is_err());
    }
}
