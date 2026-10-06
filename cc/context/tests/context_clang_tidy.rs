use std::path::{Path, PathBuf};
use std::process::Command;

struct Env {
    root: PathBuf,
    tree: PathBuf,
    tidy: PathBuf,
}

fn env_path(name: &str) -> PathBuf {
    PathBuf::from(std::env::var(name).unwrap_or_else(|_| panic!("{name}")))
}

fn tag(name: &str) -> String {
    format!("cc-context-{}-{name}", std::process::id())
}

fn stage(name: &str) -> Env {
    let root = std::env::temp_dir().join(tag(name));
    std::fs::create_dir_all(&root).expect("scratch root");
    let tree = root.join("execroot");
    std::fs::create_dir_all(&tree).expect("execroot");
    let tidy = root.join("clang-tidy");
    std::fs::copy(env_path("DX_CLANG_TIDY"), &tidy).expect("clang-tidy staged");
    let source = tree.join("cc/tests/fixtures/compilation_db");
    std::fs::create_dir_all(&source).expect("source dir");
    std::fs::copy(
        runfiles("cc/tests/fixtures/compilation_db/probe.c"),
        source.join("probe.c"),
    )
    .expect("probe staged");
    let generated =
        tree.join("bazel-out/k8-fastbuild/bin/cc/tests/fixtures/compilation_db/generated");
    std::fs::create_dir_all(&generated).expect("generated dir");
    std::fs::copy(env_path("DX_GENERATED_HEADER"), generated.join("limits.h"))
        .expect("generated header staged");
    Env { root, tree, tidy }
}

fn runfiles(relative: &str) -> PathBuf {
    let workspace = std::env::var("TEST_WORKSPACE").expect("TEST_WORKSPACE");
    let srcdir = std::env::var("TEST_SRCDIR").expect("TEST_SRCDIR");
    Path::new(&srcdir).join(workspace).join(relative)
}

fn record(name: &str) -> String {
    std::fs::read_to_string(runfiles(&format!(
        "cc/tests/fixtures/compilation_db/{name}.aquery.json"
    )))
    .expect("aquery record")
}

fn policy(env: &Env) -> PathBuf {
    let path = env.root.join(".clang-tidy");
    std::fs::copy(
        runfiles("cc/tests/fixtures/compilation_db/.clang-tidy"),
        &path,
    )
    .expect("policy staged");
    path
}

fn produce(
    env: &Env,
    name: &str,
    label: &str,
    out: &str,
) -> Result<Vec<cc_context::Entry>, String> {
    let graph = cc_context::AqueryGraph::parse(&record(name)).map_err(|err| err.to_string())?;
    let commands = graph
        .compile_commands(&env.tree)
        .map_err(|err| err.to_string())?;
    let sources = vec!["cc/tests/fixtures/compilation_db/probe.c".to_string()];
    let entries = cc_context::database(&commands, &env.tree, label, &sources)
        .map_err(|err| err.to_string())?;
    let dir = env.root.join(out);
    let policy = std::fs::read_to_string(policy(env)).expect("policy body");
    cc_context::write_bundle(&dir, &entries, &policy).map_err(|err| err.to_string())?;
    Ok(entries)
}

fn tidy(env: &Env, bundle: &str) -> (Option<i32>, String) {
    let output = Command::new(&env.tidy)
        .arg("-p")
        .arg(env.root.join(bundle))
        .arg("cc/tests/fixtures/compilation_db/probe.c")
        .current_dir(&env.tree)
        .output()
        .expect("clang-tidy runs");
    (
        output.status.code(),
        String::from_utf8_lossy(&output.stdout).into_owned()
            + &String::from_utf8_lossy(&output.stderr),
    )
}

#[test]
fn the_legacy_configuration_reports_a_real_include_and_macro_finding() {
    let env = stage("legacy");
    let entries = produce(
        &env,
        "probe_legacy",
        "//cc/tests/fixtures/compilation_db:probe_legacy",
        "legacy",
    )
    .expect("legacy bundle");
    assert_eq!(entries.len(), 1);
    assert!(entries[0]
        .arguments
        .contains(&"-DDX_ENABLE_LEGACY_BRANCH".to_string()));
    assert!(entries[0]
        .file
        .ends_with("cc/tests/fixtures/compilation_db/probe.c"));
    assert!(
        entries[0]
            .file
            .starts_with(&env.tree.to_string_lossy().to_string()),
        "{}",
        entries[0].file
    );
    let (code, text) = tidy(&env, "legacy");
    assert_eq!(code, Some(1), "{text}");
    assert!(
        text.contains("readability-else-after-return"),
        "the checked-in policy check must run: {text}"
    );
    assert!(
        text.contains("probe.c:9:3"),
        "the finding must land on the macro-guarded line: {text}"
    );
    assert!(
        !text.contains("file not found"),
        "the generated header must resolve: {text}"
    );
    std::fs::remove_dir_all(&env.root).ok();
}

#[test]
fn the_plain_configuration_is_clean_because_the_macro_is_absent() {
    let env = stage("plain");
    let entries = produce(
        &env,
        "probe_plain",
        "//cc/tests/fixtures/compilation_db:probe_plain",
        "plain",
    )
    .expect("plain bundle");
    assert!(!entries[0]
        .arguments
        .contains(&"-DDX_ENABLE_LEGACY_BRANCH".to_string()));
    let (code, text) = tidy(&env, "plain");
    assert_eq!(code, Some(0), "{text}");
    assert!(!text.contains("readability-else-after-return"), "{text}");
    assert!(!text.contains("file not found"), "{text}");
    std::fs::remove_dir_all(&env.root).ok();
}

#[test]
fn a_consumer_defined_macro_wins_over_the_generated_default() {
    let env = stage("consumer");
    let entries = produce(
        &env,
        "probe_consumer_toolchain",
        "//cc/tests/fixtures/compilation_db:probe_consumer_toolchain",
        "consumer",
    )
    .expect("consumer bundle");
    assert!(entries[0]
        .arguments
        .contains(&"-DDX_ZERO_RESULT=9".to_string()));
    let (code, text) = tidy(&env, "consumer");
    assert_eq!(code, Some(1), "{text}");
    assert!(text.contains("readability-else-after-return"), "{text}");
    assert!(!text.contains("redefined"), "{text}");
    std::fs::remove_dir_all(&env.root).ok();
}

#[test]
fn two_configurations_of_one_source_do_not_leak_into_one_database() {
    let env = stage("leak");
    let first = cc_context::AqueryGraph::parse(&record("probe_legacy")).expect("legacy record");
    let second = cc_context::AqueryGraph::parse(&record("probe_plain")).expect("plain record");
    let legacy = first.compile_commands(&env.tree).expect("legacy commands");
    let plain = second.compile_commands(&env.tree).expect("plain commands");
    assert_eq!(legacy[0].source, plain[0].source);
    assert_ne!(legacy[0].arguments, plain[0].arguments);
    let mut merged = legacy;
    merged.extend(plain);
    let error = cc_context::database(
        &merged,
        &env.tree,
        "//cc/tests/fixtures/compilation_db:probe_legacy",
        &[],
    )
    .expect_err("two configurations of one source");
    assert_eq!(
        error,
        cc_context::Error::ConflictingSource {
            input: "cc/tests/fixtures/compilation_db/probe.c".to_string(),
            count: 2
        }
    );
    std::fs::remove_dir_all(&env.root).ok();
}

#[test]
fn missing_context_fails_closed_instead_of_running_with_no_flags() {
    let env = stage("missing");
    let graph = cc_context::AqueryGraph::parse(&record("probe_legacy")).expect("legacy record");
    let commands = graph.compile_commands(&env.tree).expect("commands");
    let absent = cc_context::database(
        &commands,
        &env.tree,
        "//cc/tests/fixtures/compilation_db:absent",
        &[],
    )
    .expect_err("absent label");
    assert_eq!(
        absent,
        cc_context::Error::NoCompileAction {
            mnemonic: cc_context::COMPILE_MNEMONIC.to_string(),
            label: "//cc/tests/fixtures/compilation_db:absent".to_string()
        }
    );
    let unknown = cc_context::database(
        &commands,
        &env.tree,
        "//cc/tests/fixtures/compilation_db:probe_legacy",
        &["cc/tests/fixtures/compilation_db/absent.c".to_string()],
    )
    .expect_err("absent source");
    assert_eq!(
        unknown,
        cc_context::Error::MissingSource {
            input: "cc/tests/fixtures/compilation_db/absent.c".to_string()
        }
    );
    assert!(!env.root.join("missing").exists());
    std::fs::remove_dir_all(&env.root).ok();
}

#[test]
fn an_unbound_policy_fails_closed() {
    let env = stage("policy");
    let empty = env.root.join("empty/.clang-tidy");
    std::fs::create_dir_all(empty.parent().expect("policy parent")).expect("policy dir");
    std::fs::write(&empty, "  \n").expect("empty policy");
    assert_eq!(
        cc_context::read_policy(&empty).expect_err("empty policy"),
        cc_context::Error::EmptyPolicy {
            path: empty.to_string_lossy().into_owned()
        }
    );
    let wrong = env.root.join("policy.yaml");
    std::fs::write(&wrong, "Checks: '-*'\n").expect("wrong extension");
    assert_eq!(
        cc_context::read_policy(&wrong).expect_err("wrong extension"),
        cc_context::Error::WrongPolicyExtension {
            path: wrong.to_string_lossy().into_owned(),
            want: cc_context::POLICY_EXTENSION.to_string()
        }
    );
    std::fs::remove_dir_all(&env.root).ok();
}

#[test]
fn a_response_file_carries_the_real_target_flags() {
    let env = stage("response");
    let params = env
        .tree
        .join("bazel-out/k8-fastbuild/bin/probe_legacy.params");
    std::fs::create_dir_all(params.parent().expect("params parent")).expect("params dir");
    std::fs::write(
        &params,
        "-DDX_ENABLE_LEGACY_BRANCH -I \"quoted include dir\" -DDX_ZERO_RESULT=11\n",
    )
    .expect("params file");
    let text = record("probe_legacy");
    let mut graph = cc_context::AqueryGraph::parse(&text).expect("record decodes");
    let index = graph.actions[0]
        .arguments
        .iter()
        .position(|arg| arg == "-DDX_ENABLE_LEGACY_BRANCH")
        .expect("define present");
    graph.actions[0].arguments[index] =
        "@bazel-out/k8-fastbuild/bin/probe_legacy.params".to_string();
    let commands = graph.compile_commands(&env.tree).expect("commands");
    assert!(
        commands[0]
            .arguments
            .iter()
            .any(|arg| arg == "quoted include dir"),
        "quoted"
    );
    assert!(
        commands[0]
            .arguments
            .iter()
            .any(|arg| arg == "-DDX_ZERO_RESULT=11"),
        "zero"
    );
    assert!(
        !commands[0].arguments.iter().any(|arg| arg.starts_with('@')),
        "at"
    );
    let entries = vec![cc_context::Entry {
        directory: env.tree.to_string_lossy().into_owned(),
        file: env
            .tree
            .join("cc/tests/fixtures/compilation_db/probe.c")
            .to_string_lossy()
            .into_owned(),
        arguments: commands[0].arguments.clone(),
    }];
    let dir = env.root.join("response");
    let policy = std::fs::read_to_string(policy(&env)).expect("policy body");
    cc_context::write_bundle(&dir, &entries, &policy).expect("bundle");
    let (code, text) = tidy(&env, "response");
    assert_eq!(code, Some(1), "{text}");
    assert!(text.contains("readability-else-after-return"), "{text}");
    assert!(!text.contains("file not found"), "{text}");
    std::fs::remove_dir_all(&env.root).ok();
}
