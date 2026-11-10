use super::{
    AUDIT_TODAY_ENV, QueryBazel, parse_locked_for_configured, today_utc_from, uncovered_message,
};
use dx_adopt::dependency_sets::{Ecosystem, ResolvedSet};
use dx_plan::resolve::{QueryResult, QueryRunner};
use dx_tool_resolve::Bazel as _;
use std::cell::RefCell;
use std::path::Path;

fn unreadable_lock_error(ecosystem: Ecosystem, lock: &str) -> String {
    let set = ResolvedSet {
        name: "deps".to_owned(),
        ecosystem,
        dir: String::new(),
        manifests: Vec::new(),
        locks: vec![lock.to_owned()],
        repository: None,
    };
    let locks = vec![(lock.to_owned(), "{}".to_owned())];
    parse_locked_for_configured(&set, &locks)
        .err()
        .map(|error| error.to_string())
        .unwrap_or_default()
}

#[test]
fn uncovered_message_names_the_set_and_ecosystem() {
    let uv = uncovered_message("tools", "uv");
    assert!(uv.contains("tools") && uv.contains("(uv)"), "{uv}");
    assert!(!uv.contains("PowerShell"), "{uv}");
    let shell = uncovered_message("powershell", "powershell");
    assert_eq!(
        shell,
        "failed to assess powershell: no advisory backend for PowerShell Gallery modules; NuGet.org advisories do not apply; accept it with an [[uncovered]] entry in security.toml"
    );
}

#[test]
fn uncovered_message_names_luarocks() {
    assert_eq!(
        uncovered_message("rocks", "luarocks"),
        "failed to assess rocks: no advisory backend for LuaRocks rocks; OSV defines no LuaRocks ecosystem (https://ossf.github.io/osv-schema/); accept it with an [[uncovered]] entry in security.toml"
    );
}

#[test]
fn set_without_a_readable_lock_fails() {
    let cargo = unreadable_lock_error(Ecosystem::Cargo, "cargo-bazel-lock.json");
    assert!(
        cargo.contains("deps") && cargo.contains("Cargo.lock"),
        "{cargo}"
    );
    let nuget = unreadable_lock_error(Ecosystem::Nuget, "other.lock");
    assert!(nuget.contains("paket.lock"), "{nuget}");
    let maven = unreadable_lock_error(Ecosystem::Maven, "pins.txt");
    assert!(maven.contains(".json"), "{maven}");
}

fn locked_names(ecosystem: Ecosystem, lock: &str, text: &str) -> Vec<(String, String)> {
    let set = ResolvedSet {
        name: "deps".to_owned(),
        ecosystem,
        dir: String::new(),
        manifests: Vec::new(),
        locks: vec![lock.to_owned()],
        repository: None,
    };
    let locks = vec![(lock.to_owned(), text.to_owned())];
    let (packages, _) = parse_locked_for_configured(&set, &locks).expect("parses");
    packages.into_iter().map(|p| (p.name, p.version)).collect()
}

#[test]
fn fpm_lock_packages_are_audited() {
    let text = "[dependencies.alpha]\nversion = \"0.1.0\"\n";
    assert_eq!(
        locked_names(Ecosystem::Fpm, "native/fpm.lock", text),
        [("alpha".to_owned(), "0.1.0".to_owned())]
    );
}

#[test]
fn stack_lock_hackage_entries_are_audited() {
    let text = "packages:\n- completed:\n    hackage: aeson-2.1.0.0@sha256:ab,100\n  original:\n    hackage: aeson-2.1.0.0\n";
    assert_eq!(
        locked_names(Ecosystem::Cabal, "app/stack.yaml.lock", text),
        [("aeson".to_owned(), "2.1.0.0".to_owned())]
    );
}

#[test]
fn zig_digests_carry_no_versions_to_audit() {
    assert!(locked_names(Ecosystem::Zig, "native/zig.digests.json", "{}").is_empty());
}

struct Recorder(RefCell<Vec<Vec<String>>>);

impl QueryRunner for Recorder {
    fn run_query(&self, argv: &[String], _cwd: &Path) -> std::io::Result<QueryResult> {
        self.0.borrow_mut().push(argv.to_vec());
        Ok(QueryResult {
            code: Some(0),
            stdout: Vec::new(),
            stderr: Vec::new(),
        })
    }
}

#[test]
fn the_tool_cquery_carries_the_shared_settings_and_ends_with_the_label() {
    let recorder = Recorder(RefCell::new(Vec::new()));
    let bazel = QueryBazel {
        query_runner: &recorder,
        workspace: Path::new("."),
        startup: &[],
    };
    bazel.cquery_files("//t:tool", &[]).expect("cquery");
    let calls = recorder.0.borrow();
    let argv = &calls[0];
    for setting in dx_plan::plan::shared_settings("cquery") {
        assert!(argv.contains(&setting), "{setting}");
    }
    assert_eq!(argv.last().map(String::as_str), Some("//t:tool"));
}

#[test]
fn a_pinned_date_makes_the_audit_date_reproducible() {
    assert_eq!(today_utc_from(Some("2020-01-02".to_owned())), "2020-01-02");
    assert_eq!(AUDIT_TODAY_ENV, "DX_AUDIT_TODAY");
}

#[test]
fn an_unusable_pin_falls_back_to_today() {
    for bad in [
        "",
        "2020-1-2",
        "2020-01-02T00:00",
        "not-a-date",
        "2020/01/02x",
    ] {
        let today = today_utc_from(None);
        assert_eq!(
            today_utc_from(Some(bad.to_owned())),
            today,
            "{bad:?} is not a pinned date"
        );
    }
}
