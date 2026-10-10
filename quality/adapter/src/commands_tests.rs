use super::*;

const BIN: &str = "/scratch/bin/tool";
const FILE: &str = "/scratch/src/main.rs";

fn argv_strings(invocation: &Invocation) -> Vec<String> {
    invocation
        .argv
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}

#[test]
fn buildifier_check_encodes_config_discovery() {
    let file = Path::new(FILE);
    let hinted = buildifier_check(Path::new(BIN), &[file], Some("tools/build"));
    assert_eq!(
        argv_strings(&hinted),
        vec![BIN, "--mode=check", "--format=json", "--lint=warn", FILE]
    );
    assert_eq!(hinted.cwd_rel, "tools/build");
    let bare = buildifier_check(Path::new(BIN), &[file], None);
    assert_eq!(
        argv_strings(&bare),
        vec![
            BIN,
            "--mode=check",
            "--format=json",
            "--lint=warn",
            "--config=off",
            FILE
        ]
    );
    assert_eq!(bare.cwd_rel, "");
}

#[test]
fn buildifier_fix_mirrors_the_check_rule() {
    let file = Path::new(FILE);
    let hinted = buildifier_fix(Path::new(BIN), &[file], Some(""));
    assert_eq!(
        argv_strings(&hinted),
        vec![BIN, "--mode=fix", "--lint=fix", FILE]
    );
    let bare = buildifier_fix(Path::new(BIN), &[file], None);
    assert!(argv_strings(&bare).contains(&"--config=off".to_owned()));
}

#[test]
fn rustfmt_passes_caller_edition_and_config() {
    let file = Path::new(FILE);
    let check = rustfmt(
        Path::new(BIN),
        &[file],
        Path::new("/scratch/r.toml"),
        "2021",
        true,
    );
    assert_eq!(
        argv_strings(&check),
        vec![
            BIN,
            "--edition",
            "2021",
            "--color=never",
            "--config-path",
            "/scratch/r.toml",
            "--check",
            FILE
        ]
    );
    let older = rustfmt(
        Path::new(BIN),
        &[file],
        Path::new("/scratch/r.toml"),
        "2018",
        true,
    );
    assert_eq!(argv_strings(&older)[2], "2018");
    let fix = rustfmt(
        Path::new(BIN),
        &[file],
        Path::new("/scratch/r.toml"),
        "2021",
        false,
    );
    assert!(!argv_strings(&fix).contains(&"--check".to_owned()));
}

#[test]
fn taplo_lint_and_format_encode_schema_policy() {
    let file = Path::new(FILE);
    let lint = taplo_lint(Path::new(BIN), &[file], None);
    assert_eq!(
        argv_strings(&lint),
        vec![
            BIN,
            "lint",
            "--colors=never",
            "--no-auto-config",
            "--no-schema",
            FILE
        ]
    );
    let lint_configured = taplo_lint(Path::new(BIN), &[file], Some(Path::new("/s/t.toml")));
    assert_eq!(
        argv_strings(&lint_configured),
        vec![
            BIN,
            "lint",
            "--colors=never",
            "--no-auto-config",
            "--no-schema",
            "-c",
            "/s/t.toml",
            FILE
        ]
    );
    let check = taplo_format(Path::new(BIN), &[file], None, true);
    assert!(argv_strings(&check).contains(&"--check".to_owned()));
    assert!(!argv_strings(&check).contains(&"--diff".to_owned()));
    let fix = taplo_format(Path::new(BIN), &[file], None, false);
    assert!(!argv_strings(&fix).contains(&"--check".to_owned()));
    let configured = taplo_format(Path::new(BIN), &[file], Some(Path::new("/s/t.toml")), true);
    assert_eq!(
        argv_strings(&configured),
        vec![
            BIN,
            "format",
            "--colors=never",
            "--no-auto-config",
            "--check",
            "-c",
            "/s/t.toml",
            FILE
        ]
    );
}

#[test]
fn markdown_check_maps_workspace_to_absolute() {
    let invocation = markdown_check(
        Path::new(BIN),
        &[
            ("doc/guide.md", Path::new("/scratch/doc/guide.md")),
            ("README.md", Path::new("/scratch/README.md")),
        ],
        &[("LICENSE", Path::new("/scratch/LICENSE"))],
    );
    assert_eq!(
        argv_strings(&invocation),
        vec![
            BIN,
            "--source",
            "doc/guide.md=/scratch/doc/guide.md",
            "--source",
            "README.md=/scratch/README.md",
            "--sibling",
            "LICENSE=/scratch/LICENSE",
        ]
    );
    assert_eq!(invocation.cwd_rel, "");
}

#[test]
fn vale_check_never_passes_no_exit() {
    let file = Path::new(FILE);
    let invocation = vale_check(
        Path::new(BIN),
        Path::new("/scratch/v/.vale.ini"),
        &[file],
        "v",
    );
    assert_eq!(
        argv_strings(&invocation),
        vec![
            BIN,
            "--config",
            "/scratch/v/.vale.ini",
            "--output=JSON",
            "--no-color",
            "--no-global",
            "--no-wrap",
            FILE
        ]
    );
    assert_eq!(invocation.cwd_rel, "v");
}

#[test]
fn ruff_check_is_hermetic_and_config_explicit() {
    let file = Path::new("/scratch/a.py");
    let bare = ruff_check(Path::new(BIN), &[file], None);
    assert_eq!(
        argv_strings(&bare),
        vec![
            BIN,
            "check",
            "--isolated",
            "--no-cache",
            "--no-respect-gitignore",
            "--output-format",
            "json",
            "/scratch/a.py"
        ]
    );
    assert_eq!(bare.cwd_rel, "");
    let hinted = ruff_check(Path::new(BIN), &[file], Some(Path::new("/scratch/r.toml")));
    assert_eq!(
        argv_strings(&hinted),
        vec![
            BIN,
            "check",
            "--config",
            "/scratch/r.toml",
            "--no-cache",
            "--no-respect-gitignore",
            "--output-format",
            "json",
            "/scratch/a.py"
        ]
    );
    assert!(!argv_strings(&hinted).contains(&"--isolated".to_owned()));
}

#[test]
fn ruff_fix_and_format_shapes() {
    let file = Path::new("/scratch/a.py");
    let fix = ruff_fix(Path::new(BIN), &[file], None);
    assert!(argv_strings(&fix).contains(&"--fix".to_owned()));
    assert!(!argv_strings(&fix).contains(&"--output-format".to_owned()));
    let check = ruff_format_check(Path::new(BIN), &[file], None);
    assert_eq!(
        argv_strings(&check),
        vec![
            BIN,
            "format",
            "--isolated",
            "--no-cache",
            "--no-respect-gitignore",
            "--check",
            "--output-format",
            "json",
            "/scratch/a.py"
        ]
    );
    let fix_fmt = ruff_format_fix(Path::new(BIN), &[file], Some(Path::new("/s/r.toml")));
    assert!(argv_strings(&fix_fmt).contains(&"format".to_owned()));
    assert!(!argv_strings(&fix_fmt).contains(&"--check".to_owned()));
    assert!(argv_strings(&fix_fmt).contains(&"/s/r.toml".to_owned()));
}

#[test]
fn ty_check_is_concise_and_hermetic() {
    let file = Path::new("/scratch/a.py");
    let invocation = ty_check(Path::new(BIN), &[file], &[]);
    assert_eq!(
        argv_strings(&invocation),
        vec![
            BIN,
            "check",
            "--output-format",
            "concise",
            "--no-progress",
            "--no-respect-ignore-files",
            "/scratch/a.py"
        ]
    );
    assert_eq!(invocation.cwd_rel, "");
    assert!(!argv_strings(&invocation).contains(&"--fix".to_owned()));
    assert!(!argv_strings(&invocation).contains(&"--add-ignore".to_owned()));
    let search = Path::new("/scratch/pkg");
    let with_search = ty_check(Path::new(BIN), &[file], &[search]);
    let argv = argv_strings(&with_search);
    assert!(argv.contains(&"--extra-search-path".to_owned()));
    assert!(argv.contains(&"/scratch/pkg".to_owned()));
}

#[test]
fn pydoclint_check_is_quiet() {
    let file = Path::new("/scratch/a.py");
    let invocation = pydoclint_check(Path::new(BIN), &[file]);
    assert_eq!(
        argv_strings(&invocation),
        vec![BIN, "--quiet", "/scratch/a.py"]
    );
    assert_eq!(invocation.cwd_rel, "");
}

#[test]
fn flake8_check_is_isolated_single_job() {
    let file = Path::new("/scratch/a.py");
    let invocation = flake8_check(Path::new(BIN), &[file]);
    assert_eq!(
        argv_strings(&invocation),
        vec![
            BIN,
            "--isolated",
            "--color=never",
            "--jobs=1",
            "--format",
            "%(path)s:%(row)s:%(col)s:%(code)s:%(text)s",
            "/scratch/a.py"
        ]
    );
    assert_eq!(invocation.cwd_rel, "");
}

#[test]
fn pylint_check_is_json_without_cache_or_report() {
    let file = Path::new("/scratch/a.py");
    let invocation = pylint_check(Path::new(BIN), &[file]);
    assert_eq!(
        argv_strings(&invocation),
        vec![
            BIN,
            "--persistent=n",
            "--reports=n",
            "--score=n",
            "--output-format=json",
            "--jobs=1",
            "/scratch/a.py"
        ]
    );
    assert_eq!(invocation.cwd_rel, "");
}

#[test]
fn biome_lint_check_is_json_hermetic() {
    let file = Path::new("/scratch/src/a.js");
    let config = Path::new("/scratch/cfg");
    let invocation = biome_lint_check(Path::new(BIN), &[file], config);
    assert_eq!(
        argv_strings(&invocation),
        vec![
            BIN,
            "lint",
            "--reporter=json",
            "--colors=off",
            "--config-path",
            "/scratch/cfg",
            "--error-on-warnings",
            "--vcs-enabled=false",
            "/scratch/src/a.js"
        ]
    );
    assert_eq!(invocation.cwd_rel, "");
}

#[test]
fn biome_format_check_and_fix_share_config() {
    let file = Path::new("/scratch/src/a.js");
    let config = Path::new("/scratch/cfg");
    let check = biome_format_check(Path::new(BIN), &[file], config);
    assert_eq!(
        argv_strings(&check),
        vec![
            BIN,
            "format",
            "--reporter=json",
            "--colors=off",
            "--config-path",
            "/scratch/cfg",
            "/scratch/src/a.js"
        ]
    );
    assert_eq!(check.cwd_rel, "");
    let fix = biome_format_fix(Path::new(BIN), &[file], config);
    assert_eq!(
        argv_strings(&fix),
        vec![
            BIN,
            "format",
            "--reporter=json",
            "--colors=off",
            "--config-path",
            "/scratch/cfg",
            "--write",
            "/scratch/src/a.js"
        ]
    );
    assert_eq!(fix.cwd_rel, "");
}

#[test]
fn eslint_check_and_fix_share_config() {
    let launch = ResolvedTool::javascript("eslint", Path::new(BIN));
    let file = Path::new("/scratch/src/a.js");
    let config = Path::new("/scratch/cfg/eslint.config.js");
    let check = eslint_check(&launch, &[file], config).expect("resolves");
    assert_eq!(
        argv_strings(&check),
        vec![
            BIN,
            "-c",
            "/scratch/cfg/eslint.config.js",
            "-f",
            "json",
            "/scratch/src/a.js"
        ]
    );
    assert_eq!(check.cwd_rel, "");
    let fix = eslint_fix(&launch, &[file], config).expect("resolves");
    assert_eq!(
        argv_strings(&fix),
        vec![
            BIN,
            "-c",
            "/scratch/cfg/eslint.config.js",
            "-f",
            "json",
            "--fix",
            "/scratch/src/a.js"
        ]
    );
    assert_eq!(fix.cwd_rel, "");
}

#[test]
fn eslint_without_a_program_fails_before_any_spawn() {
    let launch = ResolvedTool::javascript("eslint", Path::new(""));
    let error = eslint_check(&launch, &[], Path::new("/scratch/cfg/eslint.config.js"))
        .expect_err("no program");
    assert_eq!(
        error,
        LaunchError::MissingProgram {
            tool: "eslint".to_owned()
        }
    );
}

#[test]
fn prettier_check_and_fix_are_hermetic() {
    let launch = ResolvedTool::javascript("prettier", Path::new(BIN));
    let file = Path::new("/scratch/src/a.js");
    let check = prettier_check(&launch, &[file]).expect("resolves");
    assert_eq!(
        argv_strings(&check),
        vec![
            BIN,
            "--no-config",
            "--no-editorconfig",
            "--check",
            "/scratch/src/a.js"
        ]
    );
    assert_eq!(check.cwd_rel, "");
    let fix = prettier_fix(&launch, &[file]).expect("resolves");
    assert_eq!(
        argv_strings(&fix),
        vec![
            BIN,
            "--no-config",
            "--no-editorconfig",
            "--write",
            "/scratch/src/a.js"
        ]
    );
    assert_eq!(fix.cwd_rel, "");
}

#[test]
fn error_prone_check_is_plugin_only() {
    let file = Path::new("/scratch/src/Hello.java");
    let invocation = error_prone_check(Path::new(BIN), &[file]);
    assert_eq!(
        argv_strings(&invocation),
        vec![BIN, "-Xplugin:ErrorProne", "/scratch/src/Hello.java"]
    );
    assert_eq!(invocation.cwd_rel, "");
}

#[test]
fn error_prone_patch_declares_a_directory_never_in_place() {
    let file = Path::new("/scratch/src/Hello.java");
    let dir = Path::new("/scratch/patch-out");
    let invocation = error_prone_patch(
        Path::new(BIN),
        &[file],
        dir,
        "MissingOverride,DefaultCharset",
    );
    let argv = argv_strings(&invocation);
    assert_eq!(
        argv,
        vec![
            BIN,
            "-Xplugin:ErrorProne",
            "-XepPatchChecks:MissingOverride,DefaultCharset",
            "-XepPatchLocation:/scratch/patch-out",
            "/scratch/src/Hello.java"
        ]
    );
    assert_eq!(invocation.cwd_rel, "");
    assert!(
        !argv.iter().any(|arg| arg.contains("IN_PLACE")),
        "patch location is a declared dir, never IN_PLACE",
    );
    assert_eq!(ERROR_PRONE_PATCH_FILE, "error-prone.patch");
}

#[test]
fn scalafmt_check_and_fix_share_config() {
    let file = Path::new("/scratch/Sample.scala");
    let check = scalafmt_check(Path::new(BIN), &[file], None);
    assert_eq!(
        argv_strings(&check),
        vec![BIN, "--check", "/scratch/Sample.scala"]
    );
    assert_eq!(check.cwd_rel, "");
    let hinted = scalafmt_check(
        Path::new(BIN),
        &[file],
        Some(Path::new("/scratch/.scalafmt.conf")),
    );
    assert!(argv_strings(&hinted).contains(&"--config".to_owned()));
    let fix = scalafmt_fix(Path::new(BIN), &[file], None);
    assert!(!argv_strings(&fix).contains(&"--check".to_owned()));
}

#[test]
fn scalafix_check_carries_target_coupled_wiring() {
    let file = Path::new("/scratch/Sample.scala");
    let bare = scalafix_check(Path::new(BIN), &[file], None, None, None);
    assert_eq!(argv_strings(&bare), vec![BIN, "/scratch/Sample.scala"]);
    let wired = scalafix_check(
        Path::new(BIN),
        &[file],
        Some(Path::new("/scratch/root")),
        Some("/scratch/cp.jar"),
        Some(Path::new("/scratch/semanticdb")),
    );
    let argv = argv_strings(&wired);
    assert!(argv.contains(&"--sourceroot".to_owned()));
    assert!(argv.contains(&"--classpath".to_owned()));
    assert!(argv.contains(&"--semanticdb-targetroots".to_owned()));
    assert!(!argv.iter().any(|arg| arg.contains("IN_PLACE")));
}

#[test]
fn csharpier_check_and_fix_share_config() {
    let file = Path::new("/scratch/Sample.cs");
    let check = csharpier_check(Path::new(BIN), &[file], None);
    assert_eq!(
        argv_strings(&check),
        vec![BIN, "check", "/scratch/Sample.cs"]
    );
    let hinted = csharpier_check(
        Path::new(BIN),
        &[file],
        Some(Path::new("/scratch/.csharpierrc")),
    );
    assert!(argv_strings(&hinted).contains(&"--config-path".to_owned()));
    let fix = csharpier_fix(Path::new(BIN), &[file], None);
    assert_eq!(
        argv_strings(&fix),
        vec![BIN, "format", "/scratch/Sample.cs"]
    );
}

#[test]
fn fantomas_check_is_json_and_fix_is_bare() {
    let file = Path::new("/scratch/Sample.fs");
    let check = fantomas_check(Path::new(BIN), &[file]);
    assert_eq!(
        argv_strings(&check),
        vec![BIN, "check", "--json", "/scratch/Sample.fs"]
    );
    let fix = fantomas_fix(Path::new(BIN), &[file]);
    assert_eq!(argv_strings(&fix), vec![BIN, "/scratch/Sample.fs"]);
}

#[test]
fn fsharplint_check_carries_project_and_config() {
    let file = Path::new("/scratch/Sample.fs");
    let bare = fsharplint_check(Path::new(BIN), &[file], None, None);
    assert_eq!(argv_strings(&bare), vec![BIN, "/scratch/Sample.fs"]);
    let wired = fsharplint_check(
        Path::new(BIN),
        &[file],
        Some(Path::new("/scratch/Sample.fsproj")),
        Some(Path::new("/scratch/fsharplint.json")),
    );
    let argv = argv_strings(&wired);
    assert!(argv.contains(&"--project".to_owned()));
    assert!(argv.contains(&"--config".to_owned()));
}

#[test]
fn jvm_format_checks_list_paths_and_fixes_rewrite() {
    let java = Path::new("/scratch/src/Hello.java");
    let check = google_java_format_check(Path::new(BIN), &[java]);
    assert_eq!(
        argv_strings(&check),
        vec![
            BIN,
            "--dry-run",
            "--set-exit-if-changed",
            "/scratch/src/Hello.java"
        ]
    );
    assert_eq!(check.cwd_rel, "");
    let fix = google_java_format_fix(Path::new(BIN), &[java]);
    assert_eq!(
        argv_strings(&fix),
        vec![BIN, "--replace", "/scratch/src/Hello.java"]
    );
    assert_eq!(fix.cwd_rel, "");
    let kt = Path::new("/scratch/src/Hello.kt");
    let check = ktfmt_check(Path::new(BIN), &[kt]);
    assert_eq!(
        argv_strings(&check),
        vec![
            BIN,
            "--kotlinlang-style",
            "--dry-run",
            "--set-exit-if-changed",
            "/scratch/src/Hello.kt"
        ]
    );
    assert_eq!(check.cwd_rel, "");
    let fix = ktfmt_fix(Path::new(BIN), &[kt]);
    assert_eq!(
        argv_strings(&fix),
        vec![BIN, "--kotlinlang-style", "/scratch/src/Hello.kt"]
    );
    assert_eq!(fix.cwd_rel, "");
}

#[test]
fn jvm_lint_checks_use_sarif_with_explicit_config() {
    let java = Path::new("/scratch/src/Hello.java");
    let cfg = Path::new("/scratch/checkstyle.xml");
    let check = checkstyle_check(Path::new(BIN), &[java], cfg);
    assert_eq!(
        argv_strings(&check),
        vec![
            BIN,
            "-c",
            "/scratch/checkstyle.xml",
            "-f",
            "sarif",
            "/scratch/src/Hello.java"
        ]
    );
    assert_eq!(check.cwd_rel, "");
    let bare = pmd_check(Path::new(BIN), &[java], None);
    assert_eq!(
        argv_strings(&bare),
        vec![
            BIN,
            "check",
            "--dir",
            "/scratch/src/Hello.java",
            "--format",
            "sarif",
            "--rulesets",
            "rulesets/java/quickstart.xml"
        ]
    );
    let hinted = pmd_check(Path::new(BIN), &[java], Some(cfg));
    assert!(argv_strings(&hinted).contains(&"--rulesets".to_owned()));
    let spot = spotbugs_check(Path::new(BIN), &[java]);
    assert_eq!(
        argv_strings(&spot),
        vec![
            BIN,
            "-textui",
            "-effort:default",
            "-sarif",
            "/scratch/src/Hello.java"
        ]
    );
    let kt = Path::new("/scratch/src/Hello.kt");
    let check = ktlint_check(Path::new(BIN), &[kt]);
    assert_eq!(
        argv_strings(&check),
        vec![
            BIN,
            "--relative",
            "--log-level=none",
            "--reporter=sarif",
            "/scratch/src/Hello.kt"
        ]
    );
    let fix = ktlint_fix(Path::new(BIN), &[kt]);
    assert_eq!(
        argv_strings(&fix),
        vec![BIN, "--relative", "--format", "/scratch/src/Hello.kt"]
    );
}

#[test]
fn clang_format_check_and_fix_share_style() {
    let file = Path::new("/scratch/Sample.c");
    let check = clang_format_check(Path::new(BIN), &[file], None);
    assert_eq!(
        argv_strings(&check),
        vec![BIN, "--dry-run", "--Werror", "/scratch/Sample.c"]
    );
    assert_eq!(check.cwd_rel, "");
    let hinted = clang_format_check(
        Path::new(BIN),
        &[file],
        Some(Path::new("/scratch/.clang-format")),
    );
    assert!(
        argv_strings(&hinted)
            .iter()
            .any(|arg| arg.starts_with("--style=file:")),
        "hinted config rides --style=file:"
    );
    let fix = clang_format_fix(Path::new(BIN), &[file], None);
    assert_eq!(argv_strings(&fix), vec![BIN, "-i", "/scratch/Sample.c"]);
}

#[test]
fn gofumpt_check_is_diff_and_fix_is_write() {
    let file = Path::new("/scratch/Sample.go");
    let check = gofumpt_check(Path::new(BIN), &[file]);
    assert_eq!(argv_strings(&check), vec![BIN, "-d", "/scratch/Sample.go"]);
    assert_eq!(check.cwd_rel, "");
    let fix = gofumpt_fix(Path::new(BIN), &[file]);
    assert_eq!(argv_strings(&fix), vec![BIN, "-w", "/scratch/Sample.go"]);
}

#[test]
fn clang_tidy_check_carries_config_and_compile_commands() {
    let file = Path::new("/scratch/Sample.c");
    let bare = clang_tidy_check(Path::new(BIN), &[file], None, None);
    assert_eq!(
        argv_strings(&bare),
        vec![BIN, "--quiet", "/scratch/Sample.c"]
    );
    let wired = clang_tidy_check(
        Path::new(BIN),
        &[file],
        Some(Path::new("/scratch/.clang-tidy")),
        Some(Path::new("/scratch/compile-commands")),
    );
    let argv = argv_strings(&wired);
    assert!(argv.contains(&"--config-file".to_owned()));
    assert!(argv.contains(&"-p".to_owned()));
    assert!(!argv.iter().any(|arg| arg.contains("--fix")));
    assert!(!argv.iter().any(|arg| arg.contains("--export-fixes")));
}

#[test]
fn cppcheck_check_is_xml_with_optional_suppressions() {
    let file = Path::new("/scratch/Sample.c");
    let bare = cppcheck_check(Path::new(BIN), &[file], None);
    assert_eq!(
        argv_strings(&bare),
        vec![BIN, "--xml", "--xml-version=2", "/scratch/Sample.c"]
    );
    assert_eq!(bare.cwd_rel, "");
    let hinted = cppcheck_check(
        Path::new(BIN),
        &[file],
        Some(Path::new("/scratch/suppressions.txt")),
    );
    assert!(
        argv_strings(&hinted)
            .iter()
            .any(|arg| arg.starts_with("--suppressions-list=")),
        "hinted suppressions ride --suppressions-list="
    );
    assert!(!argv_strings(&hinted).contains(&"--enable=all".to_owned()));
}

#[test]
fn staticcheck_check_is_json_with_config_dir_cwd() {
    let file = Path::new("/scratch/Sample.go");
    let bare = staticcheck_check(Path::new(BIN), &[file], None);
    assert_eq!(
        argv_strings(&bare),
        vec![BIN, "-f", "json", "/scratch/Sample.go"]
    );
    assert_eq!(bare.cwd_rel, "");
    let hinted = staticcheck_check(Path::new(BIN), &[file], Some("cfg"));
    assert_eq!(
        argv_strings(&hinted),
        vec![BIN, "-f", "json", "/scratch/Sample.go"]
    );
    assert_eq!(hinted.cwd_rel, "cfg");
}

#[test]
fn govet_and_errcheck_take_plain_file_lists() {
    let file = Path::new("/scratch/Sample.go");
    let govet = govet_check(Path::new(BIN), &[file]);
    assert_eq!(argv_strings(&govet), vec![BIN, "/scratch/Sample.go"]);
    assert_eq!(govet.cwd_rel, "");
    let errcheck = errcheck_check(Path::new(BIN), &[file]);
    assert_eq!(argv_strings(&errcheck), vec![BIN, "/scratch/Sample.go"]);
    assert_eq!(errcheck.cwd_rel, "");
}

#[test]
fn structured_checks_encode_pinned_shapes() {
    let proto = Path::new("/scratch/proto/hello.proto");
    let lint = buf_lint_check(Path::new(BIN), &[proto]);
    assert_eq!(
        argv_strings(&lint),
        vec![
            BIN,
            "lint",
            "--error-format=json",
            "/scratch/proto/hello.proto"
        ]
    );
    assert_eq!(lint.cwd_rel, "");
    let check = buf_format_check(Path::new(BIN), &[proto]);
    assert_eq!(
        argv_strings(&check),
        vec![
            BIN,
            "format",
            "--diff",
            "--exit-code",
            "/scratch/proto/hello.proto"
        ]
    );
    let fix = buf_format_fix(Path::new(BIN), &[proto]);
    assert_eq!(
        argv_strings(&fix),
        vec![BIN, "format", "--write", "/scratch/proto/hello.proto"]
    );
    let qml = Path::new("/scratch/qml/Main.qml");
    let check = qmlformat_check(Path::new(BIN), &[qml]);
    assert_eq!(
        argv_strings(&check),
        vec![BIN, "--check", "/scratch/qml/Main.qml"]
    );
    let fix = qmlformat_fix(Path::new(BIN), &[qml]);
    assert_eq!(argv_strings(&fix), vec![BIN, "-i", "/scratch/qml/Main.qml"]);
    let lint = qmllint_check(Path::new(BIN), &[qml]);
    assert_eq!(
        argv_strings(&lint),
        vec![BIN, "--json", "-", "/scratch/qml/Main.qml"]
    );
}

#[test]
fn file_family_format_checks_and_fixes_use_each_tool_own_flags() {
    let cue_file = Path::new("/scratch/Sample.cue");
    assert_eq!(
        argv_strings(&cue_check(Path::new(BIN), &[cue_file])),
        vec![BIN, "fmt", "--check", "--diff", "/scratch/Sample.cue"]
    );
    assert_eq!(
        argv_strings(&cue_fix(Path::new(BIN), &[cue_file])),
        vec![BIN, "fmt", "--write", "/scratch/Sample.cue"]
    );
    let mod_file = Path::new("/scratch/go.mod");
    assert_eq!(
        argv_strings(&modfmt_check(Path::new(BIN), &[mod_file])),
        vec![BIN, "-c", "-l", "/scratch/go.mod"]
    );
    assert_eq!(
        argv_strings(&modfmt_fix(Path::new(BIN), &[mod_file])),
        vec![BIN, "-w", "/scratch/go.mod"]
    );
    let tf = Path::new("/scratch/main.tf");
    assert_eq!(
        argv_strings(&terraform_check(Path::new(BIN), &[tf])),
        vec![BIN, "fmt", "-check", "/scratch/main.tf"]
    );
    assert_eq!(
        argv_strings(&terraform_fix(Path::new(BIN), &[tf])),
        vec![BIN, "fmt", "/scratch/main.tf"]
    );
    let sh = Path::new("/scratch/run.sh");
    assert_eq!(
        argv_strings(&shfmt_check(Path::new(BIN), &[sh])),
        vec![BIN, "-d", "/scratch/run.sh"]
    );
    assert_eq!(
        argv_strings(&shfmt_fix(Path::new(BIN), &[sh])),
        vec![BIN, "-w", "/scratch/run.sh"]
    );
    let rb = Path::new("/scratch/Sample.rb");
    assert_eq!(
        argv_strings(&standardrb_check(Path::new(BIN), &[rb])),
        vec![BIN, "--format", "json", "/scratch/Sample.rb"]
    );
    assert_eq!(
        argv_strings(&standardrb_fix(Path::new(BIN), &[rb])),
        vec![BIN, "--fix", "/scratch/Sample.rb"]
    );
    let jsonnet = Path::new("/scratch/Sample.jsonnet");
    assert_eq!(
        argv_strings(&jsonnetfmt_check(Path::new(BIN), &[jsonnet])),
        vec![BIN, "--test", "/scratch/Sample.jsonnet"]
    );
    assert_eq!(
        argv_strings(&jsonnetfmt_fix(Path::new(BIN), &[jsonnet])),
        vec![BIN, "-i", "/scratch/Sample.jsonnet"]
    );
    let pkl = Path::new("/scratch/Sample.pkl");
    assert_eq!(
        argv_strings(&pkl_check(Path::new(BIN), &[pkl])),
        vec![BIN, "format", "--diff-name-only", "/scratch/Sample.pkl"]
    );
    assert_eq!(
        argv_strings(&pkl_fix(Path::new(BIN), &[pkl])),
        vec![BIN, "format", "-w", "/scratch/Sample.pkl"]
    );
    let yaml = Path::new("/scratch/Sample.yaml");
    assert_eq!(
        argv_strings(&yamlfmt_check(Path::new(BIN), &[yaml])),
        vec![BIN, "-lint", "-q", "/scratch/Sample.yaml"]
    );
    assert_eq!(
        argv_strings(&yamlfmt_fix(Path::new(BIN), &[yaml])),
        vec![BIN, "/scratch/Sample.yaml"]
    );
}

#[test]
fn file_family_lint_checks_are_check_only() {
    let css = Path::new("/scratch/style.css");
    let bare = stylelint_check(Path::new(BIN), &[css], None);
    assert_eq!(
        argv_strings(&bare),
        vec![BIN, "--formatter", "json", "/scratch/style.css"]
    );
    let hinted = stylelint_check(
        Path::new(BIN),
        &[css],
        Some(Path::new("/scratch/.stylelintrc.json")),
    );
    assert!(argv_strings(&hinted).contains(&"--config".to_owned()));
    let rb = Path::new("/scratch/Sample.rb");
    assert_eq!(
        argv_strings(&rubocop_check(Path::new(BIN), &[rb])),
        vec![BIN, "--format", "json", "/scratch/Sample.rb"]
    );
    let ps = Path::new("/scratch/Sample.ps1");
    assert_eq!(
        argv_strings(&psscriptanalyzer_check(Path::new(BIN), &[ps])),
        vec![BIN, "/scratch/Sample.ps1"]
    );
    let sh = Path::new("/scratch/run.sh");
    assert_eq!(
        argv_strings(&shellcheck_check(Path::new(BIN), &[sh])),
        vec![BIN, "--format=gcc", "/scratch/run.sh"]
    );
    let txt = Path::new("/scratch/notes.txt");
    assert_eq!(
        argv_strings(&keep_sorted_check(Path::new(BIN), &[txt])),
        vec![BIN, "--mode", "lint", "/scratch/notes.txt"]
    );
    assert_eq!(
        argv_strings(&keep_sorted_fix(Path::new(BIN), &[txt])),
        vec![BIN, "--mode", "fix", "/scratch/notes.txt"]
    );
}

#[test]
fn fixed_args_match_recorded_invocations() {
    let file = Path::new(FILE);
    let cases: &[(&[&str], Invocation)] = &[
        (GOFUMPT_CHECK_ARGS, gofumpt_check(Path::new(BIN), &[file])),
        (
            GOOGLE_JAVA_FORMAT_CHECK_ARGS,
            google_java_format_check(Path::new(BIN), &[file]),
        ),
        (KTLINT_CHECK_ARGS, ktlint_check(Path::new(BIN), &[file])),
        (BUF_LINT_ARGS, buf_lint_check(Path::new(BIN), &[file])),
        (
            DJLINT_FORMAT_CHECK_ARGS,
            djlint_format_check(Path::new(BIN), &[file]),
        ),
        (RUBOCOP_ARGS, rubocop_check(Path::new(BIN), &[file])),
        (SHELLCHECK_ARGS, shellcheck_check(Path::new(BIN), &[file])),
        (
            KEEP_SORTED_CHECK_ARGS,
            keep_sorted_check(Path::new(BIN), &[file]),
        ),
        (
            KEEP_SORTED_FIX_ARGS,
            keep_sorted_fix(Path::new(BIN), &[file]),
        ),
    ];
    for (args, invocation) in cases {
        let mut expected = vec![BIN.to_owned()];
        expected.extend(args.iter().map(|arg| arg.to_string()));
        expected.push(FILE.to_owned());
        assert_eq!(argv_strings(invocation), expected);
        assert_eq!(invocation.cwd_rel, "");
    }
}
