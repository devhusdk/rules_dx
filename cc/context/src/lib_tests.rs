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
        .compile_commands(Path::new("/execroot"))
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
    let commands = graph.compile_commands(&root).expect("compile command");
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
        .compile_commands(&root)
        .expect_err("missing response file");
    assert!(matches!(error, Error::ResponseFile { .. }), "{error}");
    fs::remove_dir_all(&root).ok();
}

#[test]
fn tokenize_response_rejects_an_unterminated_quote() {
    let error = tokenize_response("compile.params", "-I \"open").expect_err("unterminated quote");
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
    let commands = graph.compile_commands(&root).expect("compile command");
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
    let commands = graph.compile_commands(&root).expect("compile command");
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
    let commands = graph.compile_commands(&root).expect("compile commands");
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
    let commands = graph.compile_commands(&root).expect("compile command");
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
    let commands = graph.compile_commands(&root).expect("compile command");
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
    let error = graph.compile_commands(&root).expect_err("source flag");
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
