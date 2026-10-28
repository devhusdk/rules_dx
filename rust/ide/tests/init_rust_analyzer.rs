//! Runs the real `dx init --apply` in a scratch Rust workspace and drives the pinned
//! rust-analyzer headless with the `rust-analyzer.*` settings that init wrote.

use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use dx_lsp::Session;
use dx_nested::{self as nested, Case};
use dx_testing::{read_text, resolve_runfiles, rules_dx_source, write_text};
use serde_json::{json, Value};

const MODULE: &str = r#"module(name = "shop")

bazel_dep(name = "rules_dx", version = "0.0.0")
local_path_override(module_name = "rules_dx", path = "RULES_DX")
"#;

const LIB_BUILD: &str = r#"load("@rules_dx//rust/rules:defs.bzl", "rust_library")

rust_library(
    name = "greet",
    srcs = ["lib.rs"],
    edition = "2021",
    visibility = ["//visibility:public"],
)
"#;

const APP_BUILD: &str = r#"load("@rules_dx//rust/rules:defs.bzl", "rust_binary")

rust_binary(
    name = "app",
    srcs = ["main.rs"],
    edition = "2021",
    deps = ["//lib:greet"],
)
"#;

const LIB_RS: &str = "pub fn greeting() -> &'static str {\n    \"hello\"\n}\n";
const MAIN_RS: &str = "fn main() {\n    println!(\"{}\", greet::greeting());\n}\n";
const BROKEN_RS: &str = "fn main() {\n    println!(\"{}\", greet::missing());\n}\n";

const LOAD_DEADLINE: Duration = Duration::from_secs(25 * 60);
const CHECK_DEADLINE: Duration = Duration::from_secs(10 * 60);

fn env_path(name: &str) -> PathBuf {
    let rel = std::env::var(name).unwrap_or_else(|_| panic!("{name} must be set"));
    resolve_runfiles(&rel)
}

fn stage(case: &Case) {
    let rules = rules_dx_source();
    case.write(".bazelversion", &read_text(&rules.join(".bazelversion")));
    case.write(
        ".bazelrc",
        &read_text(&rules.join("tools/bazelrc/preset.bazelrc")),
    );
    case.write(
        "MODULE.bazel",
        &MODULE.replace("RULES_DX", &rules.display().to_string()),
    );
    case.write("lib/BUILD.bazel", LIB_BUILD);
    case.write("lib/lib.rs", LIB_RS);
    case.write("app/BUILD.bazel", APP_BUILD);
    case.write("app/main.rs", MAIN_RS);
}

fn init(case: &Case) {
    let root = case.root().to_string_lossy().into_owned();
    let applied = case.run(
        &env_path("DX_BIN"),
        &["init", "--workspace", &root, "--apply", "shop"],
        &[],
    );
    assert_eq!(applied.status.code(), Some(0), "{}", applied.combined());
    let ignore = read_text(&case.root().join(".bazelignore"));
    assert!(ignore.lines().any(|line| line.trim() == ".dx"), "{ignore}");
    assert!(case.root().join(".vscode/settings.json").is_file());
    let snippet = read_text(&case.root().join("MODULE.bazel.snippet"));
    let module = read_text(&case.root().join("MODULE.bazel"));
    let mut text = module;
    for line in snippet.lines() {
        let own = line.starts_with("bazel_dep(name = \"rules_dx\"");
        if line.starts_with('#') || own || line.starts_with("include(") {
            continue;
        }
        text.push_str(line);
        text.push('\n');
    }
    case.write("MODULE.bazel", &text);
}

fn nest(settings: &Value, root: &str) -> Value {
    let object = settings.as_object().expect("settings.json is an object");
    let mut nested_options = json!({});
    for (key, value) in object {
        let Some(path) = key.strip_prefix("rust-analyzer.") else {
            continue;
        };
        let mut at = &mut nested_options;
        let parts: Vec<&str> = path.split('.').collect();
        for part in &parts[..parts.len() - 1] {
            at = &mut at[*part];
        }
        at[parts[parts.len() - 1]] = substitute(value, root);
    }
    let text = nested_options.to_string();
    assert!(!text.contains("${"), "unsubstituted variable in {text}");
    assert!(text.contains("discoverConfig"), "{text}");
    assert!(text.contains("overrideCommand"), "{text}");
    nested_options
}

fn substitute(value: &Value, root: &str) -> Value {
    match value {
        Value::String(text) => Value::String(text.replace("${workspaceFolder}", root)),
        Value::Array(items) => Value::Array(items.iter().map(|v| substitute(v, root)).collect()),
        Value::Object(map) => Value::Object(
            map.iter()
                .map(|(k, v)| (k.clone(), substitute(v, root)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn uri(path: &Path) -> String {
    format!("file://{}", path.display())
}

fn start(case: &Case, root: &Path, options: Value) -> Session {
    let mut command = case.command(&env_path("DX_RUST_ANALYZER"), &[]);
    command.current_dir(root);
    let mut session = Session::spawn(command, json!({"rust-analyzer": options.clone()}))
        .unwrap_or_else(|error| panic!("{error}"));
    let init = session
        .call(
            "initialize",
            json!({
                "processId": std::process::id(),
                "rootUri": uri(root),
                "workspaceFolders": [{"uri": uri(root), "name": "shop"}],
                "initializationOptions": options,
                "capabilities": {
                    "window": {"workDoneProgress": true},
                    "workspace": {"configuration": true},
                    "textDocument": {"publishDiagnostics": {}},
                },
            }),
            Duration::from_secs(60),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    assert!(init.is_some(), "rust-analyzer never answered initialize");
    session
        .notify("initialized", json!({}))
        .unwrap_or_else(|error| panic!("{error}"));
    session
}

fn position_of(text: &str, needle: &str) -> Value {
    let offset = text.find(needle).expect("needle in text");
    let line = text[..offset].matches('\n').count();
    let column = offset - text[..offset].rfind('\n').map_or(0, |at| at + 1);
    json!({"line": line, "character": column})
}

fn definition_paths(result: &Value) -> Vec<String> {
    let items = match result {
        Value::Array(items) => items.clone(),
        Value::Null => Vec::new(),
        single => vec![single.clone()],
    };
    items
        .iter()
        .filter_map(|item| item.get("uri").or_else(|| item.get("targetUri")))
        .filter_map(Value::as_str)
        .map(|uri| uri.trim_start_matches("file://").to_owned())
        .collect()
}

fn goto_definition(session: &mut Session, root: &Path) {
    let main = root.join("app/main.rs");
    session
        .notify(
            "textDocument/didOpen",
            json!({"textDocument": {
                "uri": uri(&main), "languageId": "rust", "version": 1, "text": MAIN_RS,
            }}),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    let expected = root.join("lib/lib.rs").display().to_string();
    let deadline = Instant::now() + LOAD_DEADLINE;
    let mut last = Value::Null;
    loop {
        let reply = session
            .call(
                "textDocument/definition",
                json!({
                    "textDocument": {"uri": uri(&main)},
                    "position": position_of(MAIN_RS, "greeting"),
                }),
                Duration::from_secs(60),
            )
            .unwrap_or_else(|error| panic!("{error}"));
        if let Some(reply) = reply {
            last = reply["result"].clone();
            if definition_paths(&last).contains(&expected) {
                return;
            }
        }
        assert!(
            Instant::now() < deadline,
            "goto-definition never reached {expected}; last result {last}; stderr {}; messages {}",
            session
                .stderr()
                .lines()
                .rev()
                .take(15)
                .collect::<Vec<_>>()
                .join("\n"),
            session
                .log
                .iter()
                .filter(|m| m["method"] != "$/progress")
                .map(|m| m.to_string().chars().take(400).collect::<String>())
                .collect::<Vec<_>>()
                .join("\n")
        );
        std::thread::sleep(Duration::from_secs(5));
    }
}

fn is_rustc_error(message: &Value) -> bool {
    message["method"] == "textDocument/publishDiagnostics"
        && message["params"]["diagnostics"]
            .as_array()
            .is_some_and(|items| {
                items
                    .iter()
                    .any(|d| d["source"] == "rustc" && d["code"] == "E0425")
            })
}

fn flycheck_reports_error(session: &mut Session, root: &Path) {
    let main = root.join("app/main.rs");
    session
        .notify(
            "textDocument/didChange",
            json!({
                "textDocument": {"uri": uri(&main), "version": 2},
                "contentChanges": [{"text": BROKEN_RS}],
            }),
        )
        .unwrap_or_else(|error| panic!("{error}"));
    std::fs::write(&main, BROKEN_RS).unwrap_or_else(|error| panic!("write {main:?}: {error}"));
    let deadline = Instant::now() + CHECK_DEADLINE;
    while Instant::now() < deadline {
        session
            .notify(
                "textDocument/didSave",
                json!({"textDocument": {"uri": uri(&main)}}),
            )
            .unwrap_or_else(|error| panic!("{error}"));
        let seen = session
            .wait_for(Duration::from_secs(90), is_rustc_error)
            .unwrap_or_else(|error| panic!("{error}"));
        if seen.is_some() {
            return;
        }
    }
    let tail: Vec<String> = session
        .log
        .iter()
        .rev()
        .take(5)
        .map(Value::to_string)
        .collect();
    panic!("no rustc E0425 diagnostic from the flycheck command; last messages {tail:?}");
}

fn expunge(case: &Case, base: &str) {
    let flag = format!("--output_base={}/{base}", case.root().display());
    let _ = case.bazel(&[&flag, "shutdown"]);
}

/// Ignored until two product bugs are fixed; run with `--test_arg=--ignored`:
/// 1. The pinned rust-analyzer parses every stderr line of the discover command as JSON, so
///    `bazel run` noise ("Starting local Bazel server", "Target ... up-to-date:") is reported as
///    "Project discovery failed".
/// 2. `rules_dx` `rust_binary` is a forwarder whose deps live on `<name>_upstream`, so the
///    rules_rust aspect finds no `deps` and the discovered `app` crate has no `greet` dependency.
#[test]
#[ignore = "discover noise on stderr and forwarder targets hide deps from rules_rust (see #2392)"]
fn rust_analyzer_follows_the_settings_dx_init_writes() {
    let case = nested::case("init_rust_analyzer");
    stage(&case);
    init(&case);
    let built = case.plain("build", "//app:app", &[]);
    assert_eq!(built.status.code(), Some(0), "{}", built.combined());
    let root = std::fs::canonicalize(case.root()).expect("canonical root");
    let settings: Value =
        serde_json::from_str(&read_text(&root.join(".vscode/settings.json"))).expect("json");
    let options = nest(&settings, &root.display().to_string());
    write_text(&root.join("app/main.rs"), MAIN_RS);
    let mut session = start(&case, &root, options);
    goto_definition(&mut session, &root);
    assert!(root.join(".dx/ide/rust").is_dir());
    flycheck_reports_error(&mut session, &root);
    assert!(root.join(".dx/ide/rust_check").is_dir());
    session.shutdown();
    expunge(&case, ".dx/ide/rust");
    expunge(&case, ".dx/ide/rust_check");
    case.shutdown_server();
}
