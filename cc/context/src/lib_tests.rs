use super::*;

fn graph_json(arguments: &[&str], source: &str, target: &str) -> String {
    let arguments = arguments
        .iter()
        .map(|argument| format!("\"{argument}\""))
        .collect::<Vec<String>>()
        .join(",");
    format!(
        r#"{{"actions":[{{"mnemonic":"CppCompile","arguments":[{arguments}],"inputDepSetIds":[1],"outputIds":[2],"targetId":1,"configurationId":1,"executionPlatform":"@@platforms//host:host","actionKey":"deadbeef"}}],"artifacts":[{{"id":2,"path":"bazel-out/k8-fastbuild/bin/pkg/probe.pic.o"}},{{"id":3,"path":"pkg/{source}"}}],"depSetOfFiles":[{{"id":1,"directArtifactIds":[3],"transitiveDepSetIds":[]}}],"targets":[{{"id":1,"label":"{target}"}}],"configuration":[{{"id":1,"mnemonic":"k8-fastbuild","checksum":"f18286977fdf8ec6876d94ee65aa2f1fe504afa7d9e9f3d7d7a5cf34f197a37e"}}]}}"#
    )
}

fn identity(configuration: &str, driver: Driver) -> CompilerIdentity {
    CompilerIdentity {
        configuration: configuration.to_string(),
        compiler: "cc".to_string(),
        family: "clang".to_string(),
        driver,
        abi: "itanium".to_string(),
        target: "x86_64-linux-gnu".to_string(),
    }
}

fn gnu() -> CompilerIdentity {
    identity("k8-fastbuild", Driver::Gnu)
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("cc-context-{}-{tag}", std::process::id()));
    fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

#[test]
fn configured_compile_action_keeps_its_own_argv_and_inputs() {
    let text = graph_json(
        &[
            "/usr/bin/gcc",
            "-std=c++17",
            "-c",
            "pkg/probe.cc",
            "-o",
            "out.o",
        ],
        "probe.cc",
        "//pkg:probe",
    );
    let graph = AqueryGraph::parse(&text).expect("record decodes");
    let commands = graph
        .compile_commands(Path::new("/execroot"), &[gnu()])
        .expect("compile command");
    assert_eq!(commands.len(), 1);
    let command = &commands[0];
    assert_eq!(command.target, "//pkg:probe");
    assert_eq!(command.source, "pkg/probe.cc");
    assert_eq!(
        command.configuration,
        "k8-fastbuild (f18286977fdf8ec6876d94ee65aa2f1fe504afa7d9e9f3d7d7a5cf34f197a37e)"
    );
    assert_eq!(command.execution_platform, "@@platforms//host:host");
    assert_eq!(command.inputs, vec!["pkg/probe.cc".to_string()]);
    assert_eq!(
        command.outputs,
        vec!["bazel-out/k8-fastbuild/bin/pkg/probe.pic.o".to_string()]
    );
    assert_eq!(
        command.arguments,
        vec![
            "/usr/bin/gcc".to_string(),
            "-std=c++17".to_string(),
            "-c".to_string(),
            "pkg/probe.cc".to_string(),
            "-o".to_string(),
            "out.o".to_string(),
        ]
    );
}

#[test]
fn response_file_arguments_are_expanded_in_place() {
    let root = scratch("configured");
    fs::write(
        root.join("compile.params"),
        "-I \"quoted dir\" -DFLAG=1\n  -DOTHER=2\n",
    )
    .expect("response file");
    let text = graph_json(
        &["/usr/bin/gcc", "@compile.params", "-c", "pkg/probe.cc"],
        "probe.cc",
        "//pkg:probe",
    );
    let graph = AqueryGraph::parse(&text).expect("record decodes");
    let commands = graph
        .compile_commands(&root, &[gnu()])
        .expect("compile command");
    assert_eq!(
        commands[0].arguments,
        vec![
            "/usr/bin/gcc".to_string(),
            "-I".to_string(),
            "quoted dir".to_string(),
            "-DFLAG=1".to_string(),
            "-DOTHER=2".to_string(),
            "-c".to_string(),
            "pkg/probe.cc".to_string(),
        ]
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn an_unreadable_response_file_fails_closed() {
    let root = scratch("response_expand");
    let text = graph_json(
        &["/usr/bin/gcc", "@absent.params", "-c", "pkg/probe.cc"],
        "probe.cc",
        "//pkg:probe",
    );
    let graph = AqueryGraph::parse(&text).expect("record decodes");
    let error = graph
        .compile_commands(&root, &[gnu()])
        .expect_err("missing response file");
    assert!(matches!(error, Error::ResponseFile { .. }), "{error}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tokenize_response_rejects_an_unterminated_quote() {
    let error = tokenize_response("compile.params", "-I \"open", Quoting::Gnu)
        .expect_err("unterminated quote");
    assert_eq!(
        error,
        Error::UnterminatedQuote {
            path: "compile.params".to_string()
        }
    );
}

#[test]
fn database_entries_resolve_against_the_execroot() {
    let root = scratch("response_absent");
    let text = graph_json(
        &["/usr/bin/gcc", "-c", "pkg/probe.cc", "-o", "out.o"],
        "probe.cc",
        "//pkg:probe",
    );
    let graph = AqueryGraph::parse(&text).expect("record decodes");
    let commands = graph
        .compile_commands(&root, &[gnu()])
        .expect("compile command");
    let entries = database(
        &commands,
        &root,
        "//pkg:probe",
        &["pkg/probe.cc".to_string()],
    )
    .expect("database");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].directory, root.to_string_lossy());
    assert_eq!(entries[0].file, root.join("pkg/probe.cc").to_string_lossy());
    assert_eq!(entries[0].arguments[0], "/usr/bin/gcc");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_wrapper_target_owns_its_upstream_action() {
    assert!(owns_label("//pkg:probe_upstream", "//pkg:probe"));
    assert!(!owns_label("//pkg:other_upstream", "//pkg:probe"));
    assert!(owns_label("//pkg:probe", "//pkg:probe"));
}

#[test]
fn a_label_without_a_compile_action_fails_closed() {
    let root = scratch("database");
    let text = graph_json(
        &["/usr/bin/gcc", "-c", "pkg/probe.cc"],
        "probe.cc",
        "//pkg:probe_upstream",
    );
    let graph = AqueryGraph::parse(&text).expect("record decodes");
    let commands = graph
        .compile_commands(&root, &[gnu()])
        .expect("compile command");
    let error = database(&commands, &root, "//pkg:absent", &[]).expect_err("no compile action");
    assert_eq!(
        error,
        Error::NoCompileAction {
            mnemonic: COMPILE_MNEMONIC.to_string(),
            label: "//pkg:absent".to_string()
        }
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn two_configurations_of_one_source_fail_closed() {
    let root = scratch("no_action");
    let first = graph_json(
        &["/usr/bin/gcc", "-DDX=1", "-c", "pkg/probe.cc"],
        "probe.cc",
        "//pkg:alpha_upstream",
    );
    let second = graph_json(
        &["/usr/bin/gcc", "-c", "pkg/probe.cc"],
        "probe.cc",
        "//pkg:beta_upstream",
    );
    let mut graph = AqueryGraph::parse(&first).expect("first record decodes");
    let other = AqueryGraph::parse(&second).expect("second record decodes");
    graph.actions.extend(other.actions);
    graph
        .artifacts
        .extend(other.artifacts.into_iter().map(|artifact| Artifact {
            id: artifact.id + 100,
            path: artifact.path,
            path_fragment_id: artifact.path_fragment_id,
        }));
    let commands = graph
        .compile_commands(&root, &[gnu()])
        .expect("compile commands");
    let error = database(&commands, &root, "//pkg:alpha", &[]).expect_err("leaked context");
    assert_eq!(
        error,
        Error::ConflictingSource {
            input: "pkg/probe.cc".to_string(),
            count: 2
        }
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_missing_required_source_fails_closed() {
    let root = scratch("conflict");
    let text = graph_json(
        &["/usr/bin/gcc", "-c", "pkg/probe.cc"],
        "probe.cc",
        "//pkg:probe",
    );
    let graph = AqueryGraph::parse(&text).expect("record decodes");
    let commands = graph
        .compile_commands(&root, &[gnu()])
        .expect("compile command");
    let error = database(
        &commands,
        &root,
        "//pkg:probe",
        &["pkg/other.cc".to_string()],
    )
    .expect_err("required source");
    assert_eq!(
        error,
        Error::MissingSource {
            input: "pkg/other.cc".to_string()
        }
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_relative_execroot_fails_closed() {
    let root = scratch("missing_source");
    let text = graph_json(
        &["/usr/bin/gcc", "-c", "pkg/probe.cc"],
        "probe.cc",
        "//pkg:probe",
    );
    let graph = AqueryGraph::parse(&text).expect("record decodes");
    let commands = graph
        .compile_commands(&root, &[gnu()])
        .expect("compile command");
    let error =
        database(&commands, Path::new("relative"), "//pkg:probe", &[]).expect_err("execroot");
    assert_eq!(
        error,
        Error::RelativeExecroot {
            path: "relative".to_string()
        }
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_compile_action_without_a_source_flag_fails_closed() {
    let root = scratch("relative_execroot");
    let text = graph_json(&["/usr/bin/gcc", "-c"], "probe.cc", "//pkg:probe");
    let graph = AqueryGraph::parse(&text).expect("record decodes");
    let error = graph
        .compile_commands(&root, &[gnu()])
        .expect_err("source flag");
    assert_eq!(
        error,
        Error::MissingSourceFlag {
            target: "//pkg:probe".to_string(),
            flag: "-c".to_string()
        }
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn policy_validation_fails_closed() {
    let root = scratch("no_source_flag");
    let wrong = root.join("clang-tidy.yaml");
    fs::write(&wrong, "Checks: '-*'\n").expect("policy");
    assert_eq!(
        read_policy(&wrong).expect_err("extension"),
        Error::WrongPolicyExtension {
            path: wrong.to_string_lossy().into_owned(),
            want: POLICY_EXTENSION.to_string()
        }
    );
    let empty = root.join(POLICY_FILE);
    fs::write(&empty, "  \n").expect("policy");
    assert_eq!(
        read_policy(&empty).expect_err("empty"),
        Error::EmptyPolicy {
            path: empty.to_string_lossy().into_owned()
        }
    );
    let absent = root.join("absent/.clang-tidy");
    assert!(matches!(
        read_policy(&absent).expect_err("absent"),
        Error::UnreadablePolicy { .. }
    ));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_bundle_carries_the_database_and_the_bound_policy() {
    let root = scratch("bundle");
    let bundle = root.join("bundle");
    let entries = vec![Entry {
        directory: root.to_string_lossy().into_owned(),
        file: root.join("pkg/probe.cc").to_string_lossy().into_owned(),
        arguments: vec!["/usr/bin/gcc".to_string(), "-c".to_string()],
    }];
    write_bundle(&bundle, &entries, "Checks: '-*'\n").expect("bundle");
    let database_text = fs::read_to_string(bundle.join(DATABASE_FILE)).expect("database");
    let parsed: Vec<Entry> = serde_json::from_str(&database_text).expect("database decodes");
    assert_eq!(parsed, entries);
    assert_eq!(
        fs::read_to_string(bundle.join(POLICY_FILE)).expect("policy"),
        "Checks: '-*'\n"
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_malformed_record_fails_closed() {
    let error = AqueryGraph::parse("{").expect_err("malformed");
    assert!(matches!(error, Error::Malformed(_)), "{error}");
}

fn strings(items: &[&str]) -> Vec<String> {
    items.iter().map(|item| item.to_string()).collect()
}

fn windows(text: &str) -> Vec<String> {
    tokenize_response("x.rsp", text, Quoting::Windows).expect("tokens")
}

#[test]
fn windows_quoting_keeps_backslash_paths() {
    assert_eq!(
        windows(r"/IC:\inc\dir /Fo:out\a.obj"),
        strings(&[r"/IC:\inc\dir", r"/Fo:out\a.obj"])
    );
}

#[test]
fn windows_quoting_handles_quotes_and_backslashes() {
    assert_eq!(
        windows(r#""C:\Program Files\cl.exe" a"#),
        strings(&[r"C:\Program Files\cl.exe", "a"])
    );
    assert_eq!(windows(r#"a\"b"#), strings(&[r#"a"b"#]));
    assert_eq!(windows(r#""a\\" b"#), strings(&[r"a\", "b"]));
    assert_eq!(windows(r#""a""b""#), strings(&[r#"a"b"#]));
}

#[test]
fn gnu_quoting_handles_single_quotes() {
    let tokens = tokenize_response("x.rsp", "'a b' \"c d\" e\\ f", Quoting::Gnu).expect("tokens");
    assert_eq!(tokens, strings(&["a b", "c d", "e f"]));
}

#[test]
fn quoting_follows_the_selected_driver() {
    let cl = strings(&["cl.exe"]);
    assert_eq!(rsp_quoting(&cl, Driver::Gnu), Some(Quoting::Gnu));
    assert_eq!(rsp_quoting(&cl, Driver::Msvc), Some(Quoting::Windows));
    assert_eq!(rsp_quoting(&cl, Driver::Unknown), None);
    let mode = strings(&["clang", "--driver-mode=cl"]);
    assert_eq!(rsp_quoting(&mode, Driver::Unknown), Some(Quoting::Windows));
    let posix = strings(&["cl.exe", "--rsp-quoting=posix"]);
    assert_eq!(rsp_quoting(&posix, Driver::Msvc), Some(Quoting::Gnu));
    let windows = strings(&["gcc", "--rsp-quoting=windows"]);
    assert_eq!(rsp_quoting(&windows, Driver::Gnu), Some(Quoting::Windows));
}

fn rsp_graph(root: &Path, rsp: &str, configuration: &str) -> AqueryGraph {
    fs::write(root.join("x.rsp"), rsp).expect("rsp");
    let text = graph_json(
        &["tool", "--odd", "-c", "pkg/a.cc", "@x.rsp"],
        "a.cc",
        "//pkg:a",
    )
    .replace("k8-fastbuild\"", &format!("{configuration}\""));
    AqueryGraph::parse(&text).expect("graph")
}

#[test]
fn an_unknown_driver_keeps_argv_and_response_files_as_recorded() {
    let root = scratch("unknown");
    let graph = rsp_graph(&root, "-DX=1", "k8-fastbuild");
    for identities in [vec![identity("k8-fastbuild", Driver::Unknown)], vec![]] {
        let commands = graph
            .compile_commands(&root, &identities)
            .expect("commands");
        assert_eq!(
            commands[0].arguments,
            strings(&["tool", "--odd", "-c", "pkg/a.cc", "@x.rsp"])
        );
    }
    fs::remove_dir_all(&root).ok();
}

fn with_env(graph: &mut AqueryGraph, include: &str, external: &str) {
    graph.actions[0].environment_variables = [("INCLUDE", include), ("EXTERNAL_INCLUDE", external)]
        .iter()
        .map(|(key, value)| EnvironmentVariable {
            key: key.to_string(),
            value: value.to_string(),
        })
        .collect();
}

#[test]
fn an_msvc_identity_keeps_every_argument_and_adds_the_env_context() {
    let root = scratch("msvc");
    let mut graph = rsp_graph(&root, "/DX=1 \"a b\"", "k8-fastbuild");
    with_env(&mut graph, "C:\\sdk\\inc;C:\\crt", "C:\\ext");
    let commands = graph
        .compile_commands(&root, &[identity("k8-fastbuild", Driver::Msvc)])
        .expect("commands");
    assert_eq!(
        commands[0].arguments,
        strings(&[
            "tool",
            "/imsvcC:/sdk/inc",
            "/imsvcC:/crt",
            "/imsvcC:/ext",
            "/X",
            "--target=x86_64-linux-gnu",
            "--odd",
            "-c",
            "pkg/a.cc",
            "/DX=1",
            "a b"
        ])
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn an_msvc_action_keeps_its_own_target_flag() {
    let root = scratch("msvc_target");
    let mut graph = rsp_graph(&root, "--target=aarch64-windows-msvc", "k8-fastbuild");
    with_env(&mut graph, "inc", "");
    let commands = graph
        .compile_commands(&root, &[identity("k8-fastbuild", Driver::Msvc)])
        .expect("commands");
    let flags = &commands[0].arguments;
    assert_eq!(flags[1..3], strings(&["/imsvcinc", "/X"]));
    assert_eq!(flags.iter().filter(|a| a.contains("target")).count(), 1);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn an_msvc_action_without_include_fails_closed() {
    let root = scratch("msvc_missing");
    let graph = rsp_graph(&root, "/DX=1", "k8-fastbuild");
    let error = graph
        .compile_commands(&root, &[identity("k8-fastbuild", Driver::Msvc)])
        .expect_err("no INCLUDE");
    assert!(matches!(error, Error::MissingMsvcContext { .. }), "{error}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn an_msvc_action_with_argv_imsvc_needs_no_env() {
    let root = scratch("msvc_argv");
    let graph = rsp_graph(
        &root,
        "/imsvcC:/inc --target=x86_64-pc-windows-msvc",
        "k8-fastbuild",
    );
    let commands = graph
        .compile_commands(&root, &[identity("k8-fastbuild", Driver::Msvc)])
        .expect("commands");
    assert_eq!(
        commands[0].arguments,
        strings(&[
            "tool",
            "--odd",
            "-c",
            "pkg/a.cc",
            "/imsvcC:/inc",
            "--target=x86_64-pc-windows-msvc"
        ])
    );
    assert!(!commands[0].arguments.contains(&"/X".to_string()));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn the_version_probe_resolves_argv0_and_skips_the_flag_for_cl() {
    let root = scratch("probe");
    let exec = root.join("exec");
    let base = root.join("base");
    fs::create_dir_all(base.join("external/llvm/bin")).expect("llvm dir");
    fs::write(base.join("external/llvm/bin/clang-cl.exe"), "").expect("clang-cl");
    let (path, args) = version_probe(&exec, &base, "external/llvm/bin/clang-cl.exe");
    assert_eq!(path, base.join("external/llvm/bin/clang-cl.exe"));
    assert_eq!(args, strings(&["--version"]));
    let (path, args) = version_probe(&exec, &base, "external/msvc/bin/Hostx64/x64/CL.exe");
    assert_eq!(path, exec.join("external/msvc/bin/Hostx64/x64/CL.exe"));
    assert!(args.is_empty());
    let (path, _) = version_probe(&exec, &base, "bazel-out/bin/wrapper");
    assert_eq!(path, exec.join("bazel-out/bin/wrapper"));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn each_configuration_gets_its_own_identity() {
    let root = scratch("configs");
    let mut graph = rsp_graph(&root, "-DX=1", "arm64-opt");
    with_env(&mut graph, "inc", "");
    let identities = [gnu(), identity("arm64-opt", Driver::Msvc)];
    let commands = graph
        .compile_commands(&root, &identities)
        .expect("commands");
    let found = commands[0].identity.as_ref().expect("identity");
    assert_eq!(found.driver, Driver::Msvc);
    fs::remove_dir_all(&root).ok();
}

#[test]
fn target_and_exec_actions_each_get_their_identity_and_expand() {
    let root = scratch("exec");
    fs::write(root.join("x.rsp"), "-DX=1").expect("rsp");
    let action = |target: u32, out: u32, config: u32, source: &str| {
        format!(
            r#"{{"mnemonic":"CppCompile","arguments":["gcc","-c","{source}","@x.rsp"],"inputDepSetIds":[{config}],"outputIds":[{out}],"targetId":{target},"configurationId":{config},"executionPlatform":"@@platforms//host:host","actionKey":"k{config}"}}"#
        )
    };
    let text = format!(
        r#"{{"actions":[{},{}],"artifacts":[{{"id":2,"path":"bazel-out/k8-fastbuild/bin/pkg/a.o"}},{{"id":3,"path":"pkg/a.cc"}},{{"id":4,"path":"bazel-out/k8-opt-exec-ST-0/bin/tool/t.o"}},{{"id":5,"path":"tool/t.cc"}}],"depSetOfFiles":[{{"id":1,"directArtifactIds":[3],"transitiveDepSetIds":[]}},{{"id":2,"directArtifactIds":[5],"transitiveDepSetIds":[]}}],"targets":[{{"id":1,"label":"//pkg:a"}},{{"id":2,"label":"//tool:t"}}],"configuration":[{{"id":1,"mnemonic":"k8-fastbuild","checksum":"a"}},{{"id":2,"mnemonic":"k8-opt-exec-ST-0","checksum":"b"}}]}}"#,
        action(1, 2, 1, "pkg/a.cc"),
        action(2, 4, 2, "tool/t.cc"),
    );
    let graph = AqueryGraph::parse(&text).expect("graph");
    let identities = [gnu(), identity("k8-opt-exec-ST-0", Driver::Gnu)];
    let commands = graph
        .compile_commands(&root, &identities)
        .expect("commands");
    assert_eq!(commands.len(), 2);
    for command in &commands {
        assert!(command.identity.is_some());
        assert_eq!(command.arguments.last().map(String::as_str), Some("-DX=1"));
    }
    assert_eq!(clang_tidy_projection(&commands), Ok(()));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn the_clang_tidy_projection_fails_without_a_known_driver() {
    let root = scratch("projection");
    let graph = rsp_graph(&root, "-DX=1", "k8-fastbuild");
    let known = graph.compile_commands(&root, &[gnu()]).expect("commands");
    assert_eq!(clang_tidy_projection(&known), Ok(()));
    for identities in [vec![identity("k8-fastbuild", Driver::Unknown)], vec![]] {
        let commands = graph
            .compile_commands(&root, &identities)
            .expect("commands");
        let error = clang_tidy_projection(&commands).expect_err("unsupported");
        assert!(
            matches!(error, Error::UnsupportedProjection { .. }),
            "{error}"
        );
        let before = database(&commands, &root, "//pkg:a", &[]);
        assert_eq!(before, database(&commands, &root, "//pkg:a", &[]));
    }
    fs::remove_dir_all(&root).ok();
}

#[test]
fn identity_files_are_read_and_checked() {
    let root = scratch("identity-files");
    let write = |name: &str, text: &str| {
        let path = root.join(name);
        fs::write(&path, text).expect("identity");
        path
    };
    let a = write("a.json", r#"{"configuration":"c","driver":"gnu"}"#);
    let same = write("b.json", r#"{"configuration":"c","driver":"gnu"}"#);
    let other = write("c.json", r#"{"configuration":"c","driver":"msvc"}"#);
    let odd = write("d.json", r#"{"configuration":"d","driver":"borland"}"#);
    let bad = write("e.json", "{");
    assert_eq!(read_identities(&[a.clone(), same]).expect("same").len(), 1);
    assert!(matches!(
        read_identities(&[a, other]),
        Err(Error::ConflictingIdentity { .. })
    ));
    assert_eq!(
        read_identities(&[odd]).expect("odd")[0].driver,
        Driver::Unknown
    );
    assert!(matches!(
        read_identities(&[bad]),
        Err(Error::MalformedIdentity { .. })
    ));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn response_files_decode_boms_and_nest() {
    let root = scratch("encodings");
    let mut utf16 = vec![0xFF, 0xFE];
    for unit in "/DA=1 @inner.rsp".encode_utf16() {
        utf16.extend(unit.to_le_bytes());
    }
    fs::write(root.join("outer.rsp"), utf16).expect("outer");
    fs::write(root.join("inner.rsp"), b"\xEF\xBB\xBF/DB=2").expect("inner");
    let args = strings(&["cl.exe", "@outer.rsp", "/Tcfoo.c"]);
    let expanded = expand_response_files(&args, &root, Quoting::Windows).expect("expands");
    assert_eq!(expanded, strings(&["cl.exe", "/DA=1", "/DB=2", "/Tcfoo.c"]));
    assert_eq!(source_argument("//p:t", &expanded), Ok("foo.c"));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn a_self_including_response_file_fails() {
    let root = scratch("cycle");
    fs::write(root.join("a.rsp"), "@a.rsp").expect("a");
    let error = expand_response_files(&strings(&["gcc", "@a.rsp"]), &root, Quoting::Gnu)
        .expect_err("cycle");
    assert_eq!(
        error,
        Error::ResponseFileCycle {
            path: "a.rsp".to_string()
        }
    );
    fs::remove_dir_all(&root).ok();
}

#[test]
fn an_invalid_utf8_response_file_fails() {
    let root = scratch("badutf8");
    fs::write(root.join("a.rsp"), b"\xFF\xFF\xFF").expect("a");
    let error =
        expand_response_files(&strings(&["gcc", "@a.rsp"]), &root, Quoting::Gnu).expect_err("bad");
    assert!(
        matches!(error, Error::ResponseFileEncoding { .. }),
        "{error}"
    );
    fs::remove_dir_all(&root).ok();
}

fn qualify_graph(arguments: &str, param_files: &str) -> AqueryGraph {
    let text = format!(
        r#"{{"actions":[{{"mnemonic":"CppCompile","arguments":[{arguments}],"targetId":1,"configurationId":1,"actionKey":"k1","paramFiles":[{param_files}]}}],"targets":[{{"id":1,"label":"//pkg:probe"}}],"configuration":[{{"id":1,"mnemonic":"k8-fastbuild","checksum":"c"}}]}}"#
    );
    AqueryGraph::parse(&text).expect("record decodes")
}

fn exported(arguments: &[&str], identity: Option<CompilerIdentity>) -> CompileCommand {
    CompileCommand {
        target: "//pkg:probe".to_string(),
        configuration: "k8-fastbuild (c)".to_string(),
        execution_platform: String::new(),
        action_key: "k1".to_string(),
        source: "pkg/probe.cc".to_string(),
        arguments: arguments.iter().map(|a| a.to_string()).collect(),
        inputs: Vec::new(),
        outputs: Vec::new(),
        identity,
    }
}

const PLAIN: &str = r#""/usr/bin/gcc","-c","pkg/probe.cc""#;

#[test]
fn qualify_accepts_exact_gnu_argv() {
    let graph = qualify_graph(PLAIN, "");
    let command = exported(&["/usr/bin/gcc", "-c", "pkg/probe.cc"], Some(gnu()));
    let done = qualify(&graph, &[command], Path::new(".")).expect("qualified");
    assert_eq!(done.len(), 1);
    assert_eq!(done[0].driver, "Gnu");
}

#[test]
fn qualify_expands_param_file_with_spaced_backslash_path() {
    let graph = qualify_graph(
        r#""cl.exe","@bazel-out/p.params""#,
        r#"{"execPath":"bazel-out/p.params","arguments":["/c","C:\\Program Files\\x y\\a.cc"]}"#,
    );
    let command = exported(
        &["cl.exe", "/c", "C:\\Program Files\\x y\\a.cc"],
        Some(gnu()),
    );
    qualify(&graph, &[command], Path::new(".")).expect("qualified");
}

#[test]
fn qualify_rejects_dropped_argument() {
    let graph = qualify_graph(PLAIN, "");
    let command = exported(&["/usr/bin/gcc", "pkg/probe.cc"], Some(gnu()));
    assert!(matches!(
        qualify(&graph, &[command], Path::new(".")),
        Err(Error::Unqualified { .. })
    ));
}

#[test]
fn qualify_rejects_changed_argv0() {
    let graph = qualify_graph(PLAIN, "");
    let command = exported(&["/usr/bin/cc", "-c", "pkg/probe.cc"], Some(gnu()));
    assert!(matches!(
        qualify(&graph, &[command], Path::new(".")),
        Err(Error::Unqualified { .. })
    ));
}

fn msvc() -> CompilerIdentity {
    identity("k8-fastbuild", Driver::Msvc)
}

#[test]
fn qualify_accepts_msvc_projected_prefix() {
    let graph = qualify_graph(r#""clang-cl.exe","/c","a.cc""#, "");
    let command = exported(
        &[
            "clang-cl.exe",
            "/imsvcC:/inc",
            "/X",
            "--target=x86_64-pc-windows-msvc",
            "/c",
            "a.cc",
        ],
        Some(msvc()),
    );
    qualify(&graph, &[command], Path::new(".")).expect("qualified");
}

#[test]
fn qualify_accepts_clang_cl_argv_imsvc_unchanged() {
    let argv = ["clang-cl.exe", "/imsvcC:/inc", "/c", "a.cc"];
    let graph = qualify_graph(r#""clang-cl.exe","/imsvcC:/inc","/c","a.cc""#, "");
    let done =
        qualify(&graph, &[exported(&argv, Some(msvc()))], Path::new(".")).expect("qualified");
    assert_eq!(done[0].expected, done[0].exported);
}

#[test]
fn qualify_rejects_extra_injected_flag() {
    let graph = qualify_graph(r#""clang-cl.exe","/c","a.cc""#, "");
    let command = exported(
        &["clang-cl.exe", "/imsvcC:/inc", "-DEVIL", "/c", "a.cc"],
        Some(msvc()),
    );
    assert!(matches!(
        qualify(&graph, &[command], Path::new(".")),
        Err(Error::Unqualified { .. })
    ));
}

#[test]
fn qualify_rejects_unknown_driver_and_missing_identity() {
    let graph = qualify_graph(PLAIN, "");
    let args = ["/usr/bin/gcc", "-c", "pkg/probe.cc"];
    let unknown = exported(&args, Some(identity("k8-fastbuild", Driver::Unknown)));
    assert!(qualify(&graph, &[unknown], Path::new(".")).is_err());
    assert!(qualify(&graph, &[exported(&args, None)], Path::new(".")).is_err());
}

#[test]
fn qualify_rejects_missing_param_file() {
    let graph = qualify_graph(r#""cl.exe","@bazel-out/p.params""#, "");
    let command = exported(&["cl.exe", "/c"], Some(gnu()));
    assert!(matches!(
        qualify(&graph, &[command], Path::new(".")),
        Err(Error::Unqualified { .. })
    ));
}

#[test]
fn retain_owned_drops_compile_actions_of_other_targets() {
    let text = r#"{"actions":[
        {"mnemonic":"CppCompile","targetId":1},
        {"mnemonic":"CppCompile","targetId":2},
        {"mnemonic":"Other","targetId":2}],
      "targets":[{"id":1,"label":"//pkg:probe_upstream"},{"id":2,"label":"@@llvm+//tools:tool"}]}"#;
    let mut graph = AqueryGraph::parse(text).expect("graph");
    graph.retain_owned(&["//pkg:probe".to_string()]);
    let ids: Vec<(&str, i64)> = graph
        .actions
        .iter()
        .map(|a| (a.mnemonic.as_str(), a.target_id))
        .collect();
    assert_eq!(ids, [("CppCompile", 1), ("Other", 2)]);
}

#[test]
fn qualify_reads_an_unlisted_param_file_from_the_execroot() {
    let root = scratch("qualify-param");
    fs::write(
        root.join("probe.obj.params"),
        "/c\r\n\"C:\\Program Files\\x y\\a.cc\"\r\n",
    )
    .expect("params");
    let graph = qualify_graph(r#""cl.exe","@probe.obj.params""#, "");
    let command = exported(
        &["cl.exe", "/c", "C:\\Program Files\\x y\\a.cc"],
        Some(gnu()),
    );
    qualify(&graph, &[command], &root).expect("qualified");
    let missing = qualify_graph(r#""cl.exe","@gone.params""#, "");
    let command = exported(&["cl.exe", "/c"], Some(gnu()));
    assert!(matches!(
        qualify(&missing, &[command], &root),
        Err(Error::Unqualified { .. })
    ));
    fs::remove_dir_all(&root).ok();
}

#[test]
fn unquote_line_unescapes_quotes_in_quoted_lines() {
    assert_eq!(
        unquote_line(r#""/D__DATE__=\"redacted\"""#),
        r#"/D__DATE__="redacted""#
    );
    assert_eq!(
        unquote_line(r#"/D__DATE__=\"redacted\""#),
        r#"/D__DATE__="redacted""#
    );
    assert_eq!(unquote_line("/c"), "/c");
}
