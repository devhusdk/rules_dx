use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const LEDGER_SCHEMA: &str = "release-ledger/v1";

/// Outcome of one release operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum OpState {
    Validated,
    Executed,
    Failed,
    Partial,
    NotAuthorized,
}

/// One ledger line: what was attempted, on what, and how it ended.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct OpRecord {
    pub operation: String,
    pub subject: String,
    pub state: OpState,
    pub detail: String,
}

impl OpRecord {
    pub fn new(operation: &str, subject: &str, state: OpState, detail: &str) -> Self {
        Self {
            operation: operation.to_owned(),
            subject: subject.to_owned(),
            state,
            detail: detail.to_owned(),
        }
    }
}

/// Renders the records as sorted JSON under the `release-ledger/v1` schema.
pub fn render_ledger(records: &[OpRecord]) -> String {
    let mut sorted = records.to_vec();
    sorted.sort_by(|a, b| (&a.operation, &a.subject).cmp(&(&b.operation, &b.subject)));
    let value = serde_json::json!({ "records": sorted, "schema": LEDGER_SCHEMA });
    let mut text = serde_json::to_string_pretty(&value).unwrap_or_default();
    text.push('\n');
    text
}

/// Exit code for a ledger: 1 when any record failed or is partial.
pub fn ledger_exit(records: &[OpRecord]) -> i32 {
    let bad = records
        .iter()
        .any(|record| matches!(record.state, OpState::Failed | OpState::Partial));
    i32::from(bad)
}

/// A release as read from GitHub.
#[derive(Debug, PartialEq, Eq)]
pub struct RemoteRelease {
    pub draft: bool,
    pub assets: BTreeMap<String, Option<String>>,
}

#[derive(Deserialize)]
struct ReleaseJson {
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    assets: Vec<AssetJson>,
}

#[derive(Deserialize)]
struct AssetJson {
    name: String,
    digest: Option<String>,
}

/// Parses `gh api repos/.../releases/tags/<tag>` output; digests are `sha256:<hex>`.
pub fn parse_release_assets(json: &str) -> Result<RemoteRelease, String> {
    let release: ReleaseJson = serde_json::from_str(json)
        .map_err(|err| format!("release: cannot parse release JSON: {err}"))?;
    let assets = release
        .assets
        .into_iter()
        .map(|asset| {
            let digest = asset
                .digest
                .as_deref()
                .and_then(|text| text.strip_prefix("sha256:"))
                .map(str::to_owned);
            (asset.name, digest)
        })
        .collect();
    Ok(RemoteRelease {
        draft: release.draft,
        assets,
    })
}

/// Compares staged `(name, sha256)` assets with the release; returns records and names still to upload.
pub fn reconcile_assets(
    staged: &[(String, String)],
    remote: &RemoteRelease,
    approved: bool,
) -> (Vec<OpRecord>, Vec<String>) {
    let mut records = Vec::new();
    let mut pending = Vec::new();
    for (name, sha) in staged {
        match remote.assets.get(name) {
            None if !approved => records.push(OpRecord::new(
                "asset-upload",
                name,
                OpState::NotAuthorized,
                "upload needs owner approval",
            )),
            None if !remote.draft => records.push(OpRecord::new(
                "asset-upload",
                name,
                OpState::Partial,
                "release is published; asset is missing and will not be added",
            )),
            None => pending.push(name.clone()),
            Some(Some(digest)) if digest == sha => records.push(OpRecord::new(
                "asset-upload",
                name,
                OpState::Validated,
                "remote digest matches",
            )),
            Some(Some(digest)) => records.push(OpRecord::new(
                "asset-upload",
                name,
                OpState::Failed,
                &format!("mismatched existing asset: remote sha256 {digest}, staged {sha}"),
            )),
            Some(None) => records.push(OpRecord::new(
                "asset-upload",
                name,
                OpState::Failed,
                "mismatched existing asset: remote has no sha256 digest",
            )),
        }
    }
    for name in remote.assets.keys() {
        if !staged.iter().any(|(staged_name, _)| staged_name == name) {
            records.push(OpRecord::new(
                "asset-upload",
                name,
                OpState::Failed,
                "unexpected extra asset on the release",
            ));
        }
    }
    (records, pending)
}

/// Live GitHub calls, faked in tests.
pub trait ReleaseBackend {
    fn view(&self, tag: &str) -> Result<String, String>;
    fn upload(&self, tag: &str, path: &str) -> Result<String, String>;
}

/// Uploads what reconcile asks for, re-reads the release, and reconciles again.
pub fn sync_assets(
    backend: &dyn ReleaseBackend,
    tag: &str,
    staged: &[(String, String)],
    paths: &BTreeMap<String, String>,
    approved: bool,
) -> Vec<OpRecord> {
    let first = backend
        .view(tag)
        .and_then(|json| parse_release_assets(&json));
    let remote = match first {
        Ok(remote) => remote,
        Err(err) => return vec![OpRecord::new("release-view", tag, OpState::Failed, &err)],
    };
    let (records, pending) = reconcile_assets(staged, &remote, approved);
    if pending.is_empty() {
        return records;
    }
    let mut failures = Vec::new();
    for name in &pending {
        let path = paths.get(name).map(String::as_str).unwrap_or(name);
        if let Err(err) = backend.upload(tag, path) {
            failures.push((name.clone(), err));
            break;
        }
    }
    let after = backend
        .view(tag)
        .and_then(|json| parse_release_assets(&json));
    let remote = match after {
        Ok(remote) => remote,
        Err(err) => return vec![OpRecord::new("release-view", tag, OpState::Failed, &err)],
    };
    let (mut records, _) = reconcile_assets(staged, &remote, approved);
    let uploaded_any = pending.iter().any(|name| remote.assets.contains_key(name));
    for (name, err) in failures {
        let state = if uploaded_any {
            OpState::Partial
        } else {
            OpState::Failed
        };
        records.retain(|record| record.subject != name);
        records.push(OpRecord::new(
            "asset-upload",
            &name,
            state,
            &format!("upload failed: {err}"),
        ));
    }
    records.sort_by(|a, b| a.subject.cmp(&b.subject));
    records
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::RefCell;

    const A: &str = "aa";
    const B: &str = "bb";

    fn staged() -> Vec<(String, String)> {
        vec![("a.bin".into(), A.into()), ("b.bin".into(), B.into())]
    }

    fn paths() -> BTreeMap<String, String> {
        staged()
            .into_iter()
            .map(|(name, _)| (name.clone(), format!("/out/{name}")))
            .collect()
    }

    struct Fake {
        draft: bool,
        assets: RefCell<BTreeMap<String, Option<String>>>,
        uploads: RefCell<Vec<String>>,
        fail_on: Option<&'static str>,
    }

    impl Fake {
        fn new(draft: bool, assets: &[(&str, Option<&str>)]) -> Self {
            Self {
                draft,
                assets: RefCell::new(
                    assets
                        .iter()
                        .map(|(n, d)| (n.to_string(), d.map(str::to_owned)))
                        .collect(),
                ),
                uploads: RefCell::new(Vec::new()),
                fail_on: None,
            }
        }
    }

    impl ReleaseBackend for Fake {
        fn view(&self, _tag: &str) -> Result<String, String> {
            let assets: Vec<_> = self
                .assets
                .borrow()
                .iter()
                .map(|(n, d)| {
                    serde_json::json!({"name": n, "digest": d.as_ref().map(|d| format!("sha256:{d}"))})
                })
                .collect();
            Ok(serde_json::json!({"draft": self.draft, "assets": assets}).to_string())
        }

        fn upload(&self, _tag: &str, path: &str) -> Result<String, String> {
            if self.fail_on.is_some_and(|bad| path.ends_with(bad)) {
                return Err("boom".to_owned());
            }
            let name = path.rsplit('/').next().unwrap_or(path).to_owned();
            let sha = if name == "a.bin" { A } else { B };
            self.uploads.borrow_mut().push(name.clone());
            self.assets.borrow_mut().insert(name, Some(sha.to_owned()));
            Ok(String::new())
        }
    }

    fn run(fake: &Fake, approved: bool) -> Vec<OpRecord> {
        sync_assets(fake, "v1.2.3", &staged(), &paths(), approved)
    }

    fn states(records: &[OpRecord]) -> Vec<OpState> {
        records.iter().map(|r| r.state).collect()
    }

    #[test]
    fn absent_assets_are_uploaded_then_validated() {
        let fake = Fake::new(true, &[]);
        let records = run(&fake, true);
        assert_eq!(fake.uploads.borrow().len(), 2);
        assert_eq!(states(&records), vec![OpState::Validated; 2]);
        assert_eq!(ledger_exit(&records), 0);
    }

    #[test]
    fn repeat_run_uploads_nothing() {
        let fake = Fake::new(true, &[("a.bin", Some(A)), ("b.bin", Some(B))]);
        let records = run(&fake, true);
        assert!(fake.uploads.borrow().is_empty());
        assert_eq!(states(&records), vec![OpState::Validated; 2]);
    }

    #[test]
    fn mismatched_missing_digest_and_extra_assets_fail() {
        let fake = Fake::new(
            true,
            &[
                ("a.bin", Some("zz")),
                ("b.bin", None),
                ("c.bin", Some("cc")),
            ],
        );
        let records = run(&fake, true);
        assert_eq!(states(&records), vec![OpState::Failed; 3]);
        assert!(fake.uploads.borrow().is_empty());
        assert_eq!(ledger_exit(&records), 1);
    }

    #[test]
    fn published_release_is_never_uploaded_to() {
        let fake = Fake::new(false, &[("a.bin", Some(A))]);
        let records = run(&fake, true);
        assert!(fake.uploads.borrow().is_empty());
        assert_eq!(states(&records), vec![OpState::Validated, OpState::Partial]);
        assert_eq!(ledger_exit(&records), 1);
    }

    #[test]
    fn upload_error_fails() {
        let mut fake = Fake::new(true, &[]);
        fake.fail_on = Some("a.bin");
        let records = run(&fake, true);
        assert!(records.iter().any(|r| r.state == OpState::Failed));
        assert_eq!(ledger_exit(&records), 1);
    }

    #[test]
    fn interrupted_second_upload_is_partial() {
        let mut fake = Fake::new(true, &[]);
        fake.fail_on = Some("b.bin");
        let records = run(&fake, true);
        assert_eq!(fake.uploads.borrow().as_slice(), ["a.bin"]);
        assert_eq!(states(&records), vec![OpState::Validated, OpState::Partial]);
        assert_eq!(ledger_exit(&records), 1);
    }

    #[test]
    fn unapproved_is_not_authorized_and_uploads_nothing() {
        let fake = Fake::new(true, &[]);
        let records = run(&fake, false);
        assert!(fake.uploads.borrow().is_empty());
        assert_eq!(states(&records), vec![OpState::NotAuthorized; 2]);
        assert_eq!(ledger_exit(&records), 0);
    }

    #[test]
    fn view_error_fails() {
        struct Down;
        impl ReleaseBackend for Down {
            fn view(&self, _: &str) -> Result<String, String> {
                Err("offline".to_owned())
            }
            fn upload(&self, _: &str, _: &str) -> Result<String, String> {
                Err("offline".to_owned())
            }
        }
        let records = sync_assets(&Down, "v1", &staged(), &paths(), true);
        assert_eq!(states(&records), vec![OpState::Failed]);
    }

    #[test]
    fn ledger_renders_sorted_kebab_case_json() {
        let text = render_ledger(&[
            OpRecord::new("z", "s", OpState::NotAuthorized, "d"),
            OpRecord::new("a", "s", OpState::Executed, "d"),
        ]);
        assert!(text.contains("\"schema\": \"release-ledger/v1\""));
        assert!(text.contains("not-authorized"));
        assert!(text.find("\"a\"").unwrap() < text.find("\"z\"").unwrap());
        assert!(parse_release_assets("not json").is_err());
    }
}
