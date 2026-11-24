use dx_guard::allowlist::{self, Counts, hit};
use dx_guard::lexer::{Token, rust_tokens};
use dx_guard::snapshot::snapshot;
use dx_guard::starlark::{callee_name, exprs, is_starlark, parse, string};
use serde_json as _;
use starlark_syntax::syntax::ast::{ArgumentP, AstExpr, ExprP};
use std::path::Path;
use yaml_serde as _;

const KINDS: [&str; 9] = [
    "default-shell-env",
    "env-inherit",
    "genrule",
    "home",
    "host-path",
    "rc-env-passthrough",
    "run-shell",
    "sh-rule",
    "shebang",
];

const ROOTED_DIRS: [&str; 2] = ["/usr/bin", "/usr/local/bin"];

const PATH_VARS: [&str; 2] = ["$PATH", "${PATH}"];

const PATH_STARTS: [char; 6] = [' ', '"', '\'', ':', '=', ';'];

const WINDOWS_DIRS: [&str; 8] = [
    ":/windows",
    ":\\windows",
    "/system32",
    "\\system32",
    "%systemroot%",
    "%windir%",
    "$env:systemroot",
    "$env:windir",
];

const WINDOWS_TOKENS: [&str; 3] = ["SystemRoot", "windir", "System32"];

const XCODE_FILE: &str = "apple/private/tools/ops/src/xcode.rs";

const XCODE_TOOLS: [&str; 6] = [
    "/usr/bin/xcrun",
    "/usr/bin/codesign",
    "/usr/bin/security",
    "/usr/bin/plutil",
    "/usr/bin/ditto",
    "/usr/bin/openssl",
];

const SHELLS: [&str; 5] = ["sh", "bash", "zsh", "dash", "ksh"];

const CALLS: [(&str, &str); 4] = [
    ("run_shell(", "run-shell"),
    ("genrule(", "genrule"),
    ("sh_test(", "sh-rule"),
    ("sh_binary(", "sh-rule"),
];

const PASSTHROUGH: [&str; 3] = ["--action_env=", "--host_action_env=", "--test_env="];

fn host_path(text: &str) -> bool {
    PATH_VARS.iter().any(|var| text.contains(var))
        || ROOTED_DIRS.iter().any(|dir| {
            text.match_indices(dir).any(|(at, _)| {
                text.get(..at)
                    .expect("string slice within bounds")
                    .chars()
                    .next_back()
                    .is_none_or(|c| c == '(' || PATH_STARTS.contains(&c))
            })
        })
        || text
            .split([':', '=', ' ', '"', '\''])
            .any(|part| part == "/bin")
}

fn windows_dir(text: &str) -> bool {
    let text = text.to_ascii_lowercase();
    WINDOWS_DIRS.iter().any(|dir| text.contains(dir))
}

fn home(text: &str) -> bool {
    text.contains("$HOME") || text.contains("${HOME}")
}

fn shebang(text: &str) -> bool {
    let Some(rest) = text.lines().next().and_then(|line| line.strip_prefix("#!")) else {
        return false;
    };
    rest.split_whitespace()
        .map(|word| word.rsplit('/').next().unwrap_or(word))
        .any(|word| SHELLS.contains(&word))
}

fn is_false(expr: &AstExpr) -> bool {
    matches!(&expr.node, ExprP::Identifier(ident) if ident.node.ident == "False")
}

fn text_kinds(text: &str, mut add: impl FnMut(&str)) {
    if host_path(text) {
        add("host-path");
    }
    if text.contains("use_default_shell_env = True") {
        add("default-shell-env");
    }
    for (needle, kind) in CALLS {
        if text.contains(needle) {
            add(kind);
        }
    }
}

fn scan_starlark(path: &str, source: &str, counts: &mut Counts) {
    let module = parse(path, source).unwrap_or_else(|error| panic!("{error}"));
    for expr in exprs(module.statement()) {
        if let Some(text) = string(expr) {
            if host_path(text) || windows_dir(text) {
                hit(counts, path, "host-path");
            }
            if text == "HOME" || home(text) {
                hit(counts, path, "home");
            }
            if text == "env_inherit" {
                hit(counts, path, "env-inherit");
            }
            if text == "use_default_shell_env" {
                hit(counts, path, "default-shell-env");
            }
            if shebang(text) {
                hit(counts, path, "shebang");
            }
        }
        let ExprP::Call(callee, args) = &expr.node else {
            continue;
        };
        match callee_name(callee) {
            "run_shell" => hit(counts, path, "run-shell"),
            "genrule" => hit(counts, path, "genrule"),
            "sh_test" | "sh_binary" => hit(counts, path, "sh-rule"),
            _ => {}
        }
        for arg in &args.args {
            match &arg.node {
                ArgumentP::Named(key, _) if key.node == "env_inherit" => {
                    hit(counts, path, "env-inherit");
                }
                ArgumentP::Named(key, value)
                    if key.node == "use_default_shell_env" && !is_false(value) =>
                {
                    hit(counts, path, "default-shell-env");
                }
                ArgumentP::Positional(_)
                | ArgumentP::Named(..)
                | ArgumentP::Args(_)
                | ArgumentP::KwArgs(_) => {}
            }
        }
    }
}

fn scan_rust(path: &str, source: &str, counts: &mut Counts) {
    for token in rust_tokens(source) {
        let Token::Text(text) = token else {
            continue;
        };
        if path == XCODE_FILE && XCODE_TOOLS.contains(&text.as_str()) {
            continue;
        }
        text_kinds(&text, |kind| hit(counts, path, kind));
        if WINDOWS_TOKENS.contains(&text.as_str()) {
            hit(counts, path, "host-path");
        }
        if shebang(&text) {
            hit(counts, path, "shebang");
        }
    }
}

fn scan_lines(path: &str, source: &str, counts: &mut Counts) {
    let rc = path.ends_with(".bazelrc");
    for line in source.lines() {
        let line = line.trim();
        if rc && line.starts_with('#') {
            continue;
        }
        text_kinds(line, |kind| hit(counts, path, kind));
        if windows_dir(line) && !host_path(line) {
            hit(counts, path, "host-path");
        }
        if home(line) {
            hit(counts, path, "home");
        }
        if !rc {
            continue;
        }
        for word in line.split_whitespace() {
            let name = PASSTHROUGH.iter().find_map(|flag| word.strip_prefix(flag));
            if name.is_some_and(|name| !name.contains('=')) {
                hit(counts, path, "rc-env-passthrough");
            }
        }
    }
}

fn scan(path: &str, source: &str, counts: &mut Counts) {
    if shebang(source) {
        hit(counts, path, "shebang");
    }
    if is_starlark(path) {
        scan_starlark(path, source, counts);
    } else if Path::new(path)
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("rs"))
    {
        scan_rust(path, source, counts);
    } else {
        scan_lines(path, source, counts);
    }
}

fn scan_all(sources: &[(&str, &str)]) -> Vec<(String, String, usize)> {
    let mut counts = Counts::new();
    for (path, source) in sources {
        scan(path, source, &mut counts);
    }
    counts
        .into_iter()
        .map(|((path, kind), count)| (path, kind, count))
        .collect()
}

fn lines(entries: &[(&str, &str, usize)]) -> Vec<(String, String, usize)> {
    entries
        .iter()
        .map(|(path, kind, count)| ((*path).to_owned(), (*kind).to_owned(), *count))
        .collect()
}

#[test]
fn the_tree_matches_the_allowlist() {
    let mut counts = Counts::new();
    for (path, source) in &snapshot() {
        scan(path, source, &mut counts);
    }
    allowlist::verify(&counts, &KINDS);
}

#[test]
fn host_directories_on_a_path_count() {
    let bzl = r#"
def _impl(ctx):
    ctx.actions.run(env = {"PATH": "/usr/bin:/bin"})
    ctx.actions.run(env = {"PATH": "/usr/local/bin"})
    ctx.actions.run(env = {"PATH": tool + ":$PATH"})
    ctx.actions.run(env = {"PATH": "${PATH}"})
    ctx.actions.run(env = {"PATH": "/bin"})
    ctx.actions.run(env = {"PATH": "/dx_no_path", "TOOL": "bazel-out/bin/tool"})
"#;
    assert_eq!(
        scan_all(&[("pkg/defs.bzl", bzl)]),
        lines(&[("pkg/defs.bzl", "host-path", 5)]),
    );
}

#[test]
fn repository_relative_host_directories_do_not_count() {
    assert!(!host_path("toolchain/usr/bin/swiftc.exe"));
    assert!(!host_path("@r//:toolchain/usr/bin/x"));
    assert!(host_path("exec /usr/bin/xcrun"));
    assert!(host_path("PATH=/usr/bin"));
    assert!(host_path("/usr/bin/env"));
}

#[test]
fn windows_system_directories_on_a_path_count() {
    let bzl = r#"
def _impl(ctx):
    ctx.actions.run(env = {"PATH": "C:/Windows/System32;C:/Windows"})
    ctx.actions.run(env = {"PATH": r"C:\WINDOWS\system32"})
    ctx.actions.run(env = {"SYSTEMROOT": "%SystemRoot%"})
    ctx.actions.run(env = {"OS": "Windows", "DIR": "System32", "PLATFORM": "windows_x86_64"})
"#;
    let sh = "PATH=\"$env:windir\"\n";
    let rust = "const DIR: &str = \"C:/Windows/System32\";\n";
    assert_eq!(
        scan_all(&[
            ("pkg/defs.bzl", bzl),
            ("pkg/run.sh", sh),
            ("pkg/src/lib.rs", rust),
        ]),
        lines(&[
            ("pkg/defs.bzl", "host-path", 3),
            ("pkg/run.sh", "host-path", 1)
        ]),
    );
}

#[test]
fn rust_tokens_naming_the_system_root_count() {
    let rust = r#"
fn path() -> PathBuf {
    let root = std::env::var_os("SystemRoot");
    let dir = std::env::var("windir");
    Path::new(&root).join("System32")
}
"#;
    assert_eq!(
        scan_all(&[("pkg/src/lib.rs", rust)]),
        lines(&[("pkg/src/lib.rs", "host-path", 3)]),
    );
}

#[test]
fn rust_path_test_data_naming_windows_does_not_count() {
    let rust = r#"
const A: &str = "C:/Windows/Temp";
const B: &str = "C:\\Windows\\System32\\cmd.exe";
const C: &str = "systemroot";
const D: &str = "SystemRoot=C:";
"#;
    assert_eq!(scan_all(&[("pkg/src/lib.rs", rust)]), lines(&[]));
}

#[test]
fn windows_scripts_naming_the_system_root_count() {
    let bat = "set PATH=%SystemRoot%\\System32\nrem %windir%\n";
    let cmd = "set PATH=C:\\Windows\\System32\n";
    let ps1 = "$env:PATH = \"$env:SystemRoot\\System32\"\nWrite-Host ok\n";
    assert_eq!(
        scan_all(&[("pkg/a.bat", bat), ("pkg/b.cmd", cmd), ("pkg/c.ps1", ps1)]),
        lines(&[
            ("pkg/a.bat", "host-path", 2),
            ("pkg/b.cmd", "host-path", 1),
            ("pkg/c.ps1", "host-path", 1),
        ]),
    );
}

#[test]
fn the_xcode_tools_are_free_only_whole_and_only_in_the_xcode_file() {
    let rust = r#"
pub const XCRUN: &str = "/usr/bin/xcrun";
pub const CODESIGN: &str = "/usr/bin/codesign";
pub const SECURITY: &str = "/usr/bin/security";
pub const PLUTIL: &str = "/usr/bin/plutil";
pub const DITTO: &str = "/usr/bin/ditto";
"#;
    assert_eq!(scan_all(&[(XCODE_FILE, rust)]), lines(&[]));
    assert_eq!(
        scan_all(&[("apple/private/tools/ops/src/other.rs", rust)]),
        lines(&[("apple/private/tools/ops/src/other.rs", "host-path", 5)]),
    );
    let near = "const A: &str = \"/usr/bin/xcrunx\";\nconst B: &str = \"/usr/bin/xcrun --x\";\nconst C: &str = \"/usr/bin/lipo\";\n";
    assert_eq!(
        scan_all(&[(XCODE_FILE, near)]),
        lines(&[(XCODE_FILE, "host-path", 3)]),
    );
}

#[test]
fn home_counts_as_a_key_and_as_a_shell_variable() {
    let bzl = r#"
def _impl(ctx):
    ctx.actions.run(env = {"HOME": ctx.attr.home})
    ctx.actions.run(env = {"CACHE": "$HOME/.cache", "OTHER": "${HOME}"})
    ctx.actions.run(env = {"HOMEBREW": "1"})
"#;
    let sh = "cd \"$HOME\"\nexport X=${HOME}/x\necho HOME\n";
    assert_eq!(
        scan_all(&[("pkg/defs.bzl", bzl), ("pkg/run.tpl", sh)]),
        lines(&[("pkg/defs.bzl", "home", 3), ("pkg/run.tpl", "home", 2),]),
    );
}

#[test]
fn env_inherit_counts_as_an_argument_and_as_a_key() {
    let build = r#"
rust_test(name = "a", env_inherit = ["PATH"])
rust_test(name = "b", env_inherit = INHERITED)
"#;
    let bzl = r#"
def wrapped_test(name, **kwargs):
    kwargs["env_inherit"] = ["HOME"]
    native.cc_test(name = name, **kwargs)
"#;
    assert_eq!(
        scan_all(&[("pkg/BUILD.bazel", build), ("pkg/defs.bzl", bzl)]),
        lines(&[
            ("pkg/BUILD.bazel", "env-inherit", 2),
            ("pkg/defs.bzl", "env-inherit", 1),
            ("pkg/defs.bzl", "home", 1),
        ]),
    );
}

#[test]
fn the_default_shell_env_counts_unless_it_is_false() {
    let bzl = r#"
def _impl(ctx):
    ctx.actions.run(use_default_shell_env = True)
    ctx.actions.run(use_default_shell_env = flag)
    ctx.actions.run(use_default_shell_env = False)
    kwargs = {"use_default_shell_env": True}
    ctx.actions.run(**kwargs)
"#;
    let rust = r##"
fn rule() -> &'static str {
    r#"ctx.actions.run(use_default_shell_env = True)"#
}
"##;
    assert_eq!(
        scan_all(&[("pkg/defs.bzl", bzl), ("pkg/src/lib.rs", rust)]),
        lines(&[
            ("pkg/defs.bzl", "default-shell-env", 3),
            ("pkg/src/lib.rs", "default-shell-env", 1),
        ]),
    );
}

#[test]
fn shell_actions_and_shell_rules_count() {
    let bzl = r#"
def _impl(ctx):
    ctx.actions.run_shell(command = "true")

def macro(name):
    native.genrule(name = name, cmd = "true")
    native.sh_binary(name = name + "_bin")
"#;
    let build = r#"
genrule(name = "a", cmd = "true")
sh_test(name = "b", srcs = ["b.sh"])
sh_binary(name = "c", srcs = ["c.sh"])
cc_library(name = "d")
"#;
    let rust = r#"
const BUILD: &str = "sh_test(name = \"t\")\n";
const RULE: &str = "ctx.actions.run_shell(command = cmd)";
// sh_test( in a comment
"#;
    assert_eq!(
        scan_all(&[
            ("pkg/BUILD.bazel", build),
            ("pkg/defs.bzl", bzl),
            ("pkg/src/lib.rs", rust),
        ]),
        lines(&[
            ("pkg/BUILD.bazel", "genrule", 1),
            ("pkg/BUILD.bazel", "sh-rule", 2),
            ("pkg/defs.bzl", "genrule", 1),
            ("pkg/defs.bzl", "run-shell", 1),
            ("pkg/defs.bzl", "sh-rule", 1),
            ("pkg/src/lib.rs", "run-shell", 1),
            ("pkg/src/lib.rs", "sh-rule", 1),
        ]),
    );
}

#[test]
fn shell_shebangs_count_in_files_and_in_written_text() {
    let bzl = r##"
_LAUNCHER = """#!/usr/bin/env bash
exec "$0.bin"
"""
_PYTHON = "#!/usr/bin/env python3\n"
"##;
    let rust = "const SCRIPT: &str = \"#!/bin/sh\\nexit 0\\n\";\n";
    assert_eq!(
        scan_all(&[
            ("pkg/defs.bzl", bzl),
            ("pkg/run.sh", "#!/bin/bash\ntrue\n"),
            ("pkg/src/lib.rs", rust),
            ("pkg/tool.sh", "#!/usr/bin/env python3\n"),
        ]),
        lines(&[
            ("pkg/defs.bzl", "shebang", 1),
            ("pkg/run.sh", "shebang", 1),
            ("pkg/src/lib.rs", "shebang", 1),
        ]),
    );
}

#[test]
fn rc_flags_that_pass_a_host_variable_through_count() {
    let rc = "
# build --action_env=HOME
build --action_env=CC
build --host_action_env=CC --test_env=LANG
test --test_env=MODE=fast
build --action_env=PATH=/usr/bin
";
    assert_eq!(
        scan_all(&[(".bazelrc", rc)]),
        lines(&[
            (".bazelrc", "host-path", 1),
            (".bazelrc", "rc-env-passthrough", 3),
        ]),
    );
}

#[test]
fn a_spare_or_loose_allowlist_line_fails() {
    let mut found = Counts::new();
    hit(&mut found, "pkg/BUILD.bazel", "genrule");
    hit(&mut found, "pkg/BUILD.bazel", "genrule");
    let check = |allowlist: &str| allowlist::check(&found, allowlist, &KINDS, allowlist::REMOVE);
    assert_eq!(check("pkg/BUILD.bazel genrule 2\n"), Vec::<String>::new());
    assert_eq!(
        check("pkg/BUILD.bazel genrule 1\n"),
        ["pkg/BUILD.bazel genrule 2: 1 allowed; remove the new one"],
    );
    assert_eq!(
        check("pkg/BUILD.bazel genrule 3\n"),
        ["pkg/BUILD.bazel genrule 3: 2 left; lower the allowlist count"],
    );
    assert_eq!(
        check("gone/BUILD.bazel genrule 1\npkg/BUILD.bazel genrule 2\n"),
        ["gone/BUILD.bazel genrule 1: none left; delete the allowlist line"],
    );
    assert_eq!(
        check("pkg/BUILD.bazel genrule 2\npkg/BUILD.bazel sh_test 1\n"),
        ["malformed line: \"pkg/BUILD.bazel sh_test 1\""],
    );
    assert_eq!(
        check("pkg/BUILD.bazel genrule 2\npkg/BUILD.bazel genrule 2\n"),
        ["not sorted or repeated: \"pkg/BUILD.bazel genrule 2\""],
    );
}
