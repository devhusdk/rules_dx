use super::*;

fn manifest_for(label: &str, languages: &[&str]) -> Manifest {
    Manifest {
        label: label.to_owned(),
        languages: languages
            .iter()
            .map(|language| (*language).to_owned())
            .collect(),
        module: "m".to_owned(),
        repo_name: "r".to_owned(),
        workspace_dir: "ws".to_owned(),
        patterns: vec!["//...".to_owned()],
        failures: Vec::new(),
        coverage: Vec::new(),
        covered: Vec::new(),
        generate: false,
        files: Vec::new(),
        module_fragment: Some(PathBuf::from("f")),
        families: Vec::new(),
        compiler: None,
        platform: String::new(),
        lint_findings: Vec::new(),
        dx: PathBuf::from("d"),
        module_file: PathBuf::from("MODULE.bazel"),
        bazelversion: PathBuf::from(".bazelversion"),
    }
}

const CPP_RUST_LANGUAGES: &[&str] = &["cpp", "java", "javascript", "kotlin", "rust", "typescript"];

fn cpp_rust() -> Manifest {
    manifest_for("//tools/testing/hermetic:cpp_rust", CPP_RUST_LANGUAGES)
}

fn probe_record(name: &str, expect: &str, marker: &str, network: &str, argv: &[&str]) -> Value {
    json!({
        "name": name,
        "verdict": "ok",
        "expect": expect,
        "marker": marker,
        "network": network,
        "argv": argv,
    })
}

fn good_evidence() -> Value {
    let probes: Vec<Value> = REQUIRED_PROBES
        .iter()
        .map(|name| {
            let (expect, marker) = probe_expectation(name, "linux");
            let network = if *name == "fetch" {
                "allowed"
            } else {
                "denied"
            };
            probe_record(name, expect, marker, network, &[])
        })
        .collect();
    json!({
        "schema": SCHEMA,
        "qualification": "//tools/testing/hermetic:cpp_rust",
        "languages": ["cpp", "java", "javascript", "kotlin", "rust", "typescript"],
        "rules_dx": {"commit": "abc123", "dirty": false},
        "result": "qualified",
        "host": {"os": "linux", "arch": "x86_64", "kernel": "6.8.0"},
        "probes": probes,
        "artifacts": [
            {"path": "bazel-out/k8-fastbuild/bin/app", "sha256": sha256_hex(b"app"), "machine": "x86_64"},
            {"path": "bazel-out/k8-fastbuild/bin/libapp.a", "sha256": sha256_hex(b"lib"), "machine": null},
        ],
        "target": {"machines": ["x86_64"]},
    })
}

fn verify(evidence: &Value) -> Result<(), String> {
    verify_evidence(evidence, "abc123", &cpp_rust())
}

#[test]
fn sha256_matches_the_standard_vectors() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
    assert_eq!(
        sha256_hex(b"abcdbcdecdefdefgefghfghighijhijkijkljklmklmnlmnomnopnopq"),
        "248d6a61d20638b8e5c026930c3e6039a33ce45964ff2167f6ecedd419db06c1"
    );
    assert_eq!(
        sha256_hex(&[b'a'; 1000]),
        "41edece42d63e8d9bf515a9ba6932e1c20cbc9f5a5d134645adb5db1b9737ea3"
    );
}

#[test]
fn consumer_paths_stay_inside_the_workspace() {
    assert_eq!(
        checked_dest("examples/a b/BUILD.bazel").unwrap(),
        "examples/a b/BUILD.bazel"
    );
    for bad in [
        "",
        "/etc/passwd",
        "../up",
        "a/../b",
        "a//b",
        "./a",
        "a\\b",
        "a/",
    ] {
        assert!(checked_dest(bad).is_err(), "{bad:?} must be rejected");
    }
}

#[test]
fn manifest_parses_and_resolves_against_the_runfiles_base() {
    let text = json!({
        "label": "//tools/testing/hermetic:cpp_rust",
        "languages": ["cpp", "java", "javascript", "kotlin", "rust", "typescript"],
        "module": "hermetic_consumer",
        "repo_name": "my_dx",
        "workspace_dir": "consumer ws",
        "patterns": ["//..."],
        "failures": {},
        "coverage": [],
        "covered": [],
        "files": [{"src": "examples/x.cc", "dest": "examples/x.cc"}],
        "module_fragment": "tools/testing/hermetic/cpp_rust.MODULE.bazel",
        "dx": "cli/cli/dx",
        "module_file": "MODULE.bazel",
        "bazelversion": ".bazelversion",
    })
    .to_string();
    let manifest = parse_manifest(&text, Path::new("/run")).unwrap();
    assert_eq!(manifest.workspace_dir, "consumer ws");
    assert_eq!(
        manifest.languages,
        vec!["cpp", "java", "javascript", "kotlin", "rust", "typescript"]
    );
    assert_eq!(
        manifest.files,
        vec![(
            PathBuf::from("/run/examples/x.cc"),
            "examples/x.cc".to_owned()
        )]
    );
    assert_eq!(manifest.dx, PathBuf::from("/run/cli/cli/dx"));
    let mut flagged: Value = serde_json::from_str(&text).unwrap();
    assert!(manifest.families.is_empty());
    flagged["families"] = json!(["go"]);
    let manifest = parse_manifest(&flagged.to_string(), Path::new("/run")).unwrap();
    assert_eq!(manifest.families, vec!["go".to_owned()]);
}

#[test]
fn manifest_rejects_escaping_files_and_empty_patterns() {
    let base = json!({
        "label": "//x:y",
        "languages": ["cpp"],
        "module": "m",
        "repo_name": "r",
        "workspace_dir": "ws",
        "patterns": ["//..."],
        "failures": {},
        "coverage": [],
        "covered": [],
        "files": [{"src": "a", "dest": "../a"}],
        "module_fragment": "f",
        "dx": "d",
        "module_file": "MODULE.bazel",
        "bazelversion": ".bazelversion",
    });
    let err = parse_manifest(&base.to_string(), Path::new("/")).unwrap_err();
    assert!(err.contains("inside the workspace"), "{err}");
    let mut empty = base.clone();
    empty["files"] = json!([]);
    empty["patterns"] = json!([]);
    let err = parse_manifest(&empty.to_string(), Path::new("/")).unwrap_err();
    assert!(err.contains("no consumer target patterns"), "{err}");
    let mut negative = base.clone();
    negative["files"] = json!([]);
    negative["patterns"] = json!(["//...", "-//slow/..."]);
    let err = parse_manifest(&negative.to_string(), Path::new("/")).unwrap_err();
    assert!(err.contains("-//slow/... is negative"), "{err}");
    let mut no_language = base;
    no_language["files"] = json!([]);
    no_language["languages"] = json!([]);
    let err = parse_manifest(&no_language.to_string(), Path::new("/")).unwrap_err();
    assert!(err.contains("no languages"), "{err}");
    let mut unnamed = no_language;
    unnamed["languages"] = json!(["cpp"]);
    unnamed["label"] = json!("");
    let err = parse_manifest(&unnamed.to_string(), Path::new("/")).unwrap_err();
    assert_eq!(err, "manifest field label must be a non-empty string");
}

#[test]
fn verdict_requires_the_intended_failure_reason() {
    assert!(verdict(&Expect::Pass, 0, "").is_ok());
    assert_eq!(
        verdict(&Expect::Pass, 1, "").unwrap_err(),
        "exited 1, want 0"
    );
    let fail = Expect::Fail(NETWORK_MARKER.to_owned());
    assert!(verdict(&fail, 1, "connect: Network is unreachable").is_ok());
    assert_eq!(
        verdict(&fail, 0, "Network is unreachable").unwrap_err(),
        "succeeded, want a failure"
    );
    assert_eq!(
        verdict(&fail, 1, "No such file or directory").unwrap_err(),
        "exited 1 without \"Network is unreachable\""
    );
}

#[test]
fn wasm_machine_names_core_modules() {
    assert_eq!(
        wasm_machine(b"\0asm\x01\0\0\0\x01\x04").as_deref(),
        Some("wasm32")
    );
    assert_eq!(wasm_machine(b"\0asm\x0d\0\x01\0"), None);
    assert_eq!(wasm_machine(b"\0asm\x01\0\0"), None);
    assert_eq!(wasm_machine(b"!<arch>\nxxxxxxxx"), None);
}

#[test]
fn shell_quote_survives_spaces_and_quotes() {
    assert_eq!(shell_quote("/a b/c"), "'/a b/c'");
    assert_eq!(shell_quote("it's"), "'it'\\''s'");
}

#[test]
fn passwd_and_status_name_the_real_user() {
    let passwd = "root:x:0:0:root:/root:/bin/bash\nrunner:x:1001:118::/home/runner:/bin/bash\n";
    assert_eq!(
        passwd_home(passwd, 1001),
        Some(PathBuf::from("/home/runner"))
    );
    assert_eq!(passwd_home(passwd, 7), None);
    let status = "Name:\tqualify\nUid:\t1001\t1001\t1001\t1001\nGid:\t118\t118\t118\t118\n";
    assert_eq!(status_id(status, "Uid:"), Some(1001));
    assert_eq!(status_id(status, "Gid:"), Some(118));
    assert_eq!(status_id(status, "Groups:"), None);
}

#[cfg(unix)]
#[test]
fn ambient_tools_resolve_links_once() {
    let dir = tempfile::tempdir().unwrap();
    let bin = dir.path().join("bin");
    let usr = dir.path().join("usr bin");
    std::fs::create_dir_all(&bin).unwrap();
    std::fs::create_dir_all(&usr).unwrap();
    std::fs::write(usr.join("gcc-13"), "").unwrap();
    std::os::unix::fs::symlink(usr.join("gcc-13"), usr.join("gcc")).unwrap();
    std::os::unix::fs::symlink(usr.join("gcc-13"), usr.join("cc")).unwrap();
    std::os::unix::fs::symlink(usr.join("gcc"), bin.join("cc")).unwrap();
    std::fs::create_dir_all(usr.join("node")).unwrap();
    let found = ambient_tools(&[bin, usr.clone()], &["cc", "gcc", "node", "python3"]);
    let real = std::fs::canonicalize(usr.join("gcc-13")).unwrap();
    assert_eq!(found, vec![("cc".to_owned(), real)]);
}

#[cfg(target_os = "linux")]
#[test]
fn isolation_script_masks_before_dropping_to_the_real_user() {
    let isolation = Isolation {
        shim: PathBuf::from("/q/bin/ambient-tool"),
        tools: vec![PathBuf::from("/usr/bin/x86_64-linux-gnu-gcc-13")],
        sdk_dirs: vec![PathBuf::from("/opt")],
        home_dirs: vec![PathBuf::from("/home/u/.cargo")],
        home_files: vec![PathBuf::from("/home/u/my rc")],
        ip: PathBuf::from("/usr/sbin/ip"),
        uid: 1001,
        gid: 118,
        launcher: PathBuf::new(),
        grants: Vec::new(),
        denies: Vec::new(),
        group: None,
    };
    assert_eq!(
        isolation.script(Net::Denied),
        "set -eu\n'/usr/sbin/ip' link set lo up\nmount -t tmpfs dx-hermetic '/opt'\nmount -t tmpfs dx-hermetic '/home/u/.cargo'\nmount --bind /dev/null '/home/u/my rc'\nmount --bind '/q/bin/ambient-tool' '/usr/bin/x86_64-linux-gnu-gcc-13'\nexec unshare --user --map-user=1001 --map-group=118 -- \"$@\"\n"
    );
    assert!(!isolation.script(Net::Allowed).contains("link set lo up"));
    let command = isolation.command(Net::Denied, &["/bin/true".to_owned()]);
    let args: Vec<String> = command
        .get_args()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect();
    assert_eq!(
        &args[..5],
        ["--user", "--map-root-user", "--mount", "--net", "--"]
    );
    assert_eq!(args.last().map(String::as_str), Some("/bin/true"));
    let fetch = isolation.command(Net::Allowed, &[]);
    assert!(!fetch.get_args().any(|arg| arg == "--net"));
}

#[test]
fn leak_package_targets_stay_out_of_wildcards() {
    let build = leaks_build("cc");
    assert!(build.contains("cmd = \"cc --version > $@\""));
    assert!(build.contains(
        "cmd = \"test \\\"$${DX_HOME_CONFIG:-absent}\\\" = present || { echo dx-hermetic: home config $${DX_HOME_CONFIG:-absent} >&2; exit 1; }; echo present > $@\","
    ));
    assert!(!build.contains(HOME_MARKER));
    assert_eq!(HOME_RC, "build --action_env=DX_HOME_CONFIG=present\n");
    assert!(build.contains("/dev/tcp/1.1.1.1/443"));
    assert_eq!(build.matches("\"manual\"").count(), 3);
    assert!(build.contains("\"requires-network\""));
}

#[test]
fn built_query_drops_manual_targets_from_every_pattern() {
    assert_eq!(
        built_query(&["//...".to_owned()]),
        "(//...) except attr(tags, \"\\bmanual\\b\", //...)"
    );
    assert_eq!(
        built_query(&["//a/...".to_owned(), "//b:c".to_owned()]),
        "(//a/... + //b:c) except attr(tags, \"\\bmanual\\b\", //a/... + //b:c)"
    );
}

#[test]
fn artifacts_hash_and_identify_each_listed_file() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("bazel-out/bin");
    std::fs::create_dir_all(out.join("tree")).unwrap();
    let mut elf = vec![0x7f, b'E', b'L', b'F', 2, 1, 1, 0];
    elf.resize(18, 0);
    elf.extend_from_slice(&[0xb7, 0x00, 1, 2]);
    std::fs::write(out.join("app"), &elf).unwrap();
    std::fs::write(out.join("lib a.a"), b"!<arch>\n").unwrap();
    let listed = "bazel-out/bin/app\n\nbazel-out/bin/lib a.a\nbazel-out/bin/tree\n";
    let found = artifacts(dir.path(), listed).unwrap();
    assert_eq!(
        found,
        vec![
            json!({"path": "bazel-out/bin/app", "bytes": 22, "sha256": sha256_hex(&elf), "machine": "aarch64", "producers": []}),
            json!({"path": "bazel-out/bin/lib a.a", "bytes": 8, "sha256": sha256_hex(b"!<arch>\n"), "machine": null, "producers": []}),
        ]
    );
    assert_eq!(unique_machines(&found), vec!["aarch64"]);
    assert!(artifacts(dir.path(), "bazel-out/bin/tree\n").is_err());
    assert!(artifacts(dir.path(), "bazel-out/bin/missing\n").is_err());
}

#[test]
fn artifacts_records_wasm32_for_modules() {
    let dir = tempfile::tempdir().unwrap();
    let out = dir.path().join("bazel-out/bin");
    std::fs::create_dir_all(&out).unwrap();
    let module = b"\0asm\x01\0\0\0";
    std::fs::write(out.join("tasks.wasm"), module).unwrap();
    std::fs::write(out.join("tasks"), fake_elf()).unwrap();
    let found = artifacts(
        dir.path(),
        "bazel-out/bin/tasks.wasm\nbazel-out/bin/tasks\n",
    )
    .unwrap();
    assert_eq!(found[0]["machine"], "wasm32");
    assert_eq!(found[1]["machine"], "x86_64");
    assert_eq!(unique_machines(&found), vec!["wasm32", "x86_64"]);
}

#[test]
fn verify_accepts_complete_current_evidence() {
    verify(&good_evidence()).unwrap();
}

#[test]
fn verify_rejects_stale_dirty_or_foreign_evidence() {
    let mut stale = good_evidence();
    stale["rules_dx"]["commit"] = json!("old");
    assert!(verify(&stale).unwrap_err().contains("stale"));
    let mut dirty = good_evidence();
    dirty["rules_dx"]["dirty"] = json!(true);
    assert!(verify(&dirty).unwrap_err().contains("uncommitted"));
    let mut foreign = good_evidence();
    foreign["qualification"] = json!("//other:qualification");
    assert!(
        verify(&foreign)
            .unwrap_err()
            .contains("want //tools/testing/hermetic:cpp_rust")
    );
    let mut schema = good_evidence();
    schema["schema"] = json!("dx.hermetic.v0");
    assert!(verify(&schema).unwrap_err().contains("schema"));
    let mut failed = good_evidence();
    failed["result"] = json!("failed");
    assert!(verify(&failed).unwrap_err().contains("result"));
    let mut hostless = good_evidence();
    hostless["host"]["kernel"] = json!("");
    assert!(verify(&hostless).unwrap_err().contains("host kernel"));
}

#[test]
fn verify_rejects_missing_or_broken_probes() {
    for name in REQUIRED_PROBES {
        let mut missing = good_evidence();
        missing["probes"]
            .as_array_mut()
            .unwrap()
            .retain(|probe| probe["name"] != *name);
        assert_eq!(
            verify(&missing).unwrap_err(),
            format!("evidence is missing required probe {name}")
        );
    }
    let mut wrong = good_evidence();
    wrong["probes"][4]["verdict"] = json!("wrong");
    assert!(verify(&wrong).unwrap_err().contains("did not hold"));
    let mut online = good_evidence();
    online["probes"][1]["network"] = json!("allowed");
    assert!(verify(&online).unwrap_err().contains("ran with network"));
}

fn with_probe(name: &str, field: &str, value: Value) -> Value {
    let mut evidence = good_evidence();
    let probe = evidence["probes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|probe| probe["name"] == name)
        .unwrap();
    probe[field] = value;
    evidence
}

#[test]
fn verify_rejects_probes_that_recorded_the_wrong_expectation() {
    for name in ["ambient_tool", "network", "home_config", "cache_miss"] {
        for (field, value) in [("expect", json!("pass")), ("marker", json!(""))] {
            let flipped = with_probe(name, field, value);
            assert!(verify(&flipped).unwrap_err().contains("recorded expect"));
        }
    }
    let control = with_probe("home_config_control", "expect", json!("fail"));
    assert!(verify(&control).unwrap_err().contains("recorded expect"));
}

#[test]
fn verify_rejects_evidence_for_other_languages() {
    let mut other = good_evidence();
    other["languages"] = json!(["cpp"]);
    assert!(verify(&other).unwrap_err().contains("languages"));
    let mut none = good_evidence();
    none["languages"] = json!(null);
    assert!(verify(&none).unwrap_err().contains("languages"));
    assert!(
        verify_evidence(
            &good_evidence(),
            "abc123",
            &manifest_for("//tools/testing/hermetic:cpp_rust", &[])
        )
        .is_err()
    );
}

#[cfg(unix)]
#[test]
fn ambient_tools_include_versioned_variants() {
    let dir = tempfile::tempdir().unwrap();
    for name in ["gcc-12", "g++-12", "clang-16", "gccx", "tar", "gcc-ar"] {
        std::fs::write(dir.path().join(name), "").unwrap();
    }
    let found = ambient_tools(&[dir.path().to_path_buf()], &["gcc", "g++", "clang", "tar"]);
    let names: Vec<&str> = found.iter().map(|(name, _)| name.as_str()).collect();
    assert_eq!(names, vec!["gcc-12", "g++-12", "clang-16", "tar"]);
}

#[test]
fn verify_rejects_missing_or_inconsistent_artifacts() {
    let mut none = good_evidence();
    none["artifacts"] = json!([]);
    assert!(verify(&none).unwrap_err().contains("no built artifacts"));
    let mut undigested = good_evidence();
    undigested["artifacts"][0]["sha256"] = json!("abc");
    assert!(verify(&undigested).unwrap_err().contains("no sha256"));
    let mut mismatch = good_evidence();
    mismatch["target"]["machines"] = json!(["aarch64"]);
    assert!(verify(&mismatch).unwrap_err().contains("disagree"));
    let mut foreign = good_evidence();
    foreign["artifacts"][0]["machine"] = json!(null);
    foreign["target"]["machines"] = json!([]);
    assert!(
        verify(&foreign)
            .unwrap_err()
            .contains("no native artifact machine")
    );
}

#[test]
fn verify_accepts_machine_free_artifacts_for_managed_languages() {
    let mut beam = good_evidence();
    beam["qualification"] = json!("//tools/testing/hermetic:elixir");
    beam["languages"] = json!(["elixir"]);
    beam["artifacts"] = json!([
        {"path": "bazel-out/k8-fastbuild/bin/app.beam", "sha256": sha256_hex(b"beam"), "machine": null},
    ]);
    beam["target"]["machines"] = json!([]);
    verify_evidence(
        &beam,
        "abc123",
        &manifest_for("//tools/testing/hermetic:elixir", &["elixir"]),
    )
    .unwrap();
}

#[test]
fn verify_accepts_tool_only_msix_evidence() {
    let mut msix = good_evidence();
    msix["qualification"] = json!("//tools/testing/hermetic:msix");
    msix["languages"] = json!(["msix"]);
    msix["artifacts"] = json!([
        {"path": "bazel-out/k8-fastbuild/bin/app/tools_test", "sha256": sha256_hex(b"sh"), "machine": null},
    ]);
    msix["target"]["machines"] = json!([]);
    verify_evidence(
        &msix,
        "abc123",
        &manifest_for("//tools/testing/hermetic:msix", &["msix"]),
    )
    .unwrap();
}

fn compiler_manifest() -> Manifest {
    Manifest {
        compiler: Some(("gcc".to_owned(), "GCC: (GNU) 16.2.0".to_owned())),
        ..cpp_rust()
    }
}

#[test]
fn verify_checks_the_compiler_that_built_each_artifact() {
    let mut ev = good_evidence();
    ev["artifacts"][0]["producers"] = json!(["GCC: (GNU) 15.1.0", "GCC: (GNU) 16.2.0"]);
    verify_evidence(&ev, "abc123", &compiler_manifest()).unwrap();

    ev["artifacts"][0]["producers"] = json!(["clang version 23.1.2"]);
    let err = verify_evidence(&ev, "abc123", &compiler_manifest()).unwrap_err();
    assert!(err.contains("not built by gcc"), "{err}");
    assert!(err.contains("clang version 23.1.2"), "{err}");

    ev["artifacts"][0]["producers"] = json!([]);
    let err = verify_evidence(&ev, "abc123", &compiler_manifest()).unwrap_err();
    assert!(err.contains("unproven"), "{err}");
}

fn appimage_evidence() -> Value {
    let mut ev = good_evidence();
    ev["qualification"] = json!("//tools/testing/hermetic:appimage");
    ev["languages"] = json!(["appimage"]);
    ev["artifacts"] = json!([
        {"path": "bazel-out/k8-fastbuild/bin/app/dxappimage.AppImage", "sha256": sha256_hex(b"img"), "machine": null},
    ]);
    ev["target"]["machines"] = json!([]);
    ev
}

#[test]
fn verify_requires_a_native_machine_for_appimage() {
    let err = verify_evidence(
        &appimage_evidence(),
        "abc123",
        &manifest_for("//tools/testing/hermetic:appimage", &["appimage"]),
    )
    .unwrap_err();
    assert!(err.contains("no native artifact machine"), "{err}");
}

#[test]
fn verify_accepts_appimage_evidence_with_an_elf_machine() {
    let mut ev = appimage_evidence();
    ev["artifacts"][0]["machine"] = json!("x86_64");
    ev["target"]["machines"] = json!(["x86_64"]);
    verify_evidence(
        &ev,
        "abc123",
        &manifest_for("//tools/testing/hermetic:appimage", &["appimage"]),
    )
    .unwrap();
}

fn runfiles_manifest() -> Manifest {
    let rel =
        std::env::var("DX_HERMETIC_MANIFEST").expect("DX_HERMETIC_MANIFEST names the manifest");
    let text =
        std::fs::read_to_string(dx_testing::resolve_runfiles(&rel)).expect("manifest is readable");
    let base = std::env::current_dir().expect("runfiles cwd");
    parse_manifest(&text, &base).expect("manifest parses")
}

#[test]
fn the_cpp_rust_target_names_a_real_consumer() {
    let manifest = runfiles_manifest();
    assert_eq!(manifest.label, "//tools/testing/hermetic:cpp_rust");
    assert_eq!(
        manifest.languages,
        vec!["cpp", "java", "javascript", "kotlin", "rust", "typescript"]
    );
    assert_eq!(manifest.repo_name, "my_dx");
    assert!(
        manifest.workspace_dir.contains(' '),
        "the workspace path keeps a space"
    );
    let dests: Vec<&str> = manifest
        .files
        .iter()
        .map(|(_, dest)| dest.as_str())
        .collect();
    for dest in [
        ".bazelrc",
        "BUILD.bazel",
        "cc/tests/fixtures/adopt-cpp/solo/BUILD.bazel",
        "cc/tests/fixtures/adopt-cpp/solo/pure.cc",
        "cc/tests/fixtures/adopt-cpp/solo/pure.h",
        "cc/tests/fixtures/adopt-cpp/solo/pure_test.cc",
        "rust/tests/fixtures/adopt-rust/solo/BUILD.bazel",
        "rust/tests/fixtures/adopt-rust/solo/src/lib.rs",
        "rust/tests/fixtures/adopt-rust/solo/src/net.rs",
    ] {
        assert!(dests.contains(&dest), "missing consumer file {dest}");
    }
    for (src, dest) in &manifest.files {
        assert!(src.is_file(), "{src:?} for {dest} is not in the runfiles");
        if dest.ends_with("BUILD.bazel") {
            let original = std::fs::read_to_string(src).unwrap();
            let text = consumer_build(&original, "my_dx");
            assert!(
                text.contains("@my_dx//") || !original.contains("@rules_dx//"),
                "{dest} loads through the renamed repository"
            );
            assert!(
                !text.contains("@rules_dx//"),
                "{dest} must not use the default name"
            );
        }
    }
}

#[cfg(unix)]
#[test]
fn the_consumer_materializes_from_dx_init() {
    let manifest = runfiles_manifest();
    let dir = tempfile::tempdir().unwrap();
    let workspace = dir.path().join(&manifest.workspace_dir);
    materialize(&manifest, &workspace, Path::new("/rules dx source"), "cc").unwrap();
    let module = std::fs::read_to_string(workspace.join("MODULE.bazel")).unwrap();
    for needle in [
        "module(name = \"hermetic_consumer\")",
        "bazel_dep(name = \"rules_dx\", repo_name = \"my_dx\", version = ",
        "bazel_dep(name = \"googletest\", version = \"1.18.0\")",
        "path = \"/rules dx source\",",
    ] {
        assert!(
            module.contains(needle),
            "MODULE.bazel lacks {needle}: {module}"
        );
    }
    assert!(
        workspace.join(".dx/pins").is_file(),
        "dx init pins the version"
    );
    assert_eq!(
        std::fs::read(workspace.join(".bazelversion")).unwrap(),
        std::fs::read(&manifest.bazelversion).unwrap()
    );
    for (src, dest) in &manifest.files {
        let original = std::fs::read(src).unwrap();
        let want = if dest.ends_with("BUILD.bazel") {
            consumer_build(&String::from_utf8(original).unwrap(), "my_dx").into_bytes()
        } else {
            original
        };
        let got = std::fs::read(workspace.join(dest)).unwrap();
        if dest == ".bazelrc" || dest == "BUILD.bazel" {
            // The second init adds the preset imports and the exports block around the fixture text.
            let want = String::from_utf8(want).unwrap();
            let got = String::from_utf8(got).unwrap();
            assert!(
                got.contains(want.trim_end()),
                "{dest} keeps its source: {got}"
            );
        } else {
            assert_eq!(got, want, "{dest} is derived from its source");
        }
    }
    assert_eq!(
        std::fs::read_to_string(workspace.join(LEAKS_PACKAGE).join("BUILD.bazel")).unwrap(),
        leaks_build_host("cc")
    );
}

#[cfg(unix)]
#[test]
fn materialize_reports_each_missing_input() {
    let real = runfiles_manifest();
    let dir = tempfile::tempdir().unwrap();
    let source = Path::new("/src");
    let blocked = dir.path().join("file");
    std::fs::write(&blocked, "").unwrap();
    let err = materialize(&real, &blocked.join("ws"), source, "cc").unwrap_err();
    assert!(err.starts_with("cannot create "), "{err}");
    let mut missing = real.clone();
    missing.dx = dir.path().join("no-dx");
    let err = materialize(&missing, &dir.path().join("a"), source, "cc").unwrap_err();
    assert!(err.starts_with("cannot run dx init: "), "{err}");
    let mut failing = real.clone();
    failing.dx = PathBuf::from("/usr/bin/false");
    let err = materialize(&failing, &dir.path().join("b"), source, "cc").unwrap_err();
    assert!(err.starts_with("dx init failed: "), "{err}");
    let mut silent = real.clone();
    silent.dx = PathBuf::from("/usr/bin/true");
    let err = materialize(&silent, &dir.path().join("c"), source, "cc").unwrap_err();
    assert!(err.starts_with("dx init wrote no MODULE.bazel: "), "{err}");
    let mut no_fragment = real;
    no_fragment.module_fragment = Some(dir.path().join("no-fragment"));
    let err = materialize(&no_fragment, &dir.path().join("d"), source, "cc").unwrap_err();
    assert!(err.starts_with("cannot read "), "{err}");
}

struct FakeHost {
    dir: tempfile::TempDir,
    host: HostPaths,
}

fn fake_host() -> FakeHost {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path();
    for sub in ["sbin", "bin", "sdk", "sdk/bin", "kept-sdk"] {
        std::fs::create_dir_all(root.join(sub)).unwrap();
    }
    for file in ["sbin/ip", "bin/cc", "bin/gcc-13", "bin/git", "sdk/bin/node"] {
        std::fs::write(root.join(file), "#!/bin/sh\n").unwrap();
    }
    let host = HostPaths {
        ip_candidates: vec![root.join("missing/ip"), root.join("sbin/ip")],
        sdk_candidates: vec![
            root.join("sdk"),
            root.join("absent-sdk"),
            root.join("kept-sdk"),
        ],
        tool_dirs: vec![root.join("bin"), root.join("sdk/bin")],
    };
    FakeHost { dir, host }
}

#[cfg(target_os = "linux")]
#[test]
fn plan_with_masks_fake_tools_sdks_and_home_config_but_keeps_needed_paths() {
    let fake = fake_host();
    let root = fake.dir.path();
    let home = root.join("home");
    std::fs::create_dir_all(home.join(".cargo")).unwrap();
    std::fs::create_dir_all(home.join(".config")).unwrap();
    std::fs::write(home.join(".netrc"), "machine x\n").unwrap();
    std::fs::write(home.join(".npmrc"), "x\n").unwrap();
    let shim = root.join("shim");
    let keep = vec![
        home.join(".config/dx"),
        home.join(".npmrc"),
        root.join("kept-sdk"),
        root.join("bin/git"),
    ];
    let isolation = Isolation::plan_with(&fake.host, &shim, &keep, Some(&home)).unwrap();
    assert_eq!(isolation.shim, shim);
    assert_eq!(isolation.home_dirs, vec![home.join(".cargo")]);
    assert_eq!(isolation.home_files, vec![home.join(".netrc")]);
    let status = std::fs::read_to_string("/proc/self/status").unwrap();
    assert_eq!(Some(isolation.uid), status_id(&status, "Uid:"));
    assert_eq!(Some(isolation.gid), status_id(&status, "Gid:"));
    assert_eq!(isolation.ip, root.join("sbin/ip"));
    assert_eq!(isolation.sdk_dirs, vec![root.join("sdk")]);
    let real = std::fs::canonicalize(root.join("bin")).unwrap();
    assert_eq!(isolation.tools, vec![real.join("cc"), real.join("gcc-13")]);
}

fn planned_with_home(fake: &FakeHost) -> (Isolation, PathBuf) {
    let root = fake.dir.path();
    let home = root.join("home");
    std::fs::create_dir_all(home.join(".cargo")).unwrap();
    std::fs::write(home.join(".netrc"), "machine x\n").unwrap();
    let isolation = Isolation::plan_with(&fake.host, &root.join("shim"), &[], Some(&home)).unwrap();
    (isolation, home)
}

#[test]
fn windows_denies_drop_grants_their_parents_and_repeats() {
    let fake = fake_host();
    let root = fake.dir.path();
    let (mut isolation, home) = planned_with_home(&fake);
    let planned_tools = isolation.tools.clone();
    isolation.grants = vec![root.join("sdk/bin")];
    isolation.tools.push(root.join("sdk/bin/node"));
    isolation.home_files.push(home.join(".cargo/config.toml"));
    isolation.home_files.push(home.join(".netrc"));
    let denies = isolation.windows_denies();
    let mut expected = vec![
        root.join("kept-sdk"),
        home.join(".cargo"),
        home.join(".netrc"),
    ];
    expected.extend(planned_tools);
    assert_eq!(denies, expected);
    for deny in &denies {
        for grant in &isolation.grants {
            assert!(
                !grant.starts_with(deny) && !deny.starts_with(grant),
                "{deny:?}"
            );
        }
    }
}

#[test]
fn windows_denies_are_empty_when_the_grants_cover_everything() {
    let fake = fake_host();
    let (mut isolation, _) = planned_with_home(&fake);
    assert!(!isolation.windows_denies().is_empty());
    isolation.grants = vec![fake.dir.path().to_path_buf()];
    isolation.grants.extend(isolation.tools.clone());
    assert!(isolation.windows_denies().is_empty());
}

#[cfg(target_os = "linux")]
#[test]
fn plan_with_fails_without_ip() {
    let mut fake = fake_host();
    fake.host.ip_candidates = vec![fake.dir.path().join("none/ip")];
    let err =
        Isolation::plan_with(&fake.host, &fake.dir.path().join("shim"), &[], None).unwrap_err();
    assert_eq!(
        err,
        "namespace isolation needs ip from iproute2 to raise loopback"
    );
}

#[test]
fn plan_with_keep_covering_every_sdk_masks_none() {
    let fake = fake_host();
    let keep = fake.host.sdk_candidates.clone();
    let isolation =
        Isolation::plan_with(&fake.host, &fake.dir.path().join("shim"), &keep, None).unwrap();
    assert!(isolation.sdk_dirs.is_empty());
    assert!(isolation.home_dirs.is_empty() && isolation.home_files.is_empty());
}

#[cfg(unix)]
#[test]
fn host_paths_detect_uses_the_host_constants() {
    let host = HostPaths::detect();
    let strs = |paths: &[PathBuf]| -> Vec<String> {
        paths
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect()
    };
    assert_eq!(strs(&host.ip_candidates), IP_CANDIDATES);
    assert_eq!(strs(&host.tool_dirs), TOOL_DIRS);
    assert_eq!(strs(&host.sdk_candidates)[..SDK_DIRS.len()], *SDK_DIRS);
}

#[cfg(unix)]
#[test]
fn capture_keeps_exit_and_both_streams() {
    let mut command = Command::new("/bin/sh");
    command.args(["-c", "printf out; printf err >&2; exit 3"]);
    let outcome = capture(command).unwrap();
    assert_eq!(
        (
            outcome.exit,
            outcome.stdout.as_str(),
            outcome.stderr.as_str()
        ),
        (3, "out", "err")
    );
    let err = capture(Command::new("/no/such/program")).unwrap_err();
    assert!(err.starts_with("cannot start "), "{err}");
}

fn fixture_session() -> Session {
    Session {
        manifest: parse_manifest(
            &json!({
                "label": "//x:y",
                "languages": ["cpp"],
                "module": "m",
                "repo_name": "r",
                                "workspace_dir": "ws",
                "patterns": ["//..."],
                "failures": {},
                "coverage": [],
                "covered": [],
                "files": [],
                "module_fragment": "f",
                "dx": "d",
                "module_file": "MODULE.bazel",
                "bazelversion": ".bazelversion",
            })
            .to_string(),
            Path::new("/"),
        )
        .unwrap(),
        root: PathBuf::from("/q"),
        workspace: PathBuf::from("/q/consumer ws"),
        out: PathBuf::from("/out"),
        source: PathBuf::from("/src"),
        bazelisk: PathBuf::from("/q/bin/bazelisk"),
        bazel: PathBuf::from("/q/bin/bazelisk"),
        bazelisk_home: PathBuf::from("/q/bazelisk"),
        isolation: Isolation::default(),
        probes: Vec::new(),
        bash: Some(PathBuf::from("/msys/bash.exe")),
    }
}

fn fixture_probe() -> Probe<'static> {
    Probe {
        name: "hermetic_test".to_owned(),
        net: Net::Denied,
        home: Path::new("/q/home"),
        output_root: Path::new("/q/cold"),
        repos: Path::new("/q/repos"),
        command: "test",
        targets: vec!["//...".to_owned()],
        extra: vec!["--test_output=errors".to_owned()],
        expect: Expect::Pass,
    }
}

#[test]
fn hermetic_test_builds_generate_check_only_with_coverage_targets() {
    let mut session = fixture_session();
    assert_eq!(session.hermetic_test_targets(), ["//..."]);
    session.manifest.coverage = vec!["//lib:t".to_owned()];
    let label = format!("@{}//dx:generate_check", session.manifest.repo_name);
    assert_eq!(
        session.hermetic_test_targets(),
        ["//...".to_owned(), label.clone()]
    );
    let probe = Probe {
        targets: session.hermetic_test_targets(),
        ..fixture_probe()
    };
    let argv = session.bazel_tail(&probe);
    for setting in dx_process::head_settings("r") {
        assert!(argv.contains(&setting), "{setting} missing from {argv:?}");
    }
    assert_eq!(argv.last(), Some(&label));
}

#[cfg(unix)]
#[test]
fn each_output_root_has_its_own_dx_disk_cache() {
    let session = fixture_session();
    let cache = |root: &str, command: &'static str| {
        let probe = Probe {
            command,
            output_root: Path::new(root),
            ..fixture_probe()
        };
        session
            .bazel_tail(&probe)
            .into_iter()
            .filter(|arg| arg.starts_with("--disk_cache="))
            .collect::<Vec<_>>()
    };
    assert_eq!(
        cache("/q/warm", "coverage"),
        ["--disk_cache=/q/warm_cache/dx/disk-cache"]
    );
    assert!(cache("/q/cold", "cquery").is_empty());
}

#[test]
fn probes_carry_the_dx_settings_only_on_build_like_verbs() {
    let session = fixture_session();
    let settings = dx_process::head_settings("r");
    for (command, carried) in [
        ("cquery", true),
        ("build", true),
        ("query", false),
        ("fetch", false),
    ] {
        let probe = Probe {
            command,
            ..fixture_probe()
        };
        let argv = session.bazel_tail(&probe);
        for setting in &settings {
            assert_eq!(argv.contains(setting), carried, "{command}: {argv:?}");
        }
    }
}

#[test]
fn qualification_root_stays_under_tmp_off_windows() {
    let tmp = Path::new("/tmp/t");
    assert_eq!(
        qualification_root_for(tmp, false),
        tmp.join("qualification")
    );
}

#[test]
fn qualification_root_on_windows_is_short_and_stable() {
    let tmp = Path::new(r"D:\_bazel\execroot\_main\_tmp\0123456789abcdef0123456789abcdef");
    let root = qualification_root_for(tmp, true);
    let text = root.to_string_lossy().into_owned();
    assert!(text.starts_with(r"D:\q\"), "{text}");
    assert_eq!(text.len(), r"D:\q\".len() + 6);
    assert_eq!(qualification_root_for(tmp, true), root);
    assert_ne!(qualification_root_for(Path::new(r"D:\other"), true), root);
    let relative = Path::new("tmp");
    assert_eq!(
        qualification_root_for(relative, true),
        relative.join("qualification")
    );
}

#[test]
fn longest_npm_package_dir_fits_the_windows_chdir_limit() {
    let tmp = Path::new(r"D:\_bazel\execroot\_main\_tmp\0123456789abcdef0123456789abcdef");
    // The longest store entry in the node-app lock, with the widest peer hash rules_js can write.
    let package = "bazel-out/x64_windows-fastbuild/bin/javascript/tests/fixtures/node-app/node_modules/.aspect_rules_js/@babel+plugin-syntax-logical-assignment-operators@_-2147483648/node_modules/@babel/plugin-syntax-logical-assignment-operators";
    assert_eq!(package.len(), 226);
    let output_base = startup_flags_for(&qualification_root_for(tmp, true).join("cold"), true)
        .iter()
        .find_map(|flag| flag.strip_prefix("--output_base=").map(str::to_owned))
        .expect("windows pins the output base");
    let dir = format!(r"{output_base}\execroot\_main\{package}");
    assert!(dir.len() <= 258, "{} characters: {dir}", dir.len());
}

#[test]
fn startup_flags_pin_the_output_base_only_on_windows() {
    assert_eq!(
        startup_flags_for(Path::new(r"D:\q\abc\cold"), true),
        [
            "--nosystem_rc",
            r"--output_user_root=D:\q\abc\cold",
            r"--output_base=D:\q\abc\c",
        ]
    );
    assert_eq!(
        startup_flags_for(Path::new("/q/cold"), false),
        ["--nosystem_rc", "--output_user_root=/q/cold"]
    );
}

#[test]
fn dx_startup_options_carry_the_windows_roots() {
    let options = dx_startup_options(Path::new(r"D:\q\abc\cold"), true);
    assert_eq!(
        options,
        [
            r"--bazel-startup-option=--output_user_root=D:\q\abc\cold",
            r"--bazel-startup-option=--output_base=D:\q\abc\c",
        ]
    );
    for option in &options {
        let token = option.strip_prefix("--bazel-startup-option=").unwrap();
        assert_eq!(
            dx_process::validate_startup_option(token).as_deref(),
            Ok(token)
        );
    }
    assert!(dx_startup_options(Path::new("/q/cold"), false).is_empty());
}

#[test]
fn dx_windows_path_puts_bash_ahead_of_system32() {
    let bin = Path::new(r"D:\q\abc\bin");
    let bash = Path::new("C:/msys64/usr/bin/bash.exe");
    assert_eq!(
        dx_windows_path(bin, Some(bash), r"C:\Windows\System32;C:\Windows"),
        r"D:\q\abc\bin;C:/msys64/usr/bin;C:\Windows\System32;C:\Windows"
    );
    assert_eq!(
        dx_windows_path(bin, None, r"C:\Windows\System32"),
        r"D:\q\abc\bin;C:\Windows\System32"
    );
}

#[test]
fn detected_windows_sh_reads_the_rules_shell_build() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(detected_windows_sh(dir.path()), None);
    let repo = dir
        .path()
        .join("external")
        .join("rules_shell++sh_configure+local_config_shell");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(
        repo.join("BUILD"),
        "sh_toolchain(\n    name = \"linux_sh\",\n    path = \"/bin/bash\",\n)\n\
         sh_toolchain(\n    name = \"windows_sh\",\n    path = \"C:/Windows/System32/bash.exe\",\n)\n",
    )
    .unwrap();
    assert_eq!(
        detected_windows_sh(dir.path()).as_deref(),
        Some("C:/Windows/System32/bash.exe")
    );
}

#[test]
fn check_windows_sh_rejects_the_wsl_stub_and_an_empty_path() {
    let root = r"C:\Windows";
    assert!(check_windows_sh("C:/msys64/usr/bin/bash.exe", root).is_ok());
    let stub = check_windows_sh("c:/windows/system32/bash.exe", root).unwrap_err();
    assert!(stub.contains("WSL stub"), "{stub}");
    assert!(check_windows_sh("", root).is_err());
}

#[cfg(unix)]
#[test]
fn bazel_argv_pins_startup_and_build_flags_before_the_targets() {
    let argv = fixture_session().bazel_argv(&fixture_probe());
    assert_eq!(
        argv[..6],
        [
            "/usr/bin/env",
            "-i",
            "HOME=/q/home",
            "PATH=/usr/bin:/bin",
            "BAZELISK_HOME=/q/bazelisk",
            "XDG_CACHE_HOME=/q/cold_cache",
        ]
    );
    let gc_size = format!(
        "--experimental_disk_cache_gc_max_size={}",
        dx_process::DISK_CACHE_GC_MAX_SIZE
    );
    let gc_age = format!(
        "--experimental_disk_cache_gc_max_age={}",
        dx_process::DISK_CACHE_GC_MAX_AGE
    );
    assert_eq!(
        argv[9..],
        [
            "/q/consumer ws",
            "/q/bin/bazelisk",
            "--nosystem_rc",
            "--output_user_root=/q/cold",
            "--max_idle_secs=60",
            "test",
            "--repository_cache=/q/repos",
            "--noshow_progress",
            "--color=no",
            "--curses=no",
            "--incompatible_strict_action_env",
            "--sandbox_default_allow_network=false",
            "--@r//config:clippy_output_diagnostics=true",
            "--@r//config:rustc_output_diagnostics=true",
            "--disk_cache=/q/cold_cache/dx/disk-cache",
            &gc_size,
            &gc_age,
            "--test_output=errors",
            "--",
            "//...",
        ]
    );
    assert!(argv[8].contains("shutdown"), "{}", argv[8]);
}

#[test]
fn probes_run_the_resolved_bazel_when_it_differs_from_bazelisk() {
    let mut session = fixture_session();
    session.bazel = PathBuf::from("/q/bazelisk/downloads/bazel.exe");
    assert_eq!(
        session.bazel_tail(&fixture_probe())[0],
        "/q/bazelisk/downloads/bazel.exe"
    );
}

#[test]
fn bazel_real_prefers_the_wrapper_line_then_the_first_path_entry() {
    assert_eq!(
        bazel_real("BAZELISK_SKIP_WRAPPER=true\r\nBAZEL_REAL=C:\\q\\bazel.exe\r\n").unwrap(),
        PathBuf::from("C:\\q\\bazel.exe")
    );
    assert_eq!(
        bazel_real("Path=/q/bazelisk/bin;C:\\Windows\r\nBAZELISK_SKIP_WRAPPER=true\r\n").unwrap(),
        Path::new("/q/bazelisk/bin").join(format!("bazel{}", std::env::consts::EXE_SUFFIX))
    );
    for printed in [
        "BAZELISK_SKIP_WRAPPER=true\n",
        "BAZEL_REAL=\nPATH=\n",
        "PATH=;C:\\Windows\n",
    ] {
        assert_eq!(
            bazel_real(printed).unwrap_err(),
            "bazelisk --print_env printed neither BAZEL_REAL nor PATH"
        );
    }
}

fn fake_pe(characteristics: u16) -> Vec<u8> {
    let mut head = b"MZ".to_vec();
    head.resize(0x3c, 0);
    head.extend_from_slice(&0x40u32.to_le_bytes());
    head.extend_from_slice(b"PE\0\0");
    head.resize(0x40 + 22, 0);
    head.extend_from_slice(&characteristics.to_le_bytes());
    head
}

#[test]
fn relocatable_rejects_only_a_pe_with_stripped_relocations() {
    assert!(!relocatable(&fake_pe(0x23)));
    assert!(relocatable(&fake_pe(0x22)));
    assert!(relocatable(&fake_elf()));
    assert!(relocatable(b"MZ"));
    assert!(relocatable(&fake_pe(0x23)[..0x40 + 22]));
}

#[test]
fn read_head_reads_at_most_a_page() {
    let dir = tempfile::tempdir().unwrap();
    let file = dir.path().join("bazel.exe");
    std::fs::write(&file, vec![0u8; 5000]).unwrap();
    assert_eq!(read_head(&file).unwrap().len(), 4096);
    let missing = dir.path().join("missing.exe");
    assert!(
        read_head(&missing)
            .unwrap_err()
            .starts_with(&format!("cannot read {}: ", missing.display()))
    );
}

#[test]
fn git_reports_clean_dirty_and_failing_trees() {
    let dir = tempfile::tempdir().unwrap();
    git(dir.path(), &["init", "-q"]).unwrap();
    assert_eq!(git(dir.path(), &["status", "--porcelain"]).unwrap(), "");
    std::fs::write(dir.path().join("new"), "").unwrap();
    assert_eq!(
        git(dir.path(), &["status", "--porcelain"]).unwrap(),
        "?? new"
    );
    let err = git(dir.path(), &["rev-parse", "--verify", "HEAD"]).unwrap_err();
    assert!(
        err.starts_with("git [\"rev-parse\", \"--verify\", \"HEAD\"] failed: "),
        "{err}"
    );
}

fn fake_elf() -> Vec<u8> {
    let mut head = vec![0x7f, b'E', b'L', b'F', 2, 1, 1, 0];
    head.resize(18, 0);
    head.extend_from_slice(&[0x3e, 0x00]);
    head
}

#[cfg(unix)]
#[test]
fn bazelisk_comes_from_the_override_then_path() {
    let dir = tempfile::tempdir().unwrap();
    let first = dir.path().join("first");
    let second = dir.path().join("second");
    std::fs::create_dir_all(&first).unwrap();
    std::fs::create_dir_all(second.join("bazelisk")).unwrap();
    std::fs::write(second.join("bazel"), fake_elf()).unwrap();
    let path = format!("{}::{}", first.display(), second.display());
    let chosen = dir.path().join("chosen");
    std::fs::write(&chosen, fake_elf()).unwrap();
    assert_eq!(
        find_bazelisk(Some(&chosen.to_string_lossy()), &path).unwrap(),
        chosen
    );
    assert_eq!(
        find_bazelisk(Some(""), &path).unwrap(),
        second.join("bazel")
    );
    std::fs::write(first.join("bazelisk"), fake_elf()).unwrap();
    assert_eq!(find_bazelisk(None, &path).unwrap(), first.join("bazelisk"));
    let err = find_bazelisk(None, "").unwrap_err();
    assert_eq!(
        err,
        "no native bazelisk: set DX_HERMETIC_BAZELISK or put bazelisk on PATH"
    );
}

#[cfg(unix)]
#[test]
fn bazelisk_refuses_wrapper_scripts() {
    let dir = tempfile::tempdir().unwrap();
    let wrapper = dir.path().join("bazel");
    std::fs::write(&wrapper, "#!/usr/bin/env bash\nexec bazelisk \"$@\"\n").unwrap();
    let err = find_bazelisk(Some(&wrapper.to_string_lossy()), "").unwrap_err();
    assert_eq!(
        err,
        format!(
            "DX_HERMETIC_BAZELISK {} is not a native bazelisk binary; a wrapper script would run outside the isolation",
            wrapper.display()
        )
    );
    let path = dir.path().to_string_lossy().into_owned();
    assert!(find_bazelisk(None, &path).is_err());
    let mach_o = dir.path().join("mach-o");
    std::fs::write(
        &mach_o,
        [
            0xcf, 0xfa, 0xed, 0xfe, 12, 0, 0, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ],
    )
    .unwrap();
    assert_eq!(
        find_bazelisk(Some(&mach_o.to_string_lossy()), "").unwrap(),
        mach_o
    );
    let missing = dir.path().join("missing");
    assert!(find_bazelisk(Some(&missing.to_string_lossy()), "").is_err());
    let real = dir.path().join("real").join("bazel");
    std::fs::create_dir_all(real.parent().unwrap()).unwrap();
    std::fs::write(&real, fake_elf()).unwrap();
    let both = format!("{}:{}", path, real.parent().unwrap().display());
    assert_eq!(find_bazelisk(None, &both).unwrap(), real);
}

#[cfg(unix)]
#[test]
fn file_helpers_create_parents_and_name_the_failing_path() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let target = dir.path().join("a/b/c.txt");
    write(&target, "hi").unwrap();
    copy(&target, &dir.path().join("d/e.txt")).unwrap();
    assert_eq!(
        std::fs::read_to_string(dir.path().join("d/e.txt")).unwrap(),
        "hi"
    );
    executable(&target).unwrap();
    assert_eq!(
        target.metadata().unwrap().permissions().mode() & 0o777,
        0o755
    );
    let err = write(&target.join("x"), "").unwrap_err();
    assert!(err.starts_with("cannot create "), "{err}");
    let err = write(&dir.path().join("a"), "").unwrap_err();
    assert!(err.starts_with("cannot write "), "{err}");
    let err = copy(&dir.path().join("none"), &dir.path().join("f")).unwrap_err();
    assert!(err.starts_with("cannot copy "), "{err}");
    let err = copy(&target, &target.join("x")).unwrap_err();
    assert!(err.starts_with("cannot create "), "{err}");
    let err = write(Path::new("/"), "").unwrap_err();
    assert!(err.starts_with("cannot write /: "), "{err}");
    let err = copy(&target, Path::new("/")).unwrap_err();
    assert!(err.starts_with("cannot copy "), "{err}");
    let err = executable(&dir.path().join("none")).unwrap_err();
    assert!(err.starts_with("cannot mark "), "{err}");
}

#[cfg(unix)]
#[test]
fn remove_root_clears_read_only_trees_without_following_links() {
    use std::os::unix::fs::PermissionsExt;
    let dir = tempfile::tempdir().unwrap();
    let outside = dir.path().join("outside");
    std::fs::create_dir_all(&outside).unwrap();
    std::fs::write(outside.join("keep"), "").unwrap();
    std::fs::set_permissions(&outside, std::fs::Permissions::from_mode(0o500)).unwrap();
    let root = dir.path().join("root");
    std::fs::create_dir_all(root.join("ro/inner")).unwrap();
    std::fs::write(root.join("ro/inner/file"), "").unwrap();
    std::os::unix::fs::symlink(&outside, root.join("ro/link")).unwrap();
    for path in [root.join("ro/inner"), root.join("ro")] {
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o500)).unwrap();
    }
    remove_root(&root);
    assert!(!root.exists());
    assert_eq!(
        outside.metadata().unwrap().permissions().mode() & 0o777,
        0o500
    );
    remove_root(&dir.path().join("never made"));
    std::fs::set_permissions(&outside, std::fs::Permissions::from_mode(0o700)).unwrap();
}

#[cfg(unix)]
#[test]
fn host_names_this_machine() {
    let host = host();
    assert_eq!(host["os"], json!(std::env::consts::OS));
    assert_eq!(host["arch"], json!(std::env::consts::ARCH));
    assert!(!host["kernel"].as_str().unwrap().is_empty());
}

#[test]
fn consumer_build_keeps_targets_and_drops_repository_only_statements() {
    let original = "load(\"@rules_dx//cc/rules:defs.bzl\", \"cc_test\")\nload(\"//quality:fixtures.bzl\", \"real_source_target\")\n\npackage(default_visibility = [\"//:__subpackages__\"])\n\nexports_files(\n    [\"a.cc\"],\n    visibility = [\"//x:__pkg__\"],\n)\n\ncc_test(\n    name = \"t\",\n    srcs = [\"a.cc\"],\n    visibility = [\"//:__subpackages__\"],\n)\n\nreal_source_target(\n    name = \"corpus_starlark\",\n    starlark_srcs = [\"BUILD.bazel\"],\n)\n";
    assert_eq!(
        consumer_build(original, "my_dx"),
        "load(\"@my_dx//cc/rules:defs.bzl\", \"cc_test\")\n\ncc_test(\n    name = \"t\",\n    srcs = [\"a.cc\"],\n)\n"
    );
}

#[test]
fn consumer_build_keeps_a_public_default_visibility() {
    let original = "_PUBLIC = [\"//visibility:public\"]\n\npackage(default_visibility = _PUBLIC)\n";
    assert_eq!(consumer_build(original, "my_dx"), original);
}

fn probe_manifest_json() -> Value {
    json!({
        "label": "//x:y",
        "languages": ["java"],
        "module": "m",
        "repo_name": "r",
        "workspace_dir": "ws",
        "patterns": ["//..."],
        "failures": {
            "//app/cli:cli_indirect": "[strict] Using type billing.",
            "//app/lib:lib_without_gson": "package com.google.gson does not exist",
        },
        "coverage": ["//app/..."],
        "covered": ["app/lib/Lib.java"],
        "files": [{"src": "Lib.java", "dest": "app/lib/Lib.java"}],
        "module_fragment": "f",
        "dx": "d",
        "module_file": "MODULE.bazel",
        "bazelversion": ".bazelversion",
    })
}

fn probe_manifest() -> Manifest {
    parse_manifest(&probe_manifest_json().to_string(), Path::new("/")).unwrap()
}

#[test]
fn manifest_parses_failures_and_coverage() {
    let manifest = probe_manifest();
    assert_eq!(
        manifest.failures,
        vec![
            (
                "//app/cli:cli_indirect".to_owned(),
                "[strict] Using type billing.".to_owned()
            ),
            (
                "//app/lib:lib_without_gson".to_owned(),
                "package com.google.gson does not exist".to_owned()
            ),
        ]
    );
    assert_eq!(manifest.coverage, vec!["//app/..."]);
    assert_eq!(manifest.covered, vec!["app/lib/Lib.java"]);
}

#[test]
fn manifest_generate_defaults_off_and_leads_the_required_probes() {
    let mut json = probe_manifest_json();
    let off = parse_manifest(&json.to_string(), Path::new("/")).unwrap();
    assert!(!off.generate);
    assert_eq!(required_probes(&off, "linux")[0].0, "fetch");
    json["generate"] = json!(true);
    let on = parse_manifest(&json.to_string(), Path::new("/")).unwrap();
    assert!(on.generate);
    let required = required_probes(&on, "linux");
    assert_eq!(required[0].0, "dx_generate");
    assert_eq!(required[0].3, Net::Allowed);
    assert_eq!(required[1].0, "fetch");
}

#[test]
fn manifest_rejects_bad_failures_and_coverage() {
    let cases: Vec<(&str, Value, &str)> = vec![
        ("failures", json!({"app:x": "m"}), "must be a //label"),
        ("failures", json!({"//app:x": ""}), "non-empty marker"),
        ("failures", json!({"//app:x": 3}), "non-empty marker"),
        ("failures", json!([]), "failures must be a map"),
        (
            "failures",
            json!({"//app:x.y": "m", "//app:x_y": "m"}),
            "share the probe name fails___app_x_y",
        ),
        ("coverage", json!(["-//app/..."]), "positive //pattern"),
        ("coverage", json!(["app/..."]), "positive //pattern"),
        ("coverage", json!([]), "set together"),
        ("covered", json!([]), "set together"),
        ("covered", json!(["../Lib.java"]), "inside the workspace"),
        ("covered", json!(["app/Other.java"]), "not a consumer file"),
    ];
    for (key, bad, want) in cases {
        let mut text = probe_manifest_json();
        text[key] = bad;
        let err = parse_manifest(&text.to_string(), Path::new("/")).unwrap_err();
        assert!(err.contains(want), "{key}: {err}");
    }
    let mut missing = probe_manifest_json();
    missing.as_object_mut().unwrap().remove("covered");
    let err = parse_manifest(&missing.to_string(), Path::new("/")).unwrap_err();
    assert_eq!(err, "manifest field covered must be a list");
}

#[test]
fn failure_probe_names_are_sanitized() {
    assert_eq!(
        failure_probe_name("//examples/java-app/cli:cli_indirect"),
        "fails___examples_java-app_cli_cli_indirect"
    );
    assert_eq!(failure_probe_name("//a b:c.d"), "fails___a_b_c_d");
}

#[test]
fn owned_prefixes_map_patterns_to_packages() {
    let patterns: Vec<String> = ["//a/b/...", "//a/c:t", "//d", "//...", "//:all"]
        .map(str::to_owned)
        .to_vec();
    assert_eq!(
        owned_prefixes(&patterns),
        vec!["a/b/", "a/c/", "d/", "", ""]
    );
}

const OWNED_LCOV: &str = "SF:app/lib/Lib.java\nDA:1,1\nDA:2,0\nend_of_record\nSF:bazel-out/k8-fastbuild/bin/app/lib/BuildInfo.java\nDA:3,0\nend_of_record\n";

#[test]
fn coverage_sources_count_found_and_hit_lines() {
    assert_eq!(
        coverage_sources(OWNED_LCOV).unwrap(),
        vec![
            CoveredSource {
                path: "app/lib/Lib.java".to_owned(),
                lines_found: 2,
                lines_hit: 1,
            },
            CoveredSource {
                path: "bazel-out/k8-fastbuild/bin/app/lib/BuildInfo.java".to_owned(),
                lines_found: 1,
                lines_hit: 0,
            },
        ]
    );
    assert!(coverage_sources("DA:1,1\n").is_err());
}

#[test]
fn coverage_accepts_owned_sources_and_generated_files() {
    let manifest = probe_manifest();
    check_coverage(&coverage_sources(OWNED_LCOV).unwrap(), &manifest).unwrap();
}

#[test]
fn coverage_rejects_foreign_missing_or_unhit_sources() {
    let manifest = probe_manifest();
    let check = |extra: &str| {
        check_coverage(
            &coverage_sources(&format!("{OWNED_LCOV}{extra}")).unwrap(),
            &manifest,
        )
        .unwrap_err()
    };
    assert!(check("SF:app/lib/gson.jar\nDA:1,1\nend_of_record\n").contains("non-source"));
    assert!(check("SF:app/lib/Lib.class\nDA:1,1\nend_of_record\n").contains("non-source"));
    assert!(check("SF:external/gson/Gson.java\nDA:1,1\nend_of_record\n").contains("non-source"));
    assert!(check("SF:/abs/Lib.java\nDA:1,1\nend_of_record\n").contains("non-source"));
    assert!(check("SF:other/Lib.java\nDA:1,1\nend_of_record\n").contains("outside"));
    let unhit = "SF:app/lib/Lib.java\nDA:1,0\nend_of_record\n";
    let err = check_coverage(&coverage_sources(unhit).unwrap(), &manifest).unwrap_err();
    assert!(err.contains("no hit lines in app/lib/Lib.java"), "{err}");
    let other = "SF:app/lib/Other.java\nDA:1,1\nend_of_record\n";
    let err = check_coverage(&coverage_sources(other).unwrap(), &manifest).unwrap_err();
    assert!(err.contains("misses app/lib/Lib.java"), "{err}");
    let err = check_coverage(&[], &manifest).unwrap_err();
    assert!(err.contains("no source files"), "{err}");
}

fn probe_evidence(manifest: &Manifest) -> Value {
    let mut evidence = good_evidence();
    evidence["qualification"] = json!(manifest.label);
    evidence["languages"] = json!(manifest.languages);
    let probes = evidence["probes"].as_array_mut().unwrap();
    probes.push(probe_record("fetch_coverage", "pass", "", "allowed", &[]));
    probes.push(probe_record("coverage", "pass", "", "denied", &[]));
    for name in DX_PROBES {
        probes.push(probe_record(name, "pass", "", "denied", &[]));
    }
    for (label, marker) in &manifest.failures {
        probes.push(probe_record(
            &failure_probe_name(label),
            "fail",
            marker,
            "denied",
            &["build", "--", label],
        ));
    }
    evidence["coverage"] = json!({
        "report": "coverage.lcov",
        "sources": [{"path": "app/lib/Lib.java", "lines_found": 2, "lines_hit": 1}],
    });
    evidence
}

fn without_probe(evidence: &Value, name: &str) -> Value {
    let mut out = evidence.clone();
    out["probes"]
        .as_array_mut()
        .unwrap()
        .retain(|probe| probe["name"] != name);
    out
}

fn probe_mut<'a>(evidence: &'a mut Value, name: &str) -> &'a mut Value {
    evidence["probes"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|probe| probe["name"] == name)
        .unwrap()
}

#[test]
fn verify_accepts_complete_failure_and_coverage_evidence() {
    let manifest = probe_manifest();
    verify_evidence(&probe_evidence(&manifest), "abc123", &manifest).unwrap();
}

#[test]
fn verify_requires_every_failure_probe() {
    let manifest = probe_manifest();
    let evidence = probe_evidence(&manifest);
    let name = "fails___app_cli_cli_indirect";
    let err = verify_evidence(&without_probe(&evidence, name), "abc123", &manifest).unwrap_err();
    assert!(
        err.contains(&format!("missing required probe {name}")),
        "{err}"
    );
    let mut wrong = evidence.clone();
    probe_mut(&mut wrong, name)["marker"] = json!("cannot find symbol");
    let err = verify_evidence(&wrong, "abc123", &manifest).unwrap_err();
    assert!(err.contains("recorded expect"), "{err}");
    let mut online = evidence.clone();
    probe_mut(&mut online, name)["network"] = json!("allowed");
    let err = verify_evidence(&online, "abc123", &manifest).unwrap_err();
    assert!(err.contains("ran with network"), "{err}");
    let mut elsewhere = evidence;
    probe_mut(&mut elsewhere, name)["argv"] = json!(["build", "--", "//app:other"]);
    let err = verify_evidence(&elsewhere, "abc123", &manifest).unwrap_err();
    assert!(
        err.contains("did not build //app/cli:cli_indirect"),
        "{err}"
    );
}

#[test]
fn verify_requires_coverage_probes_and_owned_sources() {
    let manifest = probe_manifest();
    let evidence = probe_evidence(&manifest);
    for name in [
        "coverage",
        "fetch_coverage",
        "dx_generate_check",
        "dx_coverage",
    ] {
        let err =
            verify_evidence(&without_probe(&evidence, name), "abc123", &manifest).unwrap_err();
        assert!(
            err.contains(&format!("missing required probe {name}")),
            "{err}"
        );
    }
    let mut online = evidence.clone();
    probe_mut(&mut online, "dx_coverage")["network"] = json!("allowed");
    let err = verify_evidence(&online, "abc123", &manifest).unwrap_err();
    assert!(err.contains("ran with network"), "{err}");
    let mut jar = evidence.clone();
    jar["coverage"]["sources"]
        .as_array_mut()
        .unwrap()
        .push(json!({"path": "app/lib/gson.jar", "lines_found": 1, "lines_hit": 1}));
    let err = verify_evidence(&jar, "abc123", &manifest).unwrap_err();
    assert!(err.contains("non-source file app/lib/gson.jar"), "{err}");
    let mut unrecorded = evidence;
    unrecorded["coverage"] = json!(null);
    let err = verify_evidence(&unrecorded, "abc123", &manifest).unwrap_err();
    assert!(err.contains("no coverage sources"), "{err}");
}

#[test]
fn verify_keeps_the_base_probe_set_without_failures_or_coverage() {
    let manifest = cpp_rust();
    assert!(manifest.failures.is_empty() && manifest.coverage.is_empty());
    let names: Vec<String> = required_probes(&manifest, "linux")
        .into_iter()
        .map(|(name, ..)| name)
        .collect();
    assert_eq!(
        names,
        REQUIRED_PROBES
            .iter()
            .map(|name| (*name).to_owned())
            .collect::<Vec<_>>()
    );
    verify(&good_evidence()).unwrap();
}

#[test]
fn a_manifest_needs_a_fragment() {
    let mut text = json!({
        "label": "//tools/testing/hermetic:c_fixture",
        "languages": ["c"],
        "module": "m",
        "repo_name": "rules_dx",
        "workspace_dir": "ws",
        "patterns": ["//..."],
        "failures": {},
        "coverage": [],
        "covered": [],
        "files": [],
        "module_fragment": "",
        "dx": "d",
        "module_file": "MODULE.bazel",
        "bazelversion": ".bazelversion",
    });
    let err = parse_manifest(&text.to_string(), Path::new("/")).unwrap_err();
    assert!(err.contains("module_fragment"), "{err}");
    text["module_fragment"] = json!("f");
    let manifest = parse_manifest(&text.to_string(), Path::new("/")).unwrap();
    assert_eq!(manifest.module_fragment, Some(PathBuf::from("/f")));
}

fn c_fixture() -> Manifest {
    manifest_for("//tools/testing/hermetic:c_fixture", &["c"])
}

fn c_fixture_evidence() -> Value {
    let manifest = c_fixture();
    let mut evidence = good_evidence();
    evidence["qualification"] = json!(manifest.label);
    evidence["languages"] = json!(["c"]);
    evidence
}

#[test]
fn c_evidence_needs_a_native_artifact_machine() {
    let manifest = c_fixture();
    verify_evidence(&c_fixture_evidence(), "abc123", &manifest).unwrap();
    let mut foreign = c_fixture_evidence();
    foreign["artifacts"][0]["machine"] = json!(null);
    foreign["target"]["machines"] = json!([]);
    assert!(
        verify_evidence(&foreign, "abc123", &manifest)
            .unwrap_err()
            .contains("no native artifact machine")
    );
}

#[test]
fn macos_profile_denies_network_except_loopback() {
    let isolation = Isolation::default();
    assert_eq!(
        isolation.macos_profile(Net::Allowed),
        "(version 1)(allow default)"
    );
    let denied = isolation.macos_profile(Net::Denied);
    assert!(denied.contains("(deny network*)"));
    assert!(denied.contains("localhost:*"));
    let command = isolation.macos_command(Net::Denied, &["/bin/true".to_owned()]);
    assert_eq!(command.get_program(), "/usr/bin/sandbox-exec");
    let args: Vec<_> = command
        .get_args()
        .map(|a| a.to_string_lossy().into_owned())
        .collect();
    assert_eq!(args[0], "-p");
    assert_eq!(args[2..], ["--", "/bin/true"]);
}

#[test]
fn macos_profile_hides_planned_paths_even_with_network_allowed() {
    let isolation = Isolation {
        sdk_dirs: vec![PathBuf::from("/nonexistent/sdk")],
        home_dirs: vec![PathBuf::from("/nonexistent/h\"ome")],
        home_files: vec![PathBuf::from("/nonexistent/.netrc")],
        tools: vec![PathBuf::from("/nonexistent/bin/cc")],
        ..Isolation::default()
    };
    let profile = isolation.macos_profile(Net::Allowed);
    assert!(!profile.contains("deny network"));
    assert!(profile.contains("(deny file-read* (subpath \"/nonexistent/sdk\"))"));
    assert!(profile.contains("(deny file-read* (subpath \"/nonexistent/h\\\"ome\"))"));
    assert!(profile.contains("(deny file-read* (literal \"/nonexistent/.netrc\"))"));
    assert!(profile.contains("(deny process-exec (literal \"/nonexistent/bin/cc\"))"));
}

#[test]
fn windows_command_runs_the_launcher_with_grants_and_network_flag() {
    let isolation = Isolation {
        launcher: PathBuf::from("appcontainer_run.exe"),
        grants: vec![PathBuf::from("C:\\q\\work"), PathBuf::from("C:\\tools")],
        denies: vec![PathBuf::from("C:\\q\\secret")],
        ..Isolation::default()
    };
    let inner = ["cmd.exe".to_owned(), "/c".to_owned()];
    let args = |net| -> Vec<String> {
        let command = isolation.windows_command(net, &inner);
        assert_eq!(command.get_program(), "appcontainer_run.exe");
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    };
    let denied = args(Net::Denied);
    assert_eq!(denied[0], "--name");
    assert!(denied[1].starts_with("dx-hermetic-"));
    assert_eq!(
        denied[2..],
        [
            "--backend",
            "user",
            "--grant",
            "C:\\q\\work",
            "--grant",
            "C:\\tools",
            "--deny",
            "C:\\q\\secret",
            "--",
            "cmd.exe",
            "/c"
        ]
    );
    let allowed = args(Net::Allowed);
    assert_eq!(allowed[4], "--net");
    assert_ne!(allowed[1], denied[1]);
}

#[test]
fn windows_runs_in_a_group_leave_the_acls_to_the_groups() {
    let isolation = Isolation {
        launcher: PathBuf::from("appcontainer_run.exe"),
        grants: vec![PathBuf::from(r"\\?\C:\q\work")],
        denies: vec![PathBuf::from("C:\\q\\secret")],
        group: Some("dxh-7".to_owned()),
        ..Isolation::default()
    };
    let args = |command: Command| -> Vec<String> {
        command
            .get_args()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect()
    };
    let run = args(isolation.windows_command(Net::Denied, &["cmd.exe".to_owned()]));
    assert_eq!(
        run[2..],
        ["--backend", "user", "--group", "dxh-7", "--", "cmd.exe"]
    );
    let setup = args(isolation.windows_group_command("--setup-group", "dxh-7"));
    assert_eq!(
        setup,
        [
            "--backend",
            "user",
            "--setup-group",
            "dxh-7",
            "--grant",
            "C:\\q\\work"
        ]
    );
    let host = args(isolation.windows_host_command("--ensure-host-denies"));
    assert_eq!(
        host,
        [
            "--backend",
            "user",
            "--ensure-host-denies",
            "--deny",
            "C:\\q\\secret"
        ]
    );
}

#[test]
fn cache_miss_marker_follows_the_host_os() {
    assert_eq!(miss_marker("linux"), MISS_MARKER);
    assert_eq!(miss_marker("macos"), MACOS_DENIED_MARKER);
    assert_eq!(miss_marker("windows"), MISS_MARKER);
}

#[test]
fn markers_follow_the_host_os() {
    assert_eq!(
        markers("linux"),
        (AMBIENT_MARKER, NETWORK_MARKER, HOME_MARKER)
    );
    assert_eq!(
        markers("macos"),
        (
            "Operation not permitted",
            "Operation not permitted",
            HOME_MARKER
        )
    );
    assert_eq!(
        markers("windows"),
        (AMBIENT_MARKER, WINDOWS_DENIED_MARKER, HOME_MARKER)
    );
    assert_eq!(
        probe_expectation("network", "macos").1,
        "Operation not permitted"
    );
    assert_eq!(probe_expectation("network", "linux").1, NETWORK_MARKER);
}

#[test]
fn cache_miss_expectation_follows_the_host_os() {
    assert_eq!(probe_expectation("cache_miss", "linux").1, MISS_MARKER);
    assert_eq!(probe_expectation("cache_miss", "windows").1, MISS_MARKER);
    assert_eq!(
        probe_expectation("cache_miss", "macos").1,
        MACOS_DENIED_MARKER
    );
}

#[test]
fn macos_evidence_is_checked_against_macos_markers() {
    let manifest = cpp_rust();
    let mut evidence = good_evidence();
    evidence["exec"]["os"] = json!("macos");
    evidence["host"]["os"] = json!("macos");
    for probe in evidence["probes"].as_array_mut().unwrap() {
        let name = probe["name"].as_str().unwrap().to_owned();
        probe["marker"] = json!(probe_expectation(&name, "macos").1);
    }
    verify_evidence(&evidence, "abc123", &manifest).unwrap();
    let mut stale = evidence.clone();
    stale["exec"]["os"] = json!("linux");
    assert!(verify_evidence(&stale, "abc123", &manifest).is_err());
    let linux = good_evidence();
    assert!(verify_evidence(&linux, "abc123", &manifest).is_ok());
}

#[cfg(unix)]
#[test]
fn copy_test_logs_keeps_layout_and_returns_heads() {
    let dir = tempfile::tempdir().unwrap();
    let logs = dir.path().join("bazel-testlogs");
    let case = logs.join("javascript/cli/exit_codes_test");
    std::fs::create_dir_all(&case).unwrap();
    let body: String = (1..=50).map(|n| format!("line {n}\n")).collect();
    std::fs::write(case.join("test.log"), body).unwrap();
    std::fs::write(case.join("test.xml"), "<testsuites/>").unwrap();
    let dest = dir.path().join("out/testlogs");
    let text = copy_test_logs(&logs, &dest);
    assert!(
        dest.join("javascript/cli/exit_codes_test/test.log")
            .is_file()
    );
    assert!(
        !dest
            .join("javascript/cli/exit_codes_test/test.xml")
            .exists()
    );
    assert!(text.contains("javascript/cli/exit_codes_test/test.log"));
    assert!(text.contains("line 40") && !text.contains("line 41"));
    assert_eq!(copy_test_logs(&dir.path().join("missing"), &dest), "");
}

#[test]
fn windows_tool_dirs_lists_python_node_jdk_and_llvm() {
    let root = std::env::temp_dir().join(format!("dx-hermetic-win-{}", std::process::id()));
    let pf = root.join("pf");
    let local = root.join("local");
    for dir in [
        pf.join("Python312"),
        pf.join("nodejs"),
        pf.join("Java").join("jdk-21").join("bin"),
        pf.join("LLVM").join("bin"),
        pf.join("Eclipse Adoptium").join("jdk-17").join("bin"),
        local.join("Programs").join("Python").join("Python311"),
        pf.join("Other"),
    ] {
        std::fs::create_dir_all(&dir).unwrap();
    }
    let dirs = windows_tool_dirs(Some(&pf), Some(&local));
    let canon = |path: PathBuf| canonical(&path).unwrap();
    let want = [
        canon(pf.join("Python312")),
        canon(pf.join("Java").join("jdk-21").join("bin")),
        canon(pf.join("Eclipse Adoptium").join("jdk-17").join("bin")),
        canon(pf.join("nodejs")),
        canon(pf.join("LLVM").join("bin")),
        canon(local.join("Programs").join("Python").join("Python311")),
    ];
    let _ = std::fs::remove_dir_all(&root);
    assert_eq!(dirs, want);
    assert!(windows_tool_dirs(None, None).is_empty());
}

#[test]
fn canonical_paths_have_no_verbatim_prefix() {
    let dir = std::env::temp_dir();
    let real = canonical(&dir).unwrap();
    assert!(!real.to_string_lossy().starts_with(r"\\?\"));
    assert!(canonical(&dir.join("dx-hermetic-missing-path")).is_none());
}

#[test]
fn strip_verbatim_drops_only_the_drive_prefix() {
    let strip = |text: &str| strip_verbatim(PathBuf::from(text));
    assert_eq!(strip(r"\\?\D:\a"), PathBuf::from(r"D:\a"));
    assert_eq!(
        strip(r"\\?\UNC\srv\share"),
        PathBuf::from(r"\\?\UNC\srv\share")
    );
    assert_eq!(strip("/tmp/x"), PathBuf::from("/tmp/x"));
}

#[test]
fn windows_leak_package_uses_powershell_and_keeps_the_markers() {
    let build = leaks_build_windows("python");
    assert!(build.contains("cmd_ps = \"python --version | Out-File"));
    assert!(build.contains("dx-hermetic: home config absent"));
    assert!(build.contains("TcpClient('1.1.1.1', 443)"));
    assert!(build.contains("os error "));
    assert!(!build.contains("/dev/tcp"));
    assert_eq!(build.matches("\"manual\"").count(), 3);
    assert!(build.contains("\"requires-network\""));
}

#[test]
fn action_path_is_system_only() {
    if cfg!(windows) {
        let path = action_path(Some(r"D:\Win"), Some(Path::new(r"C:\q\shims")));
        assert_eq!(
            path,
            r"D:\Win\System32;D:\Win;D:\Win\System32\WindowsPowerShell\v1.0;C:\q\shims"
        );
    } else {
        assert_eq!(action_path(None, None), ACTION_PATH);
    }
    assert_eq!(SHIM_NAME.ends_with(".cmd"), cfg!(windows));
}

#[test]
fn windows_msvc_denies_lists_the_existing_visual_studio_and_sdk_trees() {
    let dir = tempfile::tempdir().unwrap();
    let pf = dir.path().join("pf");
    let pf86 = dir.path().join("pf86");
    for path in [
        pf86.join("Microsoft Visual Studio"),
        pf86.join("Windows Kits"),
        pf86.join("Microsoft SDKs"),
        pf.join("Microsoft Visual Studio"),
        pf.join("Other"),
    ] {
        std::fs::create_dir_all(&path).unwrap();
    }
    assert_eq!(
        windows_msvc_denies(Some(&pf), Some(&pf86)),
        [
            pf86.join("Microsoft Visual Studio"),
            pf.join("Microsoft Visual Studio"),
            pf86.join("Windows Kits"),
            pf86.join("Microsoft SDKs"),
        ]
    );
    assert_eq!(
        windows_msvc_denies(Some(&pf), Some(&pf)),
        [pf.join("Microsoft Visual Studio")]
    );
    assert!(windows_msvc_denies(None, None).is_empty());
}

#[test]
fn manifest_is_msvc_only_for_the_msvc_compiler() {
    let mut manifest = manifest_for("//q:cpp", &["cpp"]);
    assert!(!manifest.is_msvc());
    manifest.compiler = Some(("gcc".to_owned(), "GCC 16".to_owned()));
    assert!(!manifest.is_msvc());
    manifest.compiler = Some(("msvc".to_owned(), "19.44".to_owned()));
    assert!(manifest.is_msvc());
}

#[test]
fn windows_passthrough_drops_bazel_vc_when_visual_studio_is_denied() {
    let open = windows_passthrough(false);
    assert!(open.contains(&"BAZEL_VC") && open.contains(&"BAZEL_VC_FULL_VERSION"));
    let denied = windows_passthrough(true);
    assert!(!denied.iter().any(|key| key.starts_with("BAZEL_VC")));
    assert!(denied.contains(&"BAZEL_LLVM") && denied.contains(&"SystemRoot"));
}

#[test]
fn only_the_windows_ambient_probe_takes_the_isolated_path() {
    for name in [
        "network",
        "home_config",
        "home_config_control",
        "cache_miss",
    ] {
        assert!(leak_flags(name).is_empty(), "{name}");
    }
    if !cfg!(windows) {
        assert!(leak_flags("ambient_tool").is_empty());
    }
}

#[cfg(windows)]
#[test]
fn windows_ambient_probe_passes_path_and_pathext() {
    assert_eq!(
        leak_flags("ambient_tool"),
        vec!["--action_env=PATH", "--action_env=PATHEXT"]
    );
}

#[test]
fn windows_bash_prefers_bazel_sh_then_the_first_existing_candidate() {
    let dir = tempfile::tempdir().unwrap();
    let file = |name: &str| {
        let path = dir.path().join(name);
        std::fs::write(&path, "").unwrap();
        path
    };
    let own = file("own-bash.exe");
    let msys = file("msys-bash.exe");
    let git = file("git-bash.exe");
    let missing = dir.path().join("missing-bash.exe");
    let root = dir.path().join("Windows");
    std::fs::create_dir(&root).unwrap();
    let stub = root.join("bash.exe");
    std::fs::write(&stub, "").unwrap();
    let system_root = root.to_string_lossy().to_uppercase();
    let candidates = [missing.clone(), msys.clone(), git];
    assert_eq!(
        windows_bash(Some(&own), &system_root, &candidates),
        Some(own)
    );
    assert_eq!(
        windows_bash(Some(&stub), &system_root, &candidates),
        Some(msys.clone())
    );
    assert_eq!(windows_bash(None, &system_root, &candidates), Some(msys));
    assert_eq!(windows_bash(Some(&missing), &system_root, &[stub]), None);
}

#[test]
fn rename_preset_points_layers_at_the_consumer_repo_name() {
    let dir = tempfile::tempdir().unwrap();
    let layer = dir.path().join(".dx/bazelrc/correctness.bazelrc");
    std::fs::create_dir_all(layer.parent().unwrap()).unwrap();
    std::fs::write(
        &layer,
        "common:windows --extra_execution_platforms=@rules_dx//tools/platforms:windows_msvc_host,@rules_dx//tools/platforms:host\n",
    )
    .unwrap();
    rename_preset(dir.path(), "my_dx").unwrap();
    assert_eq!(
        std::fs::read_to_string(&layer).unwrap(),
        "common:windows --extra_execution_platforms=@my_dx//tools/platforms:windows_msvc_host,@my_dx//tools/platforms:host\n"
    );
}

#[test]
fn copy_latest_profile_takes_the_newest_trace() {
    let dir = tempfile::tempdir().unwrap();
    let base = dir.path().join("base");
    std::fs::create_dir(&base).unwrap();
    let now = std::time::SystemTime::now();
    for (name, age) in [
        ("command-old.profile.gz", 60),
        ("command-new.profile.gz", 0),
    ] {
        let path = base.join(name);
        std::fs::write(&path, name).unwrap();
        let file = std::fs::File::options().write(true).open(&path).unwrap();
        file.set_modified(now - std::time::Duration::from_secs(age))
            .unwrap();
    }
    std::fs::write(base.join("other.gz"), "other").unwrap();
    let dest = dir.path().join("probe.profile.gz");
    copy_latest_profile(&base, &dest);
    assert_eq!(
        std::fs::read_to_string(&dest).unwrap(),
        "command-new.profile.gz"
    );

    let empty = dir.path().join("empty");
    std::fs::create_dir(&empty).unwrap();
    let missing = dir.path().join("none.profile.gz");
    copy_latest_profile(&empty, &missing);
    assert!(!missing.exists());
}

#[test]
fn isolation_check_script_probes_each_deny_and_bash_in_one_file() {
    let denies = [
        (PathBuf::from(r"C:\Program Files (x86)\Windows Kits"), true),
        (PathBuf::from(r"C:\a&b\secret.txt"), false),
    ];
    let script =
        isolation_check_script(&denies, Some(Path::new(r"C:\msys64\usr\bin\bash.exe"))).unwrap();
    assert_eq!(
        script,
        "@echo off\r\n\
         dir /b \"C:\\Program Files (x86)\\Windows Kits\" >nul 2>&1 && echo READABLE C:\\Program Files (x86)\\Windows Kits\r\n\
         type \"C:\\a&b\\secret.txt\" >nul 2>&1 && echo READABLE C:\\a^&b\\secret.txt\r\n\
         \"C:\\msys64\\usr\\bin\\bash.exe\" -c \"exit 0\"\r\n\
         echo BASH %ERRORLEVEL%\r\n\
         echo DONE\r\n\
         exit /b 0\r\n"
    );
    assert_eq!(
        isolation_check_script(&[], None).unwrap(),
        "@echo off\r\necho DONE\r\nexit /b 0\r\n"
    );
    let err = isolation_check_script(&[(PathBuf::from(r"C:\100%\x"), true)], None).unwrap_err();
    assert!(err.contains("contains %"), "{err}");
}

#[test]
fn isolation_check_result_reports_the_first_failure() {
    let outcome = |exit: i32, stdout: &str| Outcome {
        exit,
        stdout: stdout.to_owned(),
        stderr: " why \n".to_owned(),
    };
    let bash = Path::new(r"C:\msys64\usr\bin\bash.exe");
    let launch = isolation_check_result(&outcome(5, ""), Some(bash)).unwrap_err();
    assert!(
        launch.starts_with("namespace isolation is unavailable on this host (exit 5): why;"),
        "{launch}"
    );
    let unfinished = isolation_check_result(&outcome(0, "BASH 0\r\n"), Some(bash)).unwrap_err();
    assert!(
        unfinished.starts_with("namespace isolation is unavailable"),
        "{unfinished}"
    );
    let readable = isolation_check_result(
        &outcome(0, "READABLE C:\\VS\r\nBASH 127\r\nDONE\r\n"),
        Some(bash),
    );
    assert_eq!(
        readable.unwrap_err(),
        r"C:\VS is still readable inside the isolation"
    );
    let no_bash = isolation_check_result(&outcome(0, "BASH 127\r\nDONE\r\n"), Some(bash));
    assert_eq!(
        no_bash.unwrap_err(),
        r"the isolated account cannot run C:\msys64\usr\bin\bash.exe (exit 127): why"
    );
    assert_eq!(
        isolation_check_result(&outcome(0, "BASH 0\r\nDONE\r\n"), Some(bash)),
        Ok(())
    );
    assert_eq!(
        isolation_check_result(&outcome(0, "DONE\r\n"), None),
        Ok(())
    );
}
