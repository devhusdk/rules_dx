use super::*;

const SNIPPET: &str = "bazel_dep(name = \"rules_dx\", version = \"0.0.0\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.22\")\nregister_toolchains(\"@llvm//toolchain:all\")\n";

fn manifest_for(label: &str, languages: &[&str]) -> Manifest {
    Manifest {
        label: label.to_owned(),
        languages: languages
            .iter()
            .map(|language| (*language).to_owned())
            .collect(),
        module: "m".to_owned(),
        repo_name: "r".to_owned(),
        rust_target_triples: Vec::new(),
        workspace_dir: "ws".to_owned(),
        patterns: vec!["//...".to_owned()],
        failures: Vec::new(),
        coverage: Vec::new(),
        covered: Vec::new(),
        files: Vec::new(),
        module_fragment: PathBuf::from("f"),
        fragment_before_toolchains: false,
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
            let (expect, marker) = probe_expectation(name);
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
fn module_renames_rules_dx_and_overrides_it_locally() {
    let module = compose_module(
        SNIPPET,
        "hermetic_consumer",
        "my_dx",
        "bazel_dep(name = \"googletest\", version = \"1.18.0\")\n",
        false,
        Path::new("/src/rules dx"),
    )
    .unwrap();
    assert_eq!(
        module,
        "module(name = \"hermetic_consumer\")\n\nbazel_dep(name = \"rules_dx\", repo_name = \"my_dx\", version = \"0.0.0\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.22\")\nregister_toolchains(\"@llvm//toolchain:all\")\nbazel_dep(name = \"googletest\", version = \"1.18.0\")\nlocal_path_override(\n    module_name = \"rules_dx\",\n    path = \"/src/rules dx\",\n)\n"
    );
}

#[test]
fn module_keeps_the_default_name_unrenamed() {
    let module = compose_module(SNIPPET, "m", "rules_dx", "", false, Path::new("/src")).unwrap();
    assert!(module.contains("bazel_dep(name = \"rules_dx\", version = \"0.0.0\")"));
    assert!(!module.contains("repo_name"));
}

#[test]
fn module_escapes_quotes_in_the_override_path() {
    let module = compose_module(SNIPPET, "m", "my_dx", "", false, Path::new("/a\"b\\c")).unwrap();
    assert!(module.contains("path = \"/a\\\"b\\\\c\","), "{module}");
}

#[test]
fn module_rejects_a_snippet_without_exactly_one_rules_dx() {
    let missing = compose_module(
        "bazel_dep(name = \"rules_cc\")\n",
        "m",
        "x",
        "",
        false,
        Path::new("/"),
    );
    assert!(missing.unwrap_err().contains("exactly once"));
    let twice = format!("{SNIPPET}{SNIPPET}");
    assert!(compose_module(&twice, "m", "x", "", false, Path::new("/")).is_err());
}

#[test]
fn module_puts_the_fragment_before_the_first_toolchain_registration_on_request() {
    let module = compose_module(
        SNIPPET,
        "m",
        "my_dx",
        "register_toolchains(\"@cuda_host_llvm//:all\")\n\n",
        true,
        Path::new("/src"),
    )
    .unwrap();
    assert_eq!(
        module,
        "module(name = \"m\")\n\nbazel_dep(name = \"rules_dx\", repo_name = \"my_dx\", version = \"0.0.0\")\nbazel_dep(name = \"rules_cc\", version = \"0.2.22\")\nregister_toolchains(\"@cuda_host_llvm//:all\")\n\nregister_toolchains(\"@llvm//toolchain:all\")\nlocal_path_override(\n    module_name = \"rules_dx\",\n    path = \"/src\",\n)\n"
    );
}

#[test]
fn module_appends_the_fragment_unchanged_without_the_flag() {
    let fragment = "register_toolchains(\"@cuda_host_llvm//:all\")\n";
    let module = compose_module(SNIPPET, "m", "my_dx", fragment, false, Path::new("/src")).unwrap();
    assert!(
        module.find("@llvm//toolchain:all").unwrap() < module.find("@cuda_host_llvm").unwrap(),
        "{module}"
    );
}

#[test]
fn module_rejects_the_flag_when_the_snippet_registers_no_toolchain() {
    let err = compose_module(
        "bazel_dep(name = \"rules_dx\", version = \"0.0.0\")\n",
        "m",
        "my_dx",
        "bazel_dep(name = \"rules_cuda\", version = \"0.3.0\")\n",
        true,
        Path::new("/"),
    )
    .unwrap_err();
    assert!(err.contains("no register_toolchains"), "{err}");
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
        "rust_target_triples": [],
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
    assert!(
        !manifest.fragment_before_toolchains,
        "a manifest without the key appends the fragment"
    );
    let mut flagged: Value = serde_json::from_str(&text).unwrap();
    flagged["fragment_before_toolchains"] = json!(true);
    let manifest = parse_manifest(&flagged.to_string(), Path::new("/run")).unwrap();
    assert!(manifest.fragment_before_toolchains);
}

#[test]
fn manifest_rejects_escaping_files_and_empty_patterns() {
    let base = json!({
        "label": "//x:y",
        "languages": ["cpp"],
        "module": "m",
        "repo_name": "r",
        "rust_target_triples": [],
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
    let mut no_triples = unnamed;
    no_triples["label"] = json!("//x:y");
    no_triples
        .as_object_mut()
        .unwrap()
        .remove("rust_target_triples");
    let err = parse_manifest(&no_triples.to_string(), Path::new("/")).unwrap_err();
    assert_eq!(err, "manifest field rust_target_triples must be a list");
}

const RUST_SNIPPET: &str = "rust = use_extension(\"@rules_rust//rust:extensions.bzl\", \"rust\")\nrust.toolchain(\n    edition = \"2021\",\n)\nuse_repo(rust, \"rust_toolchains\")\n";

#[test]
fn rust_target_triples_extend_the_init_toolchain() {
    let triples = vec![
        "wasm32-wasip1".to_owned(),
        "wasm32-unknown-unknown".to_owned(),
    ];
    assert_eq!(
        add_rust_target_triples(RUST_SNIPPET, &triples).unwrap(),
        "rust = use_extension(\"@rules_rust//rust:extensions.bzl\", \"rust\")\nrust.toolchain(\n    extra_target_triples = [\"wasm32-wasip1\", \"wasm32-unknown-unknown\"],\n    edition = \"2021\",\n)\nuse_repo(rust, \"rust_toolchains\")\n"
    );
    assert_eq!(add_rust_target_triples(SNIPPET, &[]).unwrap(), SNIPPET);
}

#[test]
fn rust_target_triples_need_one_rust_toolchain() {
    let triples = vec!["wasm32-wasip1".to_owned()];
    let err = add_rust_target_triples(SNIPPET, &triples).unwrap_err();
    assert!(err.contains("exactly one rust.toolchain"), "{err}");
    let twice = format!("{RUST_SNIPPET}{RUST_SNIPPET}");
    assert!(add_rust_target_triples(&twice, &triples).is_err());
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
            json!({"path": "bazel-out/bin/app", "bytes": 22, "sha256": sha256_hex(&elf), "machine": "aarch64"}),
            json!({"path": "bazel-out/bin/lib a.a", "bytes": 8, "sha256": sha256_hex(b"!<arch>\n"), "machine": null}),
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
    assert!(verify(&foreign)
        .unwrap_err()
        .contains("want //tools/testing/hermetic:cpp_rust"));
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
    assert!(verify_evidence(
        &good_evidence(),
        "abc123",
        &manifest_for("//tools/testing/hermetic:cpp_rust", &[])
    )
    .is_err());
}

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
    assert!(verify(&foreign)
        .unwrap_err()
        .contains("no native artifact machine"));
}

#[test]
fn verify_accepts_machine_free_artifacts_for_managed_languages() {
    let mut beam = good_evidence();
    beam["qualification"] = json!("//tools/testing/hermetic:gleam");
    beam["languages"] = json!(["gleam"]);
    beam["artifacts"] = json!([
        {"path": "bazel-out/k8-fastbuild/bin/app.beam", "sha256": sha256_hex(b"beam"), "machine": null},
    ]);
    beam["target"]["machines"] = json!([]);
    verify_evidence(
        &beam,
        "abc123",
        &manifest_for("//tools/testing/hermetic:gleam", &["gleam"]),
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
        "examples/adopt-cpp/solo/BUILD.bazel",
        "examples/adopt-cpp/solo/pure.cc",
        "examples/adopt-cpp/solo/pure.h",
        "examples/adopt-cpp/solo/pure_test.cc",
        "examples/adopt-rust/solo/BUILD.bazel",
        "examples/adopt-rust/solo/src/lib.rs",
        "examples/adopt-rust/solo/src/net.rs",
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
        "register_toolchains(\"@llvm//toolchain:all\")",
        "register_toolchains(\"@rust_toolchains//:all\")",
        "bazel_dep(name = \"googletest\", version = \"1.18.0\")",
        "path = \"/rules dx source\",",
    ] {
        assert!(
            module.contains(needle),
            "MODULE.bazel lacks {needle}: {module}"
        );
    }
    assert!(
        workspace.join(".dx/version").is_file(),
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
        assert_eq!(
            std::fs::read(workspace.join(dest)).unwrap(),
            want,
            "{dest} is derived from its source"
        );
    }
    assert_eq!(
        std::fs::read_to_string(workspace.join(LEAKS_PACKAGE).join("BUILD.bazel")).unwrap(),
        leaks_build("cc")
    );
}

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
    failing.dx = PathBuf::from("/bin/false");
    let err = materialize(&failing, &dir.path().join("b"), source, "cc").unwrap_err();
    assert!(err.starts_with("dx init failed: "), "{err}");
    let mut silent = real.clone();
    silent.dx = PathBuf::from("/bin/true");
    let err = materialize(&silent, &dir.path().join("c"), source, "cc").unwrap_err();
    assert!(
        err.starts_with("dx init wrote no MODULE.bazel.snippet: "),
        "{err}"
    );
    let mut no_fragment = real;
    no_fragment.module_fragment = dir.path().join("no-fragment");
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

#[test]
fn bazel_argv_pins_startup_and_build_flags_before_the_targets() {
    let session = Session {
        manifest: parse_manifest(
            &json!({
                "label": "//x:y",
                "languages": ["cpp"],
                "module": "m",
                "repo_name": "r",
                "rust_target_triples": [],
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
        bazelisk_home: PathBuf::from("/q/bazelisk"),
        isolation: Isolation::default(),
        probes: Vec::new(),
    };
    let argv = session.bazel_argv(&Probe {
        name: "hermetic_test".to_owned(),
        net: Net::Denied,
        home: Path::new("/q/home"),
        output_root: Path::new("/q/cold"),
        repos: Path::new("/q/repos"),
        command: "test",
        targets: vec!["//...".to_owned()],
        extra: vec!["--test_output=errors".to_owned()],
        expect: Expect::Pass,
    });
    assert_eq!(
        argv[..5],
        [
            "/usr/bin/env",
            "-i",
            "HOME=/q/home",
            "PATH=/usr/bin:/bin",
            "BAZELISK_HOME=/q/bazelisk"
        ]
    );
    assert_eq!(
        argv[8..],
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
            "--test_output=errors",
            "--",
            "//...",
        ]
    );
    assert!(argv[7].contains("shutdown"), "{}", argv[7]);
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
    let missing = dir.path().join("missing");
    assert!(find_bazelisk(Some(&missing.to_string_lossy()), "").is_err());
    let real = dir.path().join("real").join("bazel");
    std::fs::create_dir_all(real.parent().unwrap()).unwrap();
    std::fs::write(&real, fake_elf()).unwrap();
    let both = format!("{}:{}", path, real.parent().unwrap().display());
    assert_eq!(find_bazelisk(None, &both).unwrap(), real);
}

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
        "rust_target_triples": [],
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
    for name in ["coverage", "fetch_coverage"] {
        let err =
            verify_evidence(&without_probe(&evidence, name), "abc123", &manifest).unwrap_err();
        assert!(
            err.contains(&format!("missing required probe {name}")),
            "{err}"
        );
    }
    let mut online = evidence.clone();
    probe_mut(&mut online, "coverage")["network"] = json!("allowed");
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
    let names: Vec<String> = required_probes(&manifest)
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
