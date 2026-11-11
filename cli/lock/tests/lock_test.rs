//! The lock step writes the golden lock from the registry fixture, reuses it, and reports a stale one.

use std::cell::{Cell, RefCell};
use std::path::{Path, PathBuf};

use dx_lock::{
    Archive, Args, Error, Fetch, Lock, Outcome, Package, Resolution, Resolver, Scratch, Spec, run,
};

fn runfile(variable: &str) -> PathBuf {
    dx_testing::env_file(variable)
}

fn text(path: &Path) -> String {
    std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

fn spec() -> Spec {
    Spec {
        ecosystem: "fixture".to_owned(),
        manifest: vec!["fixture.toml".to_owned(), "fixture.json".to_owned()],
        state: vec!["fixture.state".to_owned()],
    }
}

/// Starts the fixture resolver in the scratch directory and counts the starts.
struct Registry {
    starts: Cell<usize>,
}

impl Resolver for Registry {
    fn resolve(&self, scratch: &Scratch, _: Option<&Lock>) -> Result<Resolution, Error> {
        self.starts.set(self.starts.get() + 1);
        let output = scratch
            .command(runfile("FIXTURE_RESOLVER"))
            .arg(runfile("REGISTRY_INDEX"))
            .output()
            .map_err(|error| Error::new(error.to_string()))?;
        if !output.status.success() {
            return Err(Error::new(String::from_utf8_lossy(&output.stderr)));
        }
        Resolution::from_json(&output.stdout)
    }
}

/// Returns a closure given by the test.
struct Fixed(Vec<Package>);

impl Resolver for Fixed {
    fn resolve(&self, _: &Scratch, _: Option<&Lock>) -> Result<Resolution, Error> {
        Ok(Resolution {
            packages: self.0.clone(),
        })
    }
}

/// Records whether the resolver got the previous lock and which packages it held.
#[derive(Default)]
struct Remembers(RefCell<Vec<Option<Vec<String>>>>);

impl Resolver for Remembers {
    fn resolve(&self, _: &Scratch, old: Option<&Lock>) -> Result<Resolution, Error> {
        let names = old.map(|lock| {
            lock.packages
                .iter()
                .map(|package| package.name.clone())
                .collect()
        });
        self.0.borrow_mut().push(names);
        Ok(Resolution {
            packages: vec![package("alpha", "alpha-1.2.0.pkg")],
        })
    }
}

/// Reads archives from the registry fixture and records every URL asked for.
#[derive(Default)]
struct Archives {
    asked: RefCell<Vec<String>>,
}

impl Fetch for Archives {
    fn sha256(&self, url: &str) -> Result<String, Error> {
        self.asked.borrow_mut().push(url.to_owned());
        let name = url
            .strip_prefix("https://registry.example/")
            .ok_or_else(|| Error::new("not the fixture registry"))?;
        let index = runfile("REGISTRY_INDEX");
        let path = index.parent().expect("registry directory").join(name);
        std::fs::read(&path)
            .map(|bytes| dx_digest::sha256_hex(&bytes))
            .map_err(|_| Error::new("HTTP 404"))
    }
}

impl Archives {
    fn asked(&self) -> Vec<String> {
        self.asked.borrow().clone()
    }
}

/// A set directory holding the fixture manifest.
fn set() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("set directory");
    std::fs::write(dir.path().join("fixture.json"), text(&runfile("MANIFEST"))).expect("manifest");
    dir
}

fn args(dir: &Path, check: bool) -> Args {
    Args {
        dir: dir.to_owned(),
        check,
        upgrade: Vec::new(),
    }
}

fn registry() -> Registry {
    Registry {
        starts: Cell::new(0),
    }
}

fn archive(name: &str) -> Archive {
    Archive {
        urls: vec![format!("https://registry.example/{name}")],
        ..Archive::default()
    }
}

fn package(name: &str, archive_name: &str) -> Package {
    Package {
        name: name.to_owned(),
        version: "1.0.0".to_owned(),
        archives: vec![archive(archive_name)],
        ..Package::default()
    }
}

#[test]
fn the_first_run_writes_the_golden_lock_and_the_state_file() {
    let (set, registry, archives) = (set(), registry(), Archives::default());
    let outcome = run(&spec(), &args(set.path(), false), &registry, &archives);
    assert_eq!(outcome, Ok(Outcome::Written));
    assert_eq!(
        text(&set.path().join("fixture.lock.json")),
        text(&runfile("GOLDEN"))
    );
    assert_eq!(
        text(&set.path().join("fixture.state")),
        "alpha 1.2.0\ngamma 3.0.0\nvendor/beta 0.4.1\n"
    );
    assert_eq!(
        archives.asked(),
        [
            "https://registry.example/alpha-1.2.0.pkg",
            "https://registry.example/beta-0.4.1.pkg"
        ],
        "gamma has a registry digest and is not downloaded"
    );
}

#[test]
fn a_second_run_with_equal_inputs_starts_no_resolver_and_no_download() {
    let (set, registry, archives) = (set(), registry(), Archives::default());
    run(&spec(), &args(set.path(), false), &registry, &archives).expect("first run");
    let lock = set.path().join("fixture.lock.json");
    let written = std::fs::metadata(&lock)
        .and_then(|meta| meta.modified())
        .expect("mtime");
    let (registry, archives) = (self::registry(), Archives::default());
    assert_eq!(
        run(&spec(), &args(set.path(), false), &registry, &archives),
        Ok(Outcome::Fresh)
    );
    assert_eq!(
        run(&spec(), &args(set.path(), true), &registry, &archives),
        Ok(Outcome::Fresh)
    );
    assert_eq!(registry.starts.get(), 0);
    assert_eq!(archives.asked(), Vec::<String>::new());
    assert_eq!(
        std::fs::metadata(&lock)
            .and_then(|meta| meta.modified())
            .expect("mtime"),
        written
    );
}

#[test]
fn check_fails_on_a_changed_manifest_and_writes_nothing() {
    let (set, registry, archives) = (set(), registry(), Archives::default());
    let lock = set.path().join("fixture.lock.json");
    assert_eq!(
        run(&spec(), &args(set.path(), true), &registry, &archives),
        Ok(Outcome::Stale)
    );
    assert!(!lock.exists(), "--check wrote a lock");
    run(&spec(), &args(set.path(), false), &registry, &archives).expect("first run");
    std::fs::write(
        set.path().join("fixture.json"),
        r#"{"require": ["alpha", "delta"]}"#,
    )
    .expect("manifest");
    let (registry, archives) = (self::registry(), Archives::default());
    assert_eq!(
        run(&spec(), &args(set.path(), true), &registry, &archives),
        Ok(Outcome::Stale)
    );
    assert_eq!(text(&lock), text(&runfile("GOLDEN")));
    assert_eq!(registry.starts.get(), 0);
    assert_eq!(archives.asked(), Vec::<String>::new());
}

#[test]
fn check_fails_on_a_changed_state_file() {
    let (set, registry, archives) = (set(), registry(), Archives::default());
    run(&spec(), &args(set.path(), false), &registry, &archives).expect("first run");
    std::fs::write(set.path().join("fixture.state"), "alpha 9.9.9\n").expect("state");
    assert_eq!(
        run(&spec(), &args(set.path(), true), &registry, &archives),
        Ok(Outcome::Stale)
    );
}

#[test]
fn a_changed_manifest_downloads_only_the_new_package() {
    let (set, registry, archives) = (set(), registry(), Archives::default());
    run(&spec(), &args(set.path(), false), &registry, &archives).expect("first run");
    std::fs::write(
        set.path().join("fixture.json"),
        r#"{"require": ["alpha", "delta"]}"#,
    )
    .expect("manifest");
    let (registry, archives) = (self::registry(), Archives::default());
    assert_eq!(
        run(&spec(), &args(set.path(), false), &registry, &archives),
        Ok(Outcome::Written)
    );
    assert_eq!(
        archives.asked(),
        ["https://registry.example/delta-2.0.0.pkg"]
    );
    let lock = Lock::from_json(&text(&set.path().join("fixture.lock.json"))).expect("lock");
    let names: Vec<&str> = lock
        .packages
        .iter()
        .map(|package| package.name.as_str())
        .collect();
    assert_eq!(names, ["alpha", "delta", "vendor/beta"]);
    assert_eq!(
        text(&set.path().join("fixture.state")),
        "alpha 1.2.0\ndelta 2.0.0\nvendor/beta 0.4.1\n"
    );
    assert_eq!(
        run(&spec(), &args(set.path(), true), &registry, &archives),
        Ok(Outcome::Fresh)
    );
}

#[test]
fn the_resolver_gets_the_previous_lock_only_after_a_first_run() {
    let set = set();
    let remembers = Remembers::default();
    run(
        &spec(),
        &args(set.path(), false),
        &remembers,
        &Archives::default(),
    )
    .expect("first run");
    std::fs::write(set.path().join("fixture.json"), "{}").expect("manifest");
    run(
        &spec(),
        &args(set.path(), false),
        &remembers,
        &Archives::default(),
    )
    .expect("second run");
    assert_eq!(
        *remembers.0.borrow(),
        [None, Some(vec!["alpha".to_owned()])]
    );
}

#[test]
fn a_changed_url_or_version_downloads_the_archive_again() {
    let set = set();
    let archives = Archives::default();
    let first = Fixed(vec![
        package("alpha", "alpha-1.2.0.pkg"),
        package("delta", "delta-2.0.0.pkg"),
    ]);
    run(&spec(), &args(set.path(), false), &first, &archives).expect("first run");
    std::fs::write(set.path().join("fixture.json"), "{}").expect("manifest");
    let mut moved = package("alpha", "beta-0.4.1.pkg");
    moved.archives[0]
        .urls
        .push("https://mirror.example/alpha.pkg".to_owned());
    let mut bumped = package("delta", "delta-2.0.0.pkg");
    bumped.version = "2.0.1".to_owned();
    let archives = Archives::default();
    run(
        &spec(),
        &args(set.path(), false),
        &Fixed(vec![moved, bumped]),
        &archives,
    )
    .expect("second run");
    assert_eq!(
        archives.asked(),
        [
            "https://registry.example/beta-0.4.1.pkg",
            "https://registry.example/delta-2.0.0.pkg"
        ]
    );
}

#[test]
fn the_bytes_do_not_depend_on_the_order_the_resolver_lists() {
    let mut alpha = package("alpha", "alpha-1.2.0.pkg");
    alpha.deps = vec!["delta".to_owned(), "beta".to_owned(), "delta".to_owned()];
    alpha.provides = vec!["b".to_owned(), "a".to_owned()];
    alpha
        .extra
        .insert("zeta".to_owned(), serde_json::json!({"b": 1, "a": 2}));
    alpha.extra.insert("eta".to_owned(), serde_json::json!("x"));
    alpha.archives = vec![
        Archive {
            platform: "macos_arm64".to_owned(),
            ..archive("delta-2.0.0.pkg")
        },
        Archive {
            platform: "linux_x86_64".to_owned(),
            strip_prefix: "alpha-1.2.0".to_owned(),
            ..archive("alpha-1.2.0.pkg")
        },
    ];
    let listed = vec![
        package("delta", "delta-2.0.0.pkg"),
        alpha,
        package("beta", "beta-0.4.1.pkg"),
    ];
    let mut reversed = listed.clone();
    reversed.reverse();
    reversed[1].archives.reverse();
    let (one, two) = (set(), set());
    run(
        &spec(),
        &args(one.path(), false),
        &Fixed(listed),
        &Archives::default(),
    )
    .expect("listed");
    run(
        &spec(),
        &args(two.path(), false),
        &Fixed(reversed),
        &Archives::default(),
    )
    .expect("reversed");
    let bytes = text(&one.path().join("fixture.lock.json"));
    assert_eq!(bytes, text(&two.path().join("fixture.lock.json")));
    let lock = Lock::from_json(&bytes).expect("lock");
    assert_eq!(lock.to_bytes(), bytes);
    assert_eq!(lock.packages[0].deps, ["beta", "delta"]);
    assert_eq!(lock.packages[0].archives[0].platform, "linux_x86_64");
    let keys = |needle: &str| {
        bytes
            .find(needle)
            .unwrap_or_else(|| panic!("{needle} is in the lock"))
    };
    assert!(keys("\"eta\"") < keys("\"zeta\""));
    assert!(keys("\"a\": 2") < keys("\"b\": 1"));
    assert!(bytes.ends_with("}\n") && !bytes.contains('\r'));
}

#[test]
fn a_mirror_is_read_when_the_first_url_fails() {
    let set = set();
    let mut alpha = package("alpha", "missing.pkg");
    alpha.archives[0]
        .urls
        .push("https://registry.example/alpha-1.2.0.pkg".to_owned());
    run(
        &spec(),
        &args(set.path(), false),
        &Fixed(vec![alpha]),
        &Archives::default(),
    )
    .expect("run");
    let lock = Lock::from_json(&text(&set.path().join("fixture.lock.json"))).expect("lock");
    assert_eq!(
        lock.packages[0].archives[0].sha256,
        "acf33cfbc5442b1565bc6d1ed0cfde327f3480a1f18416fdcced13de8c1e0433"
    );
}

fn failure(packages: Vec<Package>) -> String {
    let set = set();
    let error = run(
        &spec(),
        &args(set.path(), false),
        &Fixed(packages),
        &Archives::default(),
    )
    .expect_err("the lock step fails");
    assert!(
        !set.path().join("fixture.lock.json").exists(),
        "a failed step wrote a lock"
    );
    error.to_string()
}

#[test]
fn an_unusable_closure_writes_no_lock() {
    let mut needs = package("alpha", "alpha-1.2.0.pkg");
    needs.deps = vec!["omega".to_owned()];
    let mut plain = package("alpha", "alpha-1.2.0.pkg");
    plain.archives[0].urls = vec!["http://registry.example/alpha-1.2.0.pkg".to_owned()];
    let mut bare = package("alpha", "alpha-1.2.0.pkg");
    bare.archives.clear();
    let mut twice = package("alpha", "alpha-1.2.0.pkg");
    twice.archives.push(archive("beta-0.4.1.pkg"));
    let mut short = package("alpha", "alpha-1.2.0.pkg");
    short.archives[0].sha256 = "abc".to_owned();
    let mut odd = package("alpha", "alpha-1.2.0.pkg");
    odd.archives[0].platform = "plan9".to_owned();
    for (packages, want) in [
        (
            vec![needs],
            "package alpha needs omega, which is not locked",
        ),
        (vec![plain], "want https"),
        (vec![bare], "package alpha has no archive"),
        (
            vec![twice],
            "package alpha has two archives for every platform",
        ),
        (vec![short], "want 64 hex characters"),
        (vec![odd], "unknown platform plan9"),
        (
            vec![
                package("alpha", "alpha-1.2.0.pkg"),
                package("alpha", "beta-0.4.1.pkg"),
            ],
            "package alpha is listed twice",
        ),
        (
            vec![package("../alpha", "alpha-1.2.0.pkg")],
            "has a name outside",
        ),
        (
            vec![package("alpha", "missing.pkg")],
            "package alpha 1.0.0 has no archive to read:\nhttps://registry.example/missing.pkg: HTTP 404",
        ),
    ] {
        let error = failure(packages);
        assert!(error.contains(want), "{error:?} does not hold {want:?}");
    }
}

#[test]
fn a_set_without_a_manifest_fails() {
    let dir = tempfile::tempdir().expect("set directory");
    let error = run(
        &spec(),
        &args(dir.path(), false),
        &registry(),
        &Archives::default(),
    )
    .expect_err("no manifest");
    assert!(
        error
            .to_string()
            .ends_with("has no fixture.toml or fixture.json"),
        "{error}"
    );
}

#[test]
fn a_manifest_pattern_takes_every_matching_file() {
    let dir = tempfile::tempdir().expect("set directory");
    for name in ["b.fixture", "a.fixture", "other.txt"] {
        std::fs::write(dir.path().join(name), name).expect("file");
    }
    let spec = Spec {
        ecosystem: "fixture".to_owned(),
        manifest: vec!["*.fixture".to_owned()],
        state: Vec::new(),
    };
    run(
        &spec,
        &args(dir.path(), false),
        &Fixed(Vec::new()),
        &Archives::default(),
    )
    .expect("run");
    let lock = Lock::from_json(&text(&dir.path().join("fixture.lock.json"))).expect("lock");
    assert_eq!(
        lock.inputs.keys().collect::<Vec<_>>(),
        ["a.fixture", "b.fixture"]
    );
}

#[test]
fn the_flags_are_dir_and_check() {
    let parse = |args: &[&str]| Args::parse(args.iter().map(|arg| (*arg).to_owned()));
    let root = if cfg!(windows) { "C:\\set" } else { "/set" };
    assert_eq!(
        parse(&["--dir", root]),
        Ok(Args {
            dir: PathBuf::from(root),
            check: false,
            upgrade: Vec::new()
        })
    );
    assert_eq!(
        parse(&["--check", &format!("--dir={root}")]),
        Ok(Args {
            dir: PathBuf::from(root),
            check: true,
            upgrade: Vec::new()
        })
    );
    for (args, want) in [
        (vec!["--check"], "--dir is missing"),
        (vec!["--dir", "set"], "--dir set is not absolute"),
        (vec!["--dir", root, "--update"], "unknown argument --update"),
    ] {
        let error = parse(&args).expect_err("rejected").to_string();
        assert!(error.starts_with(want), "{error}");
    }
}

fn schema_errors(lock: &serde_json::Value) -> Vec<String> {
    let schema: serde_json::Value =
        serde_json::from_str(&text(&runfile("LOCK_SCHEMA"))).expect("schema is JSON");
    let validator = jsonschema::draft202012::new(&schema).expect("schema compiles");
    validator
        .iter_errors(lock)
        .map(|error| format!("{}: {error}", error.instance_path()))
        .collect()
}

#[test]
fn the_golden_lock_matches_the_schema() {
    let golden: serde_json::Value =
        serde_json::from_str(&text(&runfile("GOLDEN"))).expect("golden is JSON");
    assert_eq!(schema_errors(&golden), Vec::<String>::new());
    assert_eq!(
        Lock::from_json(&text(&runfile("GOLDEN")))
            .expect("golden")
            .to_bytes(),
        text(&runfile("GOLDEN"))
    );
}

#[test]
fn the_schema_rejects_what_the_design_bans() {
    let golden: serde_json::Value =
        serde_json::from_str(&text(&runfile("GOLDEN"))).expect("golden is JSON");
    let edits: [(&str, fn(&mut serde_json::Value)); 11] = [
        ("no digest", |lock| {
            lock["packages"][0]["archives"][0]
                .as_object_mut()
                .expect("archive")
                .remove("sha256");
        }),
        ("short digest", |lock| {
            lock["packages"][0]["archives"][0]["sha256"] = "abc".into()
        }),
        ("no archive", |lock| {
            lock["packages"][0]["archives"] = serde_json::json!([])
        }),
        ("plain http", |lock| {
            lock["packages"][0]["archives"][0]["urls"][0] = "http://registry.example/a".into()
        }),
        ("unknown platform", |lock| {
            lock["packages"][0]["archives"][0]["platform"] = "plan9".into()
        }),
        ("unknown package key", |lock| {
            lock["packages"][0]["integrity"] = "sha512-x".into()
        }),
        ("unlisted extra key", |lock| {
            lock["packages"][0]["extra"] = serde_json::json!({"main": "a"})
        }),
        ("swift target without a module", |lock| {
            lock["packages"][0]["extra"] = serde_json::json!({"swift_targets": {"A": {"deps": [], "path": "p", "product_deps": [], "srcs": []}}});
        }),
        ("swift target with an unknown key", |lock| {
            lock["packages"][0]["extra"] = serde_json::json!({"swift_targets": {"A": {"deps": [], "flags": [], "module": "A", "path": "p", "product_deps": [], "srcs": []}}});
        }),
        ("no inputs", |lock| lock["inputs"] = serde_json::json!({})),
        ("another schema", |lock| lock["schema"] = 2.into()),
    ];
    for (name, edit) in edits {
        let mut lock = golden.clone();
        edit(&mut lock);
        assert!(
            !schema_errors(&lock).is_empty(),
            "the schema accepts a lock with {name}"
        );
    }
}

#[test]
fn the_schema_accepts_swift_extra() {
    let mut lock: serde_json::Value =
        serde_json::from_str(&text(&runfile("GOLDEN"))).expect("golden is JSON");
    lock["packages"][0]["extra"] = serde_json::json!({
        "swift_products": {"A": ["A"]},
        "swift_targets": {"A": {"deps": [], "module": "A", "path": "Sources/A", "product_deps": ["b:B"], "srcs": ["Sources/A/a.swift"]}},
    });
    assert_eq!(schema_errors(&lock), Vec::<String>::new());
}

#[test]
fn upgrade_flags_collect_package_names() {
    let root = if cfg!(windows) { "C:\\set" } else { "/set" };
    let parse = |args: &[&str]| Args::parse(args.iter().map(|arg| (*arg).to_owned()));
    let args = parse(&["--dir", root, "--upgrade", "a/b", "--upgrade=c/d"]).expect("args");
    assert_eq!(args.upgrade, ["a/b", "c/d"]);
}
