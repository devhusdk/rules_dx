use super::*;
use std::path::PathBuf;

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
            "--config",
            "skip_children=true",
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
            "--config",
            "src = [\"/scratch\"]",
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
            "--config",
            "src = [\"/scratch\"]",
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
    let check = prettier_check(&launch, &[file], None, None).expect("resolves");
    assert_eq!(
        argv_strings(&check),
        vec![BIN, "--no-editorconfig", "--check", "/scratch/src/a.js"]
    );
    assert_eq!(check.cwd_rel, "");
    let fix = prettier_fix(&launch, &[file], None, None).expect("resolves");
    assert_eq!(
        argv_strings(&fix),
        vec![BIN, "--no-editorconfig", "--write", "/scratch/src/a.js"]
    );
    assert_eq!(fix.cwd_rel, "");
}

#[test]
fn prettier_sql_dialect_is_one_dx_private_flag() {
    let launch = ResolvedTool::javascript("prettier", Path::new(BIN));
    let file = Path::new("/scratch/src/a.sql");
    let check = prettier_check(&launch, &[file], None, Some("postgresql")).expect("resolves");
    let fix = prettier_fix(&launch, &[file], None, Some("postgresql")).expect("resolves");
    for (invocation, mode) in [(&check, "--check"), (&fix, "--write")] {
        assert_eq!(
            argv_strings(invocation),
            vec![
                BIN,
                "--dx-sql-dialect=postgresql",
                "--no-editorconfig",
                mode,
                "/scratch/src/a.sql"
            ]
        );
    }
}

#[test]
fn prettier_bound_config_is_one_dx_private_flag() {
    let launch = ResolvedTool::javascript("prettier", Path::new(BIN));
    let file = Path::new("/scratch/src/a.js");
    let cfg = Path::new("/scratch/cfg/.prettierrc.json");
    let check = prettier_check(&launch, &[file], Some(cfg), None).expect("resolves");
    let fix = prettier_fix(&launch, &[file], Some(cfg), None).expect("resolves");
    for (invocation, mode) in [(&check, "--check"), (&fix, "--write")] {
        let argv = argv_strings(invocation);
        assert_eq!(
            argv,
            vec![
                BIN,
                "--dx-config=/scratch/cfg/.prettierrc.json",
                "--no-editorconfig",
                mode,
                "/scratch/src/a.js"
            ]
        );
        assert_eq!(
            argv.iter()
                .filter(|arg| arg.starts_with("--dx-config="))
                .count(),
            1
        );
        for reserved in ["--config", "--no-config", "--ignore-path"] {
            assert!(
                !argv
                    .iter()
                    .any(|arg| arg == reserved || arg.starts_with(&format!("{reserved}="))),
                "{reserved} is owned by the shim"
            );
        }
    }
}

#[test]
fn remark_check_and_fix_pass_settings_only_when_bound() {
    let launch = ResolvedTool::javascript("remark", Path::new(BIN));
    let file = Path::new("/scratch/docs/a.mdx");
    let check = remark_check(&launch, &[file], None).expect("resolves");
    assert_eq!(
        argv_strings(&check),
        vec![BIN, "--check", "/scratch/docs/a.mdx"]
    );
    assert_eq!(check.cwd_rel, "");
    let settings = Path::new("/scratch/remark.json");
    let fix = remark_fix(&launch, &[file], Some(settings)).expect("resolves");
    assert_eq!(
        argv_strings(&fix),
        vec![
            BIN,
            "--settings",
            "/scratch/remark.json",
            "--write",
            "/scratch/docs/a.mdx"
        ]
    );
    let blank = ResolvedTool::javascript("remark", Path::new(""));
    assert_eq!(
        remark_check(&blank, &[file], None),
        Err(LaunchError::MissingProgram {
            tool: "remark".to_owned()
        })
    );
}

#[test]
fn scalafmt_check_and_fix_share_config() {
    let file = Path::new("/scratch/Sample.scala");
    let check = scalafmt_check(Path::new(BIN), &[file], None);
    assert_eq!(
        argv_strings(&check),
        vec![BIN, "--list", "/scratch/Sample.scala"]
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
fn scalafix_check_and_fix_carry_the_semantic_context() {
    let file = Path::new("/scratch/Sample.scala");
    let jars = [Path::new("/scratch/own.jar"), Path::new("/scratch/dep.jar")];
    let context = ScalafixContext {
        config: Path::new("/scratch/.scalafix.conf"),
        sourceroot: Path::new("/scratch"),
        classpath: &jars,
    };
    let check = scalafix_check(Path::new(BIN), &[file], &context).expect("check");
    let joined = std::env::join_paths(jars).expect("join");
    assert_eq!(
        argv_strings(&check),
        vec![
            BIN,
            "--check",
            "--no-stale-semanticdb",
            "--config",
            "/scratch/.scalafix.conf",
            "--sourceroot",
            "/scratch",
            "--classpath",
            joined.to_str().expect("utf8"),
            "/scratch/Sample.scala",
        ]
    );
    let fix = scalafix_fix(Path::new(BIN), &[file], &context).expect("fix");
    let argv = argv_strings(&fix);
    assert!(!argv.contains(&"--check".to_owned()));
    assert_eq!(
        argv.last().map(String::as_str),
        Some("/scratch/Sample.scala")
    );
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
    for flag in ["--no-cache", "--use-cache"] {
        assert!(!argv_strings(&check).contains(&flag.to_owned()));
    }
    let fix = csharpier_fix(Path::new(BIN), &[file], None);
    assert_eq!(
        argv_strings(&fix),
        vec![BIN, "format", "--no-cache", "/scratch/Sample.cs"]
    );
    let fix_hinted = csharpier_fix(
        Path::new(BIN),
        &[file],
        Some(Path::new("/scratch/.csharpierrc")),
    );
    assert_eq!(
        argv_strings(&fix_hinted),
        vec![
            BIN,
            "format",
            "--no-cache",
            "--config-path",
            "/scratch/.csharpierrc",
            "/scratch/Sample.cs"
        ]
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
fn fsharplint_check_carries_compile_context_and_config() {
    let file = Path::new("/scratch/Sample.fs");
    let first = Path::new("/scratch/A.fs");
    let reference = Path::new("/scratch/ref/Core.dll");
    let defines = vec!["DEBUG".to_owned()];
    let context = FsharplintContext {
        compile: &[first, file],
        refs: &[reference],
        defines: &defines,
        langversion: "8.0",
    };
    let argv = argv_strings(&fsharplint_check(
        Path::new(BIN),
        &[file],
        &context,
        Some(Path::new("/scratch/fsharplint.json")),
    ));
    assert_eq!(
        argv,
        vec![
            BIN,
            "--compile",
            "/scratch/A.fs",
            "--compile",
            "/scratch/Sample.fs",
            "--ref",
            "/scratch/ref/Core.dll",
            "--define",
            "DEBUG",
            "--langversion",
            "8.0",
            "--config",
            "/scratch/fsharplint.json",
            "/scratch/Sample.fs",
        ]
    );
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
    let pmd_cfg = Path::new("/scratch/pmd.xml");
    let hinted = pmd_check(Path::new(BIN), &[java], Some(pmd_cfg));
    assert_eq!(
        argv_strings(&hinted),
        vec![
            BIN,
            "check",
            "--dir",
            "/scratch/src/Hello.java",
            "--format",
            "sarif",
            "--rulesets",
            "/scratch/pmd.xml"
        ]
    );
    assert!(!argv_strings(&hinted).contains(&"rulesets/java/quickstart.xml".to_owned()));
    let spot = spotbugs_check(Path::new(BIN), &[java], &[]).expect("spot");
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
    let with_aux = spotbugs_check(
        Path::new(BIN),
        &[java],
        &[Path::new("/scratch/a.jar"), Path::new("/scratch/b.jar")],
    )
    .expect("with_aux");
    let joined_aux = std::env::join_paths(["/scratch/a.jar", "/scratch/b.jar"]).expect("join");
    let joined_aux = joined_aux.to_str().expect("utf8").to_owned();
    assert_eq!(
        argv_strings(&with_aux),
        vec![
            BIN,
            "-textui",
            "-effort:default",
            "-sarif",
            "-auxclasspath",
            &joined_aux,
            "/scratch/src/Hello.java"
        ]
    );
    let kt = Path::new("/scratch/src/Hello.kt");
    let editorconfig = Path::new("/scratch/dx-ktlint-default/.editorconfig");
    let check = ktlint_check(Path::new(BIN), &[kt], Some(editorconfig));
    assert_eq!(
        argv_strings(&check),
        vec![
            BIN,
            "--relative",
            "--log-level=none",
            "--reporter=sarif",
            "--editorconfig",
            "/scratch/dx-ktlint-default/.editorconfig",
            "/scratch/src/Hello.kt"
        ]
    );
    let fix = ktlint_fix(Path::new(BIN), &[kt], Some(editorconfig));
    assert_eq!(
        argv_strings(&fix),
        vec![
            BIN,
            "--relative",
            "--format",
            "--editorconfig",
            "/scratch/dx-ktlint-default/.editorconfig",
            "/scratch/src/Hello.kt"
        ]
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
    let check = gofumpt_check(Path::new(BIN), &[file], "go1.26");
    assert_eq!(
        argv_strings(&check),
        vec![BIN, "-lang=go1.26", "-d", "/scratch/Sample.go"]
    );
    assert_eq!(check.cwd_rel, "");
    let fix = gofumpt_fix(Path::new(BIN), &[file], "go1.26");
    assert_eq!(
        argv_strings(&fix),
        vec![BIN, "-lang=go1.26", "-w", "/scratch/Sample.go"]
    );
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
fn structured_checks_encode_pinned_shapes() {
    let proto = Path::new("/scratch/proto/hello.proto");
    let root = Path::new("/scratch");
    let second = Path::new("/scratch/common/types.proto");
    let lint = buf_lint_check(Path::new(BIN), &[proto, second], root, None);
    assert_eq!(
        argv_strings(&lint),
        vec![
            BIN,
            "lint",
            "--error-format=json",
            "--path",
            "/scratch/proto/hello.proto",
            "--path",
            "/scratch/common/types.proto",
            "/scratch"
        ]
    );
    assert_eq!(lint.cwd_rel, "");
    let config = Path::new("/scratch/buf.yaml");
    let check = buf_format_check(Path::new(BIN), &[proto], root, Some(config));
    assert_eq!(
        argv_strings(&check),
        vec![
            BIN,
            "format",
            "--config",
            "/scratch/buf.yaml",
            "--path",
            "/scratch/proto/hello.proto",
            "/scratch"
        ]
    );
    let fix = buf_format_fix(Path::new(BIN), &[proto], root);
    assert_eq!(
        argv_strings(&fix),
        vec![
            BIN,
            "format",
            "--write",
            "--path",
            "/scratch/proto/hello.proto",
            "/scratch"
        ]
    );
    let qml = Path::new("/scratch/qml/Main.qml");
    let check = qmlformat_check(Path::new(BIN), &[qml], None);
    assert_eq!(argv_strings(&check), vec![BIN, "/scratch/qml/Main.qml"]);
    let fix = qmlformat_fix(Path::new(BIN), &[qml], None);
    assert_eq!(argv_strings(&fix), vec![BIN, "-i", "/scratch/qml/Main.qml"]);
    let config = Path::new("/scratch/qml/.qmlformat.ini");
    let check = qmlformat_check(Path::new(BIN), &[qml], Some(config));
    assert_eq!(
        argv_strings(&check),
        vec![
            BIN,
            "-s",
            "/scratch/qml/.qmlformat.ini",
            "/scratch/qml/Main.qml"
        ]
    );
    let lint = qmllint_check(Path::new(BIN), &[qml], &[], true);
    assert_eq!(
        argv_strings(&lint),
        vec![BIN, "-W", "0", "--json", "-", "/scratch/qml/Main.qml"]
    );
    let isolated = qmllint_check(
        Path::new(BIN),
        &[qml],
        &[Path::new("/scratch/mod"), Path::new("/scratch/lib")],
        false,
    );
    assert_eq!(
        argv_strings(&isolated),
        vec![
            BIN,
            "-W",
            "0",
            "--json",
            "-",
            "--ignore-settings",
            "-I",
            "/scratch/mod",
            "-I",
            "/scratch/lib",
            "/scratch/qml/Main.qml"
        ]
    );
}

#[test]
fn cue_vet_is_non_concrete_and_runs_from_the_module_root() {
    let file = Path::new("/scratch/app/main.cue");
    let inv = cue_vet(Path::new(BIN), &[file], "app");
    assert_eq!(inv.cwd_rel, "app");
    assert_eq!(
        argv_strings(&inv),
        vec![BIN, "vet", "-c=false", "-E", "/scratch/app/main.cue"]
    );
}

#[test]
fn pkl_eval_is_offline_sealed_and_per_module() {
    let module = Path::new("/scratch/app/main.pkl");
    let root = Path::new("/scratch");
    let full = pkl_eval(Path::new(BIN), module, root, false);
    assert_eq!(full.cwd_rel, "");
    assert_eq!(
        argv_strings(&full),
        vec![
            BIN,
            "eval",
            "--no-cache",
            "--no-project",
            "--allowed-modules",
            "pkl:,file:",
            "--allowed-resources",
            "prop:,env:,file:",
            "--root-dir",
            "/scratch",
            "/scratch/app/main.pkl",
        ]
    );
    let load = pkl_eval(Path::new(BIN), module, root, true);
    assert_eq!(
        argv_strings(&load),
        vec![
            BIN,
            "eval",
            "--no-cache",
            "--no-project",
            "--allowed-modules",
            "pkl:,file:,repl:",
            "--allowed-resources",
            "prop:,env:,file:",
            "--root-dir",
            "/scratch",
            "-x",
            "1",
            "/scratch/app/main.pkl",
        ]
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
        vec![BIN, "fmt", "/scratch/Sample.cue"]
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
        vec![BIN, "fmt", "-check", "-no-color", "/scratch/main.tf"]
    );
    assert_eq!(
        argv_strings(&terraform_fix(Path::new(BIN), &[tf])),
        vec![BIN, "fmt", "-no-color", "/scratch/main.tf"]
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
        argv_strings(&standardrb_check(Path::new(BIN), &[rb], None)),
        vec![
            BIN,
            "--format",
            "json",
            "--cache",
            "false",
            "/scratch/Sample.rb"
        ]
    );
    assert_eq!(
        argv_strings(&standardrb_fix(Path::new(BIN), &[rb], None)),
        vec![BIN, "--fix", "--cache", "false", "/scratch/Sample.rb"]
    );
    let standard_config = Path::new("/scratch/standard.yml");
    assert_eq!(
        argv_strings(&standardrb_check(
            Path::new(BIN),
            &[rb],
            Some(standard_config)
        )),
        vec![
            BIN,
            "--format",
            "json",
            "--cache",
            "false",
            "--config",
            "/scratch/standard.yml",
            "/scratch/Sample.rb"
        ]
    );
    assert_eq!(
        argv_strings(&standardrb_fix(
            Path::new(BIN),
            &[rb],
            Some(standard_config)
        )),
        vec![
            BIN,
            "--fix",
            "--cache",
            "false",
            "--config",
            "/scratch/standard.yml",
            "/scratch/Sample.rb"
        ]
    );
    let cmake = Path::new("/scratch/CMakeLists.txt");
    let gersemi_config = Path::new("/scratch/.gersemirc");
    assert_eq!(
        argv_strings(&gersemi_check(Path::new(BIN), &[cmake], gersemi_config)),
        vec![
            BIN,
            "--check",
            "--diff",
            "--no-cache",
            "--no-color",
            "-w",
            "1",
            "--config",
            "/scratch/.gersemirc",
            "--",
            "/scratch/CMakeLists.txt"
        ]
    );
    assert_eq!(
        argv_strings(&gersemi_fix(Path::new(BIN), &[cmake], gersemi_config)),
        vec![
            BIN,
            "-i",
            "--no-cache",
            "--no-color",
            "-w",
            "1",
            "--config",
            "/scratch/.gersemirc",
            "--",
            "/scratch/CMakeLists.txt"
        ]
    );
    let make = Path::new("/scratch/Makefile");
    let mbake_config = Path::new("/scratch/.bake.toml");
    assert_eq!(
        argv_strings(&mbake_check(Path::new(BIN), &[make], mbake_config)),
        vec![
            BIN,
            "format",
            "--check",
            "--config",
            "/scratch/.bake.toml",
            "/scratch/Makefile"
        ]
    );
    assert_eq!(
        argv_strings(&mbake_fix(Path::new(BIN), &[make], mbake_config)),
        vec![
            BIN,
            "format",
            "--config",
            "/scratch/.bake.toml",
            "/scratch/Makefile"
        ]
    );
    let lua = Path::new("/scratch/sample.lua");
    let selene_flags = [
        "--display-style=json2",
        "--no-summary",
        "--no-exclude",
        "--color=Never",
        "--num-threads=1",
    ];
    let mut selene_plain = vec![BIN];
    selene_plain.extend(selene_flags);
    selene_plain.push("/scratch/sample.lua");
    assert_eq!(
        argv_strings(&selene_check(Path::new(BIN), &[lua], None)),
        selene_plain
    );
    let mut selene_configured = vec![BIN];
    selene_configured.extend(selene_flags);
    selene_configured.extend(["--config", "/scratch/selene.toml", "/scratch/sample.lua"]);
    assert_eq!(
        argv_strings(&selene_check(
            Path::new(BIN),
            &[lua],
            Some(Path::new("/scratch/selene.toml"))
        )),
        selene_configured
    );
    let zanuda = zanuda_check(
        Path::new(BIN),
        Path::new("/scratch/dx-zanuda/list.tsv"),
        Path::new("/scratch/dx-zanuda/out.jsonl"),
    );
    assert_eq!(
        argv_strings(&zanuda),
        vec![
            BIN,
            "-cmt-list",
            "/scratch/dx-zanuda/list.tsv",
            "-no-top_file_license",
            "-no-check-filesystem",
            "-ordjsonl",
            "/scratch/dx-zanuda/out.jsonl"
        ]
    );
    assert_eq!(zanuda.cwd_rel, "");
    let stylua_config = Path::new("/scratch/stylua.toml");
    assert_eq!(
        argv_strings(&stylua_check(Path::new(BIN), &[lua], None)),
        vec![
            BIN,
            "--no-editorconfig",
            "--check",
            "--output-format=json",
            "/scratch/sample.lua"
        ]
    );
    assert_eq!(
        argv_strings(&stylua_check(Path::new(BIN), &[lua], Some(stylua_config))),
        vec![
            BIN,
            "--no-editorconfig",
            "--check",
            "--output-format=json",
            "--config-path",
            "/scratch/stylua.toml",
            "/scratch/sample.lua"
        ]
    );
    assert_eq!(
        argv_strings(&stylua_fix(Path::new(BIN), &[lua], None)),
        vec![BIN, "--no-editorconfig", "/scratch/sample.lua"]
    );
    assert_eq!(
        argv_strings(&stylua_fix(Path::new(BIN), &[lua], Some(stylua_config))),
        vec![
            BIN,
            "--no-editorconfig",
            "--config-path",
            "/scratch/stylua.toml",
            "/scratch/sample.lua"
        ]
    );
    let topiary_files = [Path::new("/scratch/a.ml"), Path::new("/scratch/b.mli")];
    assert_eq!(
        argv_strings(&topiary_check(Path::new(BIN), &topiary_files)),
        vec![BIN, "/scratch/a.ml", "/scratch/b.mli"]
    );
    assert_eq!(
        argv_strings(&topiary_fix(Path::new(BIN), &topiary_files)),
        vec![BIN, "--write", "/scratch/a.ml", "/scratch/b.mli"]
    );
    let ffmt_files = [Path::new("/scratch/a.f90")];
    let ffmt_check_argv = ffmt_check(Path::new(BIN), &ffmt_files, "dx-ffmt-default");
    assert_eq!(ffmt_check_argv.cwd_rel, "dx-ffmt-default");
    assert_eq!(
        argv_strings(&ffmt_check_argv),
        vec![
            BIN,
            "--check",
            "--no-cache",
            "--no-ignore",
            "--color",
            "never",
            "/scratch/a.f90"
        ]
    );
    assert_eq!(
        argv_strings(&ffmt_fix(Path::new(BIN), &ffmt_files, "pkg")),
        vec![
            BIN,
            "--no-cache",
            "--no-ignore",
            "--color",
            "never",
            "/scratch/a.f90"
        ]
    );
    assert_eq!(
        argv_strings(&fortitude_check(
            Path::new(BIN),
            &[Path::new("/scratch/a.f90"), Path::new("/scratch/b.F90")]
        )),
        vec![
            BIN,
            "check",
            "--isolated",
            "--no-respect-gitignore",
            "--progress-bar",
            "off",
            "--output-format",
            "json",
            "/scratch/a.f90",
            "/scratch/b.F90"
        ]
    );
    assert_eq!(
        argv_strings(&xmllint_check(
            Path::new(BIN),
            &[Path::new("/scratch/a.xml"), Path::new("/scratch/b.xml")]
        )),
        vec![
            BIN,
            "--noout",
            "--nonet",
            "--nocatalogs",
            "/scratch/a.xml",
            "/scratch/b.xml"
        ]
    );
    let jsonnet = Path::new("/scratch/Sample.jsonnet");
    assert_eq!(
        argv_strings(&jsonnet_lint_check(Path::new(BIN), &[jsonnet])),
        vec![BIN, "/scratch/Sample.jsonnet"]
    );
    assert_eq!(
        argv_strings(&jsonnetfmt_check(Path::new(BIN), &[jsonnet])),
        vec![BIN, "--test", "/scratch/Sample.jsonnet"]
    );
    assert_eq!(
        argv_strings(&jsonnetfmt_fix(Path::new(BIN), &[jsonnet])),
        vec![BIN, "-i", "/scratch/Sample.jsonnet"]
    );
    let typst = Path::new("/scratch/sample.typ");
    assert_eq!(
        argv_strings(&typstyle_check(Path::new(BIN), &[typst])),
        vec![BIN, "--check", "/scratch/sample.typ"]
    );
    assert_eq!(
        argv_strings(&typstyle_fix(Path::new(BIN), &[typst])),
        vec![BIN, "-i", "/scratch/sample.typ"]
    );
    let tex = Path::new("/scratch/doc/sample.tex");
    let cruft = Path::new("/scratch/.latexindent");
    assert_eq!(
        argv_strings(&latexindent_check(Path::new(BIN), tex, cruft, None)),
        vec![
            BIN,
            "-k",
            "-g=/scratch/.latexindent/indent.log",
            "-c=/scratch/.latexindent/",
            "/scratch/doc/sample.tex"
        ]
    );
    assert_eq!(
        argv_strings(&latexindent_fix(
            Path::new(BIN),
            tex,
            cruft,
            Some(Path::new("/scratch/cfg.yaml"))
        )),
        vec![
            BIN,
            "-w",
            "-g=/scratch/.latexindent/indent.log",
            "-c=/scratch/.latexindent/",
            "-l=/scratch/cfg.yaml",
            "/scratch/doc/sample.tex"
        ]
    );
    let dart = Path::new("/scratch/lib/a.dart");
    assert_eq!(
        argv_strings(&dart_format_check(Path::new(BIN), &[dart])),
        vec![
            BIN,
            "--suppress-analytics",
            "format",
            "--output=none",
            "--set-exit-if-changed",
            "/scratch/lib/a.dart"
        ]
    );
    assert_eq!(
        argv_strings(&dart_format_fix(Path::new(BIN), &[dart])),
        vec![
            BIN,
            "--suppress-analytics",
            "format",
            "--output=write",
            "/scratch/lib/a.dart"
        ]
    );
    assert_eq!(
        argv_strings(&dart_analyze(
            Path::new(BIN),
            Path::new("/scratch/.dx-dart-analysis"),
            Path::new("/scratch/analysis_options.yaml"),
            &[dart]
        )),
        vec![
            BIN,
            "--suppress-analytics",
            "analyze",
            "--format=json",
            "--cache=/scratch/.dx-dart-analysis",
            "/scratch/analysis_options.yaml",
            "/scratch/lib/a.dart"
        ]
    );
    let zig = Path::new("/scratch/sample.zig");
    assert_eq!(
        argv_strings(&zig_ast_check(Path::new(BIN), zig)),
        vec![BIN, "ast-check", "--color", "off", "/scratch/sample.zig"]
    );
    assert_eq!(
        argv_strings(&zig_fmt_check(Path::new(BIN), &[zig])),
        vec![
            BIN,
            "fmt",
            "--check",
            "--color",
            "off",
            "/scratch/sample.zig"
        ]
    );
    assert_eq!(
        argv_strings(&zig_fmt_fix(Path::new(BIN), &[zig])),
        vec![BIN, "fmt", "--color", "off", "/scratch/sample.zig"]
    );
    let d = Path::new("/scratch/sample.d");
    assert_eq!(
        argv_strings(&dfmt_check(Path::new(BIN), d)),
        vec![BIN, "--config", "dx-dfmt-default", "/scratch/sample.d"]
    );
    assert_eq!(
        argv_strings(&dfmt_fix(Path::new(BIN), &[d])),
        vec![
            BIN,
            "--config",
            "dx-dfmt-default",
            "--inplace",
            "/scratch/sample.d"
        ]
    );
    assert_eq!(
        argv_strings(&dscanner_check(Path::new(BIN), &[d])),
        vec![
            BIN,
            "--styleCheck",
            "--config",
            "dx-dscanner-default/dscanner.ini",
            "--errorFormat",
            DSCANNER_FORMAT,
            "/scratch/sample.d"
        ]
    );
    let perl = Path::new("/scratch/sample.pl");
    assert_eq!(
        argv_strings(&perlcritic_check(Path::new(BIN), &[perl])),
        vec![
            BIN,
            "--noprofile",
            "--severity=5",
            "--nocolor",
            "--quiet",
            "--verbose",
            PERLCRITIC_FORMAT,
            "/scratch/sample.pl"
        ]
    );
    let julia = Path::new("/scratch/sample.jl");
    assert_eq!(
        argv_strings(&julia_format_check(Path::new(BIN), &[julia])),
        vec![BIN, "--check", "/scratch/sample.jl"]
    );
    assert_eq!(
        argv_strings(&julia_format_fix(Path::new(BIN), &[julia])),
        vec![BIN, "--apply", "/scratch/sample.jl"]
    );
    let pm = Path::new("/scratch/sample.pm");
    assert_eq!(
        argv_strings(&perltidy_check(Path::new(BIN), pm)),
        vec![
            BIN,
            "--noprofile",
            "--standard-output",
            "--standard-error-output",
            "--output-line-ending=unix",
            "/scratch/sample.pm"
        ]
    );
    assert_eq!(
        argv_strings(&perltidy_fix(Path::new(BIN), &[pm])),
        vec![
            BIN,
            "--noprofile",
            "-b",
            "-bext=/",
            "--standard-error-output",
            "--output-line-ending=unix",
            "/scratch/sample.pm"
        ]
    );
    let erl = Path::new("/scratch/sample.erl");
    assert_eq!(
        argv_strings(&erlfmt_check(Path::new(BIN), &[erl])),
        vec![BIN, "--check", "/scratch/sample.erl"]
    );
    assert_eq!(
        argv_strings(&erlfmt_fix(Path::new(BIN), &[erl])),
        vec![BIN, "--write", "/scratch/sample.erl"]
    );
    assert_eq!(
        argv_strings(&elvis_check(
            Path::new(BIN),
            &[Path::new("src/sample.erl")],
            Path::new("/scratch/elvis.config")
        )),
        vec![
            BIN,
            "--config",
            "/scratch/elvis.config",
            "--output-format",
            "parsable",
            "rock",
            "src/sample.erl"
        ]
    );
    let ex = Path::new("/scratch/sample.ex");
    assert_eq!(
        argv_strings(&mix_format_check(Path::new(BIN), &[ex])),
        vec![BIN, "--check-formatted", "/scratch/sample.ex"]
    );
    assert_eq!(
        argv_strings(&mix_format_fix(Path::new(BIN), &[ex])),
        vec![BIN, "/scratch/sample.ex"]
    );
    let nix = Path::new("/scratch/sample.nix");
    let other = Path::new("/scratch/other.nix");
    assert_eq!(
        argv_strings(&nixfmt_check(Path::new(BIN), &[nix, other])),
        vec![BIN, "--check", "/scratch/sample.nix", "/scratch/other.nix"]
    );
    assert_eq!(
        argv_strings(&nixfmt_fix(Path::new(BIN), &[nix, other])),
        vec![BIN, "/scratch/sample.nix", "/scratch/other.nix"]
    );
    let hs = Path::new("/scratch/Sample.hs");
    assert_eq!(
        argv_strings(&ormolu_check(Path::new(BIN), &[hs])),
        vec![
            BIN,
            "--mode",
            "check",
            "--no-cabal",
            "--no-dot-ormolu",
            "--color",
            "never",
            "/scratch/Sample.hs"
        ]
    );
    assert_eq!(
        argv_strings(&ormolu_fix(Path::new(BIN), &[hs])),
        vec![
            BIN,
            "--mode",
            "inplace",
            "--no-cabal",
            "--no-dot-ormolu",
            "--color",
            "never",
            "/scratch/Sample.hs"
        ]
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
    let bicep = Path::new("/scratch/main.bicep");
    assert_eq!(
        argv_strings(&bicep_format_check(Path::new(BIN), bicep)),
        vec![
            BIN,
            "format",
            "--newline-kind",
            "LF",
            "--stdout",
            "/scratch/main.bicep"
        ]
    );
    assert_eq!(
        argv_strings(&bicep_format_fix(Path::new(BIN), bicep)),
        vec![BIN, "format", "--newline-kind", "LF", "/scratch/main.bicep"]
    );
    assert_eq!(
        argv_strings(&bicep_lint_check(Path::new(BIN), bicep)),
        vec![
            BIN,
            "lint",
            "--no-restore",
            "--diagnostics-format",
            "sarif",
            "/scratch/main.bicep"
        ]
    );
    let rego = Path::new("/scratch/policy/a.rego");
    assert_eq!(
        argv_strings(&opa_fmt_check(Path::new(BIN), rego)),
        vec![BIN, "fmt", "/scratch/policy/a.rego"]
    );
    assert_eq!(
        argv_strings(&opa_fmt_fix(Path::new(BIN), rego)),
        vec![BIN, "fmt", "-w", "/scratch/policy/a.rego"]
    );
    assert_eq!(
        argv_strings(&regal_lint(
            Path::new(BIN),
            &[Path::new("policy/a.rego")],
            Path::new("/scratch/.regal/config.yaml")
        )),
        vec![
            BIN,
            "lint",
            "--format",
            "json",
            "--no-color",
            "--fail-level",
            "warning",
            "--config-file",
            "/scratch/.regal/config.yaml",
            "--",
            "policy/a.rego"
        ]
    );
    assert_eq!(
        argv_strings(&opa_check(Path::new(BIN), &[rego])),
        vec![
            BIN,
            "check",
            "--format",
            "json",
            "--max-errors=-1",
            "/scratch/policy/a.rego"
        ]
    );
    let html = Path::new("/scratch/templates/a.html");
    let djlint_conf = Path::new("/scratch/djlint.toml");
    assert_eq!(
        argv_strings(&djlint_check(Path::new(BIN), &[html], None)),
        vec![
            BIN,
            "--lint",
            "--quiet",
            "--linter-output-format",
            "{filename}:{line}: {code} {message}",
            "/scratch/templates/a.html"
        ]
    );
    assert_eq!(
        argv_strings(&djlint_check(Path::new(BIN), &[html], Some(djlint_conf)))[5..],
        [
            "--configuration",
            "/scratch/djlint.toml",
            "--prefer-configuration",
            "/scratch/templates/a.html"
        ]
    );
    assert_eq!(
        argv_strings(&djlint_format_check(
            Path::new(BIN),
            &[html],
            Some(djlint_conf)
        )),
        vec![
            BIN,
            "--check",
            "--configuration",
            "/scratch/djlint.toml",
            "--prefer-configuration",
            "/scratch/templates/a.html"
        ]
    );
    assert_eq!(
        argv_strings(&djlint_format_fix(Path::new(BIN), &[html], None)),
        vec![BIN, "--reformat", "--quiet", "/scratch/templates/a.html"]
    );
    let yaml = Path::new("/scratch/Sample.yaml");
    assert_eq!(
        argv_strings(&yamlfmt_check(Path::new(BIN), &[yaml], None)),
        vec![BIN, "-lint", "-q", "/scratch/Sample.yaml"]
    );
    assert_eq!(
        argv_strings(&yamlfmt_fix(Path::new(BIN), &[yaml], None)),
        vec![BIN, "/scratch/Sample.yaml"]
    );
    let conf = Path::new("/scratch/conf.yaml");
    assert_eq!(
        argv_strings(&yamlfmt_check(Path::new(BIN), &[yaml], Some(conf))),
        vec![
            BIN,
            "-lint",
            "-q",
            "-conf",
            "/scratch/conf.yaml",
            "/scratch/Sample.yaml"
        ]
    );
    assert_eq!(
        argv_strings(&yamlfmt_fix(Path::new(BIN), &[yaml], Some(conf))),
        vec![BIN, "-conf", "/scratch/conf.yaml", "/scratch/Sample.yaml"]
    );
    assert_eq!(
        argv_strings(&yamllint_check(Path::new(BIN), &[yaml], Some(conf))),
        vec![
            BIN,
            "-f",
            "parsable",
            "-c",
            "/scratch/conf.yaml",
            "/scratch/Sample.yaml"
        ]
    );
    assert_eq!(
        argv_strings(&yamllint_check(Path::new(BIN), &[yaml], None)),
        vec![
            BIN,
            "-f",
            "parsable",
            "-d",
            "default",
            "/scratch/Sample.yaml"
        ]
    );
}

#[test]
fn ansible_lint_check_offline_sarif_with_project_dir_and_optional_config() {
    let file = Path::new("/scratch/site.yml");
    let root = Path::new("/scratch");
    let base = [
        BIN,
        "--offline",
        "--nocolor",
        "-f",
        "sarif",
        "--project-dir",
        "/scratch",
    ];
    let mut with = base.to_vec();
    with.extend(["-c", "/scratch/.ansible-lint", "/scratch/site.yml"]);
    let context = AnsibleLintContext {
        config: Some(Path::new("/scratch/.ansible-lint")),
        collections: &[],
        project_dir: root,
    };
    assert_eq!(
        argv_strings(&ansible_lint_check(Path::new(BIN), &[file], &context)),
        with
    );
    let mut without = base.to_vec();
    without.push("/scratch/site.yml");
    let context = AnsibleLintContext {
        config: None,
        collections: &[],
        project_dir: root,
    };
    assert_eq!(
        argv_strings(&ansible_lint_check(Path::new(BIN), &[file], &context)),
        without
    );
}

#[test]
fn ansible_lint_check_puts_collection_roots_first() {
    let file = Path::new("/scratch/site.yml");
    let roots = [Path::new("/scratch/a"), Path::new("/scratch/b")];
    let context = AnsibleLintContext {
        config: None,
        collections: &roots,
        project_dir: Path::new("/scratch"),
    };
    let joined = std::env::join_paths(roots)
        .expect("joined")
        .into_string()
        .expect("utf8");
    assert_eq!(
        argv_strings(&ansible_lint_check(Path::new(BIN), &[file], &context)),
        [
            BIN,
            "--dx-collections-path",
            joined.as_str(),
            "--offline",
            "--nocolor",
            "-f",
            "sarif",
            "--project-dir",
            "/scratch",
            "/scratch/site.yml",
        ]
    );
}

#[test]
fn cmakelint_check_always_passes_the_config_and_ends_flags() {
    let a = Path::new("a/CMakeLists.txt");
    let b = Path::new("b.cmake");
    let argv = argv_strings(&cmakelint_check(
        Path::new(BIN),
        &[a, b],
        Path::new("/scratch/.cmakelintrc"),
    ));
    assert_eq!(
        argv,
        [
            BIN,
            "--config=/scratch/.cmakelintrc",
            "--",
            "a/CMakeLists.txt",
            "b.cmake"
        ]
    );
    for banned in ["--spaces", "--linelength", "--filter", "--quiet"] {
        assert!(!argv.iter().any(|arg| arg.starts_with(banned)), "{banned}");
    }
}

#[test]
fn cfn_lint_check_always_passes_the_config_and_no_spec_or_region_flags() {
    let a = Path::new("/scratch/a.yaml");
    let b = Path::new("/scratch/b.template");
    let argv = argv_strings(&cfn_lint_check(
        Path::new(BIN),
        &[a, b],
        Path::new("/scratch/.cfn-lint.yaml"),
    ));
    assert_eq!(
        argv,
        [
            BIN,
            "--format",
            "json",
            "--non-zero-exit-code",
            "informational",
            "--config-file",
            "/scratch/.cfn-lint.yaml",
            "--",
            "/scratch/a.yaml",
            "/scratch/b.template"
        ]
    );
    for banned in [
        "--update-specs",
        "--regions",
        "--append-rules",
        "--override-spec",
        "--templates",
    ] {
        assert!(!argv.iter().any(|arg| arg == banned), "{banned}");
    }
}

#[test]
fn sqlfluff_check_always_passes_the_config_and_ignores_ambient_config() {
    let a = Path::new("/scratch/a.sql");
    let b = Path::new("/scratch/b.sql");
    let argv = argv_strings(&sqlfluff_check(
        Path::new(BIN),
        &[a, b],
        Path::new("/scratch/.sqlfluff"),
    ));
    assert_eq!(
        argv,
        [
            BIN,
            "lint",
            "--format",
            "json",
            "--config",
            "/scratch/.sqlfluff",
            "--ignore-local-config",
            "--disregard-sqlfluffignores",
            "--nocolor",
            "--disable-progress-bar",
            "--processes",
            "1",
            "--",
            "/scratch/a.sql",
            "/scratch/b.sql"
        ]
    );
    for banned in ["--dialect", "fix", "format", "--templater"] {
        assert!(!argv.iter().any(|arg| arg == banned), "{banned}");
    }
}

#[test]
fn sqlfluff_format_invocations_share_the_bound_config_and_flags() {
    let a = Path::new("/scratch/a.sql");
    let config = Path::new("/scratch/.sqlfluff");
    let tail = [
        "--config",
        "/scratch/.sqlfluff",
        "--ignore-local-config",
        "--disregard-sqlfluffignores",
        "--nocolor",
        "--disable-progress-bar",
        "--processes",
        "1",
        "--",
        "/scratch/a.sql",
    ];
    let check = argv_strings(&sqlfluff_format_check(Path::new(BIN), &[a], config));
    let mut want = vec![
        BIN,
        "lint",
        "--format",
        "json",
        "--rules",
        SQLFLUFF_FORMAT_RULES,
    ];
    want.extend(tail);
    assert_eq!(check, want);
    let fix = argv_strings(&sqlfluff_format_fix(Path::new(BIN), &[a], config));
    let mut want = vec![BIN, "format"];
    want.extend(tail);
    assert_eq!(fix, want);
}

#[test]
fn graphql_eslint_check_passes_config_root_and_files_after_a_separator() {
    let query = Path::new("/scratch/api/query.graphql");
    let launch = ResolvedTool::javascript("graphql_eslint", Path::new(BIN));
    let check = graphql_eslint_check(
        &launch,
        &[query],
        Path::new("/scratch/api/graphql.json"),
        Path::new("/scratch"),
    )
    .expect("resolves");
    assert_eq!(
        argv_strings(&check),
        vec![
            BIN,
            "--config",
            "/scratch/api/graphql.json",
            "--root",
            "/scratch",
            "--",
            "/scratch/api/query.graphql"
        ]
    );
    let blank = ResolvedTool::javascript("graphql_eslint", Path::new(""));
    assert_eq!(
        graphql_eslint_check(
            &blank,
            &[query],
            Path::new("/scratch/api/graphql.json"),
            Path::new("/scratch"),
        ),
        Err(LaunchError::MissingProgram {
            tool: "graphql_eslint".to_owned()
        })
    );
}

#[test]
fn redocly_check_lints_one_root_with_a_json_report_and_strict_config() {
    let root = Path::new("/scratch/api/openapi.yaml");
    let launch = ResolvedTool::javascript("redocly", Path::new(BIN));
    let check = redocly_check(&launch, root, Path::new("/scratch/redocly.yaml")).expect("resolves");
    assert_eq!(
        argv_strings(&check),
        vec![
            BIN,
            "lint",
            "--format=json",
            "--max-problems=1000000",
            "--lint-config=error",
            "--config",
            "/scratch/redocly.yaml",
            "/scratch/api/openapi.yaml"
        ]
    );
    let blank = ResolvedTool::javascript("redocly", Path::new(""));
    assert_eq!(
        redocly_check(&blank, root, Path::new("/scratch/redocly.yaml")),
        Err(LaunchError::MissingProgram {
            tool: "redocly".to_owned()
        })
    );
}

#[test]
fn file_family_lint_checks_are_check_only() {
    let css = Path::new("/scratch/style.css");
    let launch = ResolvedTool::javascript("stylelint", Path::new(BIN));
    let check =
        stylelint_check(&launch, &[css], Path::new("/scratch/stylelint.json")).expect("resolves");
    assert_eq!(
        argv_strings(&check),
        vec![
            BIN,
            "--formatter",
            "json",
            "--config",
            "/scratch/stylelint.json",
            "/scratch/style.css"
        ]
    );
    let blank = ResolvedTool::javascript("stylelint", Path::new(""));
    assert_eq!(
        stylelint_check(&blank, &[css], Path::new("/scratch/stylelint.json")),
        Err(LaunchError::MissingProgram {
            tool: "stylelint".to_owned()
        })
    );
    let feature = Path::new("/scratch/a.feature");
    let launch = ResolvedTool::javascript("gplint", Path::new(BIN));
    let check =
        gplint_check(&launch, &[feature], Path::new("/scratch/.gplintrc")).expect("resolves");
    assert_eq!(
        argv_strings(&check),
        vec![
            BIN,
            "-f",
            "json",
            "-i",
            "dx-gplint-no-ignore",
            "-c",
            "/scratch/.gplintrc",
            "/scratch/a.feature"
        ]
    );
    let rb = Path::new("/scratch/Sample.rb");
    assert_eq!(
        argv_strings(&rubocop_check(Path::new(BIN), &[rb], None)),
        vec![
            BIN,
            "--format",
            "json",
            "--cache",
            "false",
            "/scratch/Sample.rb"
        ]
    );
    assert_eq!(
        argv_strings(&rubocop_check(
            Path::new(BIN),
            &[rb],
            Some(Path::new("/scratch/rubocop.yml"))
        )),
        vec![
            BIN,
            "--format",
            "json",
            "--cache",
            "false",
            "--config",
            "/scratch/rubocop.yml",
            "/scratch/Sample.rb"
        ]
    );
    let ps = Path::new("/scratch/Sample.ps1");
    assert_eq!(
        argv_strings(&psscriptanalyzer_check(Path::new(BIN), &[ps], None)),
        vec![BIN, "/scratch/Sample.ps1"]
    );
    assert_eq!(
        argv_strings(&psscriptanalyzer_check(
            Path::new(BIN),
            &[ps],
            Some(Path::new("/scratch/PSScriptAnalyzerSettings.psd1"))
        )),
        vec![
            BIN,
            "--settings",
            "/scratch/PSScriptAnalyzerSettings.psd1",
            "/scratch/Sample.ps1"
        ]
    );
    assert_eq!(
        argv_strings(&psscriptanalyzer_format_check(Path::new(BIN), &[ps], None)),
        vec![BIN, "--format-check", "/scratch/Sample.ps1"]
    );
    assert_eq!(
        argv_strings(&psscriptanalyzer_format_fix(
            Path::new(BIN),
            &[ps],
            Some(Path::new("/scratch/PSScriptAnalyzerSettings.psd1"))
        )),
        vec![
            BIN,
            "--format-write",
            "--settings",
            "/scratch/PSScriptAnalyzerSettings.psd1",
            "/scratch/Sample.ps1"
        ]
    );
    let sh = Path::new("/scratch/run.sh");
    assert_eq!(
        argv_strings(&shellcheck_check(Path::new(BIN), &[sh], None)),
        vec![BIN, "--format=gcc", "--norc", "/scratch/run.sh"]
    );
    assert_eq!(
        argv_strings(&shellcheck_check(
            Path::new(BIN),
            &[sh],
            Some(Path::new("/scratch/.shellcheckrc"))
        )),
        vec![
            BIN,
            "--format=gcc",
            "--rcfile=/scratch/.shellcheckrc",
            "/scratch/run.sh"
        ]
    );
    assert_eq!(
        argv_strings(&hlint_check(
            Path::new(BIN),
            &[Path::new("/s/A.hs")],
            Path::new("/scratch/dx-hlint-default/hlint.yaml")
        )),
        vec![
            BIN,
            "--json",
            "--hint=/scratch/dx-hlint-default/hlint.yaml",
            "--",
            "/s/A.hs"
        ]
    );
    let dockerfile = Path::new("/scratch/Dockerfile");
    assert_eq!(
        argv_strings(&hadolint_check(
            Path::new(BIN),
            &[dockerfile],
            Path::new("/scratch/.hadolint.yaml")
        )),
        vec![
            BIN,
            "--config",
            "/scratch/.hadolint.yaml",
            "--no-color",
            "-f",
            "json",
            "--",
            "/scratch/Dockerfile"
        ]
    );
    let makefile = Path::new("/scratch/Makefile");
    assert_eq!(
        argv_strings(&checkmake_check(
            Path::new(BIN),
            &[makefile],
            Path::new("/scratch/checkmake.ini")
        )),
        vec![
            BIN,
            "--debug",
            "--output",
            "json",
            "--config",
            "/scratch/checkmake.ini",
            "--",
            "/scratch/Makefile"
        ]
    );
    assert_eq!(
        argv_strings(&tflint_check(
            Path::new(BIN),
            "infra/a",
            &[Path::new("main.tf"), Path::new("vars.tf")],
            Path::new("/scratch/.tflint.hcl")
        )),
        vec![
            BIN,
            "--config",
            "/scratch/.tflint.hcl",
            "--chdir=infra/a",
            "--format=json",
            "--no-color",
            "--call-module-type=none",
            "--filter=main.tf",
            "--filter=vars.tf"
        ]
    );
    assert_eq!(
        argv_strings(&credo_check(
            Path::new(BIN),
            &[Path::new("/scratch/lib/a.ex")],
            Path::new("/scratch/.credo.exs")
        )),
        vec![
            BIN,
            "--config-file",
            "/scratch/.credo.exs",
            "/scratch/lib/a.ex"
        ]
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
fn nix_tools_pin_their_flags() {
    let nix = Path::new("/scratch/-a.nix");
    let config = Path::new("/scratch/statix.toml");
    assert_eq!(
        argv_strings(&statix_check(Path::new(BIN), nix, config)),
        vec![
            BIN,
            "check",
            "-o",
            "json",
            "--config",
            "/scratch/statix.toml",
            "--",
            "/scratch/-a.nix"
        ]
    );
    assert_eq!(
        argv_strings(&statix_fix(Path::new(BIN), nix, config)),
        vec![
            BIN,
            "fix",
            "--config",
            "/scratch/statix.toml",
            "--",
            "/scratch/-a.nix"
        ]
    );
    assert_eq!(
        argv_strings(&deadnix_check(Path::new(BIN), &[nix])),
        vec![BIN, "--fail", "-o", "json", "--", "/scratch/-a.nix"]
    );
    assert_eq!(
        argv_strings(&deadnix_fix(Path::new(BIN), &[nix])),
        vec![BIN, "--edit", "--", "/scratch/-a.nix"]
    );
    assert_eq!(statix_check(Path::new(BIN), nix, config).cwd_rel, "");
}

#[test]
fn hadolint_never_overrides_the_user_policy() {
    let invocation = hadolint_check(
        Path::new(BIN),
        &[Path::new("-t")],
        Path::new("/scratch/.hadolint.yaml"),
    );
    let argv = argv_strings(&invocation);
    let flags = &argv[..argv.len() - 1];
    for banned in [
        "-t",
        "--failure-threshold",
        "--no-fail",
        "--ignore",
        "--disable-ignore-pragma",
    ] {
        assert!(!flags.contains(&banned.to_owned()), "{banned} in {argv:?}");
    }
    assert_eq!(argv[argv.len() - 2], "--");
    assert_eq!(invocation.cwd_rel, "");
}

#[test]
fn fixed_args_match_recorded_invocations() {
    let file = Path::new(FILE);
    let cases: &[(&[&str], Invocation)] = &[
        (
            GOOGLE_JAVA_FORMAT_CHECK_ARGS,
            google_java_format_check(Path::new(BIN), &[file]),
        ),
        (
            KTLINT_CHECK_ARGS,
            ktlint_check(Path::new(BIN), &[file], None),
        ),
        (RUBOCOP_ARGS, rubocop_check(Path::new(BIN), &[file], None)),
        (
            KEEP_SORTED_CHECK_ARGS,
            keep_sorted_check(Path::new(BIN), &[file]),
        ),
        (
            KEEP_SORTED_FIX_ARGS,
            keep_sorted_fix(Path::new(BIN), &[file]),
        ),
        (
            NOTEBOOK_OUTPUTS_CHECK_ARGS,
            notebook_outputs_check(Path::new(BIN), &[file]),
        ),
        (
            NOTEBOOK_OUTPUTS_FIX_ARGS,
            notebook_outputs_fix(Path::new(BIN), &[file]),
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

#[test]
fn batch_argv_counts_bytes_and_utf16_units() {
    assert_eq!(argv_sizes(OsStr::new("")), (0, 0));
    assert_eq!(argv_sizes(OsStr::new("abc")), (3, 3));
    assert_eq!(argv_sizes(OsStr::new("--mode")), (6, 6));
    assert_eq!(argv_sizes(OsStr::new("é")), (2, 1));
    assert_eq!(argv_sizes(OsStr::new("字")), (3, 1));
    assert_eq!(argv_sizes(OsStr::new("🦀")), (4, 2));
}

fn batch_cost(head: &[OsString], batch: &[&Path]) -> (usize, usize) {
    let mut bytes = 0usize;
    let mut units = 0usize;
    for arg in head
        .iter()
        .map(|part| part.as_os_str())
        .chain(batch.iter().map(|file| file.as_os_str()))
    {
        let (argument_bytes, argument_units) = argv_sizes(arg);
        bytes += argument_bytes + 1;
        units += argument_units + 1;
    }
    (bytes, units)
}

#[test]
fn batches_keep_input_order_under_a_small_budget() {
    let head = vec![
        OsString::from("/bin/tool"),
        OsString::from("--mode"),
        OsString::from("lint"),
    ];
    let owned: Vec<PathBuf> = (0..8)
        .map(|index| PathBuf::from(format!("/s/notes-{index}.txt")))
        .collect();
    let refs: Vec<&Path> = owned.iter().map(PathBuf::as_path).collect();
    let batches = plan_batches_with_budget(&head, &refs, 80);
    assert!(batches.len() > 1, "small budget forces batches");
    let roundtrip: Vec<&Path> = batches.iter().flatten().copied().collect();
    assert_eq!(roundtrip, refs);
    for batch in &batches {
        assert!(!batch.is_empty());
        let (bytes, units) = batch_cost(&head, batch);
        assert!(bytes <= 80, "bytes {bytes} fit");
        assert!(units <= 80, "units {units} fit");
    }
}

#[test]
fn a_small_selection_stays_on_one_spawn() {
    let head = vec![OsString::from("/bin/tool")];
    let owned: Vec<PathBuf> = (0..3)
        .map(|index| PathBuf::from(format!("/s/notes-{index}.txt")))
        .collect();
    let refs: Vec<&Path> = owned.iter().map(PathBuf::as_path).collect();
    assert_eq!(plan_batches(&head, &refs), vec![refs]);
}

#[test]
fn an_empty_selection_keeps_one_spawn() {
    let head = vec![OsString::from("/bin/tool")];
    let batches: Vec<Vec<&Path>> = plan_batches(&head, &[]);
    assert_eq!(batches.len(), 1);
    assert!(batches[0].is_empty());
}

#[test]
fn an_oversized_file_travels_alone() {
    let head = vec![OsString::from("/bin/tool")];
    let big = PathBuf::from(format!("/s/{}.txt", "x".repeat(300)));
    let small = PathBuf::from("/s/a.txt");
    let refs = vec![big.as_path(), small.as_path()];
    let batches = plan_batches_with_budget(&head, &refs, 100);
    assert_eq!(batches.len(), 2);
    assert_eq!(batches[0], vec![big.as_path()]);
    assert_eq!(batches[1], vec![small.as_path()]);
}

#[test]
fn argv_budget_shrinks_behind_batch_launchers() {
    assert_eq!(
        argv_budget(OsStr::new("/bin/tool.bat")),
        LAUNCHER_BATCH_BUDGET
    );
    assert_eq!(
        argv_budget(OsStr::new("C:\\bin\\TOOL.CMD")),
        LAUNCHER_BATCH_BUDGET
    );
    assert_eq!(
        argv_budget(OsStr::new("C:\\bin\\tool.exe")),
        ARGV_BATCH_BUDGET
    );
    assert_eq!(argv_budget(OsStr::new("/bin/tool")), ARGV_BATCH_BUDGET);
    assert!(!is_batch_launcher(OsStr::new("/bin/bat")));
}

#[test]
fn windows_limit_is_tighter_for_launchers() {
    assert_eq!(
        windows_argv_limit(OsStr::new("tool.bat")),
        Some(LAUNCHER_COMMAND_LINE_LIMIT)
    );
    assert_eq!(
        windows_argv_limit(OsStr::new("tool.exe")),
        Some(WINDOWS_COMMAND_LINE_LIMIT)
    );
    assert!(ARGV_BATCH_BUDGET < WINDOWS_COMMAND_LINE_LIMIT);
    assert!(LAUNCHER_BATCH_BUDGET < LAUNCHER_COMMAND_LINE_LIMIT);
    assert!(LAUNCHER_COMMAND_LINE_LIMIT < WINDOWS_COMMAND_LINE_LIMIT);
    assert_eq!(
        native_argv_limit(OsStr::new("tool")).is_some(),
        cfg!(windows)
    );
}

#[test]
fn plan_batches_honours_launcher_budget() {
    let head = vec![OsString::from("/bin/tool.bat"), OsString::from("--check")];
    let owned: Vec<PathBuf> = (0..200)
        .map(|index| PathBuf::from(format!("/s/{index:04}-{}.txt", "p".repeat(60))))
        .collect();
    let refs: Vec<&Path> = owned.iter().map(PathBuf::as_path).collect();
    let batches = plan_batches(&head, &refs);
    assert!(batches.len() > 2, "launcher budget forces batches");
    let roundtrip: Vec<&Path> = batches.iter().flatten().copied().collect();
    assert_eq!(roundtrip, refs);
    for batch in &batches {
        let (bytes, units) = batch_cost(&head, batch);
        assert!(bytes <= LAUNCHER_BATCH_BUDGET, "bytes {bytes} fit");
        assert!(units <= LAUNCHER_BATCH_BUDGET, "units {units} fit");
    }
}

#[test]
fn file_independent_tools_batch() {
    for tool in [
        "keep_sorted",
        "notebook_outputs",
        "ruff",
        "flake8",
        "pydoclint",
        "buildifier",
        "deadnix",
        "xmllint",
        "selene",
    ] {
        assert_eq!(transport_for(tool), ArgTransport::Batched, "{tool}");
    }
    for tool in [
        "ty",
        "pylint",
        "scalafix",
        "spotbugs",
        "fsharplint",
        "rustfmt",
        "clang_tidy",
        "staticcheck",
        "biome",
        "eslint",
        "statix",
    ] {
        assert_eq!(transport_for(tool), ArgTransport::Single, "{tool}");
    }
}

#[test]
fn batched_probe_spawns_deliver_every_byte() {
    let probe = dx_testing::process_probe();
    let session = tempfile::TempDir::new().expect("scratch");
    let ambient: Vec<(OsString, OsString)> = Vec::new();
    let env = crate::exec::hermetic_env(session.path(), &[], &ambient);
    let head = vec![probe.into_os_string(), OsString::from("--print-argv")];
    let owned: Vec<OsString> = (0..400)
        .map(|index| {
            OsString::from(format!(
                "--stderr-text=padded-{index:04}-{}",
                "n".repeat(100)
            ))
        })
        .collect();
    let file_refs: Vec<&Path> = owned.iter().map(Path::new).collect();
    let batches = plan_batches(&head, &file_refs);
    assert!(batches.len() > 1, "oversized selection splits");
    let mut delivered: Vec<String> = Vec::new();
    for batch in &batches {
        let mut argv = head.clone();
        argv.extend(batch.iter().map(|file| file.as_os_str().to_owned()));
        let mut bytes = 0usize;
        let mut units = 0usize;
        for arg in &argv {
            let (argument_bytes, argument_units) = argv_sizes(arg.as_os_str());
            bytes += argument_bytes + 1;
            units += argument_units + 1;
        }
        assert!(bytes <= ARGV_BATCH_BUDGET, "bytes {bytes} fit");
        assert!(units <= ARGV_BATCH_BUDGET, "units {units} fit");
        let out = crate::exec::spawn(&argv, session.path(), &env).expect("probe runs");
        assert_eq!(out.code, Some(0));
        let mut lines: Vec<String> = String::from_utf8(out.stdout)
            .expect("probe prints text")
            .split('\n')
            .map(str::to_owned)
            .collect();
        lines.pop();
        delivered.extend(lines);
    }
    let mut expected: Vec<String> = Vec::new();
    for batch in &batches {
        expected.push("--print-argv".to_owned());
        expected.extend(batch.iter().map(|file| file.to_string_lossy().into_owned()));
    }
    assert_eq!(delivered, expected);
}

#[test]
fn probe_spawn_preserves_shell_metacharacters() {
    let probe = dx_testing::process_probe();
    let session = tempfile::TempDir::new().expect("scratch");
    let ambient: Vec<(OsString, OsString)> = Vec::new();
    let env = crate::exec::hermetic_env(session.path(), &[], &ambient);
    let tricky = vec![
        OsString::from("--stderr-text=with space"),
        OsString::from("--stderr-text=quote\"back\\slash'tick`dollar$"),
        OsString::from("--stderr-text=uniçødé-字-🦀"),
        OsString::from("--stderr-text="),
        OsString::from("--stderr-text=--mode"),
    ];
    let mut argv = vec![probe.into_os_string(), OsString::from("--print-argv")];
    argv.extend(tricky.clone());
    let out = crate::exec::spawn(&argv, session.path(), &env).expect("probe runs");
    assert_eq!(out.code, Some(0));
    assert_eq!(out.stderr, b"--mode");
    let mut lines: Vec<String> = String::from_utf8(out.stdout)
        .expect("probe prints text")
        .split('\n')
        .map(str::to_owned)
        .collect();
    lines.pop();
    let expected: Vec<String> = std::iter::once("--print-argv".to_owned())
        .chain(tricky.iter().map(|arg| arg.to_string_lossy().into_owned()))
        .collect();
    assert_eq!(lines, expected);
}

#[test]
fn spotbugs_check_rejects_aux_jar_that_cannot_be_joined() {
    let bad = if cfg!(windows) {
        "/scratch/a\".jar"
    } else {
        "/scratch/a:b.jar"
    };
    let java = Path::new("/scratch/src/Hello.java");
    assert!(spotbugs_check(Path::new(BIN), &[java], &[Path::new(bad)]).is_err());
    let colon_target = Path::new("/scratch/src/He:llo.java");
    assert!(spotbugs_check(Path::new(BIN), &[colon_target], &[]).is_ok());
}

fn infallible(invocation: Invocation) -> Result<Invocation, std::convert::Infallible> {
    Ok(invocation)
}

#[test]
fn invocation_batches_count_per_file_flags() {
    let root = PathBuf::from("/scratch/root");
    let owned: Vec<PathBuf> = (0..400)
        .map(|index| {
            PathBuf::from(format!(
                "/scratch/root/p/{index:04}-{}.proto",
                "q".repeat(60)
            ))
        })
        .collect();
    let refs: Vec<&Path> = owned.iter().map(PathBuf::as_path).collect();
    let build = |files: &[&Path]| infallible(buf_format_fix(Path::new(BIN), files, &root));
    let batches = plan_invocation_batches(&refs, build).expect("planned");
    assert!(batches.len() > 1, "per-file flags force more batches");
    let roundtrip: Vec<&Path> = batches.iter().flatten().copied().collect();
    assert_eq!(roundtrip, refs);
    for batch in &batches {
        let (bytes, units) = argv_cost(&buf_format_fix(Path::new(BIN), batch, &root).argv);
        assert!(bytes <= ARGV_BATCH_BUDGET, "bytes {bytes} fit");
        assert!(units <= ARGV_BATCH_BUDGET, "units {units} fit");
    }
}

#[test]
fn invocation_batches_keep_order_and_isolate_oversized() {
    let huge = PathBuf::from(format!("/s/{}", "h".repeat(500)));
    let small: Vec<PathBuf> = (0..6)
        .map(|index| PathBuf::from(format!("/s/{index}.txt")))
        .collect();
    let refs: Vec<&Path> = vec![
        small[0].as_path(),
        small[1].as_path(),
        huge.as_path(),
        small[2].as_path(),
        small[3].as_path(),
        small[4].as_path(),
        small[5].as_path(),
    ];
    let build = |files: &[&Path]| infallible(keep_sorted_fix(Path::new(BIN), files));
    let batches = plan_invocation_batches_with_budget(&refs, build, 200).expect("planned");
    let roundtrip: Vec<&Path> = batches.iter().flatten().copied().collect();
    assert_eq!(roundtrip, refs);
    assert!(batches.contains(&vec![huge.as_path()]), "{batches:?}");
    let empty = plan_invocation_batches(&[], build).expect("planned");
    assert_eq!(empty, vec![Vec::<&Path>::new()]);
    let failing = plan_invocation_batches(&refs, |_: &[&Path]| Err::<Invocation, _>("no build"));
    assert_eq!(failing, Err("no build"));
}

#[test]
fn invocation_batches_respect_launcher_budget() {
    let owned: Vec<PathBuf> = (0..200)
        .map(|index| PathBuf::from(format!("/s/{index:04}-{}.txt", "p".repeat(60))))
        .collect();
    let refs: Vec<&Path> = owned.iter().map(PathBuf::as_path).collect();
    let build = |files: &[&Path]| infallible(keep_sorted_fix(Path::new("/bin/tool.cmd"), files));
    let batches = plan_invocation_batches(&refs, build).expect("planned");
    assert!(batches.len() > 2, "launcher budget forces batches");
    for batch in &batches {
        let (bytes, units) = argv_cost(&keep_sorted_fix(Path::new("/bin/tool.cmd"), batch).argv);
        assert!(bytes <= LAUNCHER_BATCH_BUDGET, "bytes {bytes} fit");
        assert!(units <= LAUNCHER_BATCH_BUDGET, "units {units} fit");
    }
}

const KUBECONFORM_CONFIG: &str = r#"{"schema_version":1,"kubernetes_version":"1.31.0","strict":true,"skip_kinds":["CustomThing","Other"],"core_dir":"k.kubeconform/core","crd_dir":"k.kubeconform/crd"}"#;

#[test]
fn kubeconform_pins_local_schema_locations() {
    let config_path = Path::new("/scratch/cfg/k.kubeconform.json");
    let config =
        kubeconform_config(config_path, KUBECONFORM_CONFIG.as_bytes()).expect("config parses");
    assert_eq!(
        config.core_dir,
        Path::new("/scratch/cfg").join("k.kubeconform").join("core")
    );
    let doc = Path::new("/scratch/dx-kubeconform/docs/0.yaml");
    let invocation = kubeconform_check(Path::new(BIN), &config, &[doc]).expect("argv");
    let core = format!(
        "{}/{{{{.ResourceKind}}}}{{{{.KindSuffix}}}}.json",
        config.core_dir.display()
    );
    let crd = format!(
        "{}/{{{{.Group}}}}/{{{{.ResourceKind}}}}_{{{{.ResourceAPIVersion}}}}.json",
        config.crd_dir.display()
    );
    assert_eq!(
        argv_strings(&invocation),
        vec![
            BIN.to_owned(),
            "-output".to_owned(),
            "json".to_owned(),
            "-summary=false".to_owned(),
            "-kubernetes-version".to_owned(),
            "1.31.0".to_owned(),
            "-strict".to_owned(),
            "-skip".to_owned(),
            "CustomThing,Other".to_owned(),
            "-schema-location".to_owned(),
            core,
            "-schema-location".to_owned(),
            crd,
            "/scratch/dx-kubeconform/docs/0.yaml".to_owned(),
        ]
    );
    assert!(!argv_strings(&invocation)
        .iter()
        .any(|arg| arg.contains("ignore-missing-schemas")));
    let loose =
        r#"{"schema_version":1,"kubernetes_version":"1.31.0","core_dir":"c","crd_dir":"d"}"#;
    let config = kubeconform_config(config_path, loose.as_bytes()).expect("defaults");
    let argv = argv_strings(&kubeconform_check(Path::new(BIN), &config, &[doc]).expect("argv"));
    assert!(!argv.contains(&"-strict".to_owned()));
    assert!(!argv.contains(&"-skip".to_owned()));
    assert_eq!(
        argv.iter().filter(|arg| *arg == "-schema-location").count(),
        2
    );
}

#[test]
fn kubeconform_config_refuses_remote_or_escaping_locations() {
    let config_path = Path::new("/scratch/cfg/k.kubeconform.json");
    for (core, wanted) in [
        ("default", "local directory"),
        ("https://example.com/schemas", "local directory"),
        ("../up", "relative forward-slash"),
        ("/abs", "relative forward-slash"),
        ("a\\\\b", "relative forward-slash"),
        ("", "relative forward-slash"),
    ] {
        let text = format!(
            r#"{{"schema_version":1,"kubernetes_version":"1.31.0","core_dir":"{core}","crd_dir":"d"}}"#
        );
        let err = kubeconform_config(config_path, text.as_bytes()).expect_err(core);
        assert!(err.contains(wanted), "{core}: {err}");
    }
    let bad = [
        r#"{"schema_version":2,"kubernetes_version":"1.31.0","core_dir":"c","crd_dir":"d"}"#,
        r#"{"schema_version":1,"kubernetes_version":"1.31.0","core_dir":"c","crd_dir":"d","extra":1}"#,
        r#"{"schema_version":1,"kubernetes_version":"","core_dir":"c","crd_dir":"d"}"#,
        r#"{"schema_version":1,"kubernetes_version":"1.31.0","skip_kinds":["a,b"],"core_dir":"c","crd_dir":"d"}"#,
        r#"{"schema_version":1,"kubernetes_version":"1.31.0","core_dir":"c"}"#,
    ];
    for text in bad {
        assert!(
            kubeconform_config(config_path, text.as_bytes()).is_err(),
            "{text}"
        );
    }
    let ok = r#"{"schema_version":1,"kubernetes_version":"1.31.0","core_dir":"c","crd_dir":"d"}"#;
    assert!(kubeconform_config(Path::new("/scratch/cfg/k.json"), ok.as_bytes()).is_err());
    let remote = KubeconformConfig {
        kubernetes_version: "1.31.0".to_owned(),
        strict: false,
        skip_kinds: Vec::new(),
        core_dir: PathBuf::from("https://example.com"),
        crd_dir: PathBuf::from("/d"),
    };
    assert!(kubeconform_check(Path::new(BIN), &remote, &[]).is_err());
}

#[test]
fn flatc_check_parses_one_schema_and_writes_nothing() {
    let invocation = flatc_check(
        Path::new(BIN),
        Path::new("/scratch/a/x.fbs"),
        Path::new("/scratch"),
        Path::new("/scratch/out"),
    );
    assert_eq!(
        argv_strings(&invocation),
        vec![
            BIN,
            "-M",
            "--cpp",
            "-o",
            "/scratch/out",
            "-I",
            "/scratch",
            "/scratch/a/x.fbs"
        ]
    );
    assert_eq!(invocation.cwd_rel, "");
}

#[test]
fn helm_argv_keeps_value_order_and_never_fetches() {
    let values = [Path::new("/c/a.yaml"), Path::new("/c/b.yaml")];
    let lint = argv_strings(&helm_lint(
        Path::new(BIN),
        Path::new("/c"),
        &values,
        "1.31.0",
    ));
    assert_eq!(
        lint,
        [
            BIN,
            "lint",
            "/c",
            "--quiet",
            "--kube-version",
            "1.31.0",
            "-f",
            "/c/a.yaml",
            "-f",
            "/c/b.yaml"
        ]
    );
    let template = argv_strings(&helm_template(
        Path::new(BIN),
        "dx",
        "default",
        Path::new("/c"),
        &values,
        "1.31.0",
    ));
    assert_eq!(
        template,
        [
            BIN,
            "template",
            "dx",
            "/c",
            "--namespace",
            "default",
            "--kube-version",
            "1.31.0",
            "-f",
            "/c/a.yaml",
            "-f",
            "/c/b.yaml"
        ]
    );
    for argv in [&lint, &template] {
        for forbidden in [
            "dependency",
            "repo",
            "pull",
            "--repo",
            "--dependency-update",
        ] {
            assert!(!argv.iter().any(|arg| arg == forbidden), "{forbidden}");
        }
    }
}

#[test]
fn helm_is_a_single_transport_tool() {
    assert_eq!(transport_for("helm"), ArgTransport::Single);
}

#[test]
fn dialyzer_check_orders_plts_before_options_and_beams_after_the_separator() {
    let invocation = dialyzer_check(
        Path::new(BIN),
        &[Path::new("/s/pa")],
        &[Path::new("/s/core.plt"), Path::new("/s/ext.plt")],
        &[Path::new("/s/a.beam"), Path::new("/s/b.beam")],
    );
    assert_eq!(
        argv_strings(&invocation),
        vec![
            BIN,
            "-pa",
            "/s/pa",
            "--plts",
            "/s/core.plt",
            "/s/ext.plt",
            "--no_check_plt",
            "--no_indentation",
            "--error_location",
            "column",
            "--fullpath",
            "--",
            "/s/a.beam",
            "/s/b.beam"
        ]
    );
    let bare = dialyzer_check(Path::new(BIN), &[], &[], &[Path::new("/s/a.beam")]);
    assert!(!argv_strings(&bare).contains(&"--plts".to_owned()));
    assert!(!argv_strings(&bare).contains(&"-pa".to_owned()));
}

#[test]
fn phpcs_check_always_names_the_standard_and_puts_paths_after_the_separator() {
    let expected = |standard: &str| -> Vec<String> {
        [
            BIN,
            "--report=json",
            "-q",
            "--no-colors",
            "--no-cache",
            standard,
            "--",
            "/s/a.php",
            "/s/b.php",
        ]
        .iter()
        .map(|arg| (*arg).to_owned())
        .collect()
    };
    let paths = [Path::new("/s/a.php"), Path::new("/s/b.php")];
    let consumer = phpcs_check(Path::new(BIN), OsStr::new("/s/phpcs.xml"), &paths);
    assert_eq!(argv_strings(&consumer), expected("--standard=/s/phpcs.xml"));
    let bare = phpcs_check(Path::new(BIN), OsStr::new("PSR12"), &paths);
    assert_eq!(argv_strings(&bare), expected("--standard=PSR12"));
}

#[test]
fn phpstan_check_passes_the_config_and_puts_paths_after_the_separator() {
    let invocation = phpstan_check(
        Path::new(BIN),
        Path::new("/s/phpstan.neon"),
        &[Path::new("/s/a.php"), Path::new("/s/b.php")],
    );
    assert_eq!(
        argv_strings(&invocation),
        vec![
            BIN,
            "analyse",
            "--no-progress",
            "--no-interaction",
            "--error-format=json",
            "-c",
            "/s/phpstan.neon",
            "--",
            "/s/a.php",
            "/s/b.php"
        ]
    );
}

#[test]
fn phpstan_neon_includes_the_consumer_config_and_pins_our_values() {
    let neon = phpstan_neon(
        Some(Path::new("/s/phpstan.neon")),
        5,
        Path::new("/s/.tmp"),
        &[Path::new("/s/scan"), Path::new("/s/pkg")],
    );
    assert_eq!(
        neon,
        "includes:\n    - \"/s/phpstan.neon\"\nparameters:\n    tmpDir: \"/s/.tmp\"\n    scanDirectories:\n        - \"/s/scan\"\n        - \"/s/pkg\"\n    parallel:\n        maximumNumberOfProcesses: 1\n"
    );
    let bare = phpstan_neon(None, 5, Path::new("/s/.tmp"), &[]);
    assert!(
        bare.contains("level: 5")
            && !bare.contains("includes")
            && !bare.contains("scanDirectories")
    );
    let odd = phpstan_neon(None, 5, Path::new("/s/a\"b"), &[]);
    assert!(odd.contains("\"/s/a\\\"b\""), "{odd}");
}

#[test]
fn ansible_lint_fix_rewrites_in_place_offline() {
    let file = Path::new("/scratch/site.yml");
    let context = AnsibleLintContext {
        config: Some(Path::new("/scratch/.ansible-lint")),
        collections: &[],
        project_dir: Path::new("/scratch"),
    };
    assert_eq!(
        argv_strings(&ansible_lint_fix(Path::new(BIN), &[file], &context)),
        [
            BIN,
            "--offline",
            "--nocolor",
            "--fix=all",
            "--project-dir",
            "/scratch",
            "-c",
            "/scratch/.ansible-lint",
            "/scratch/site.yml",
        ]
    );
}

#[test]
fn check_jsonschema_argv_is_offline_and_verbose() {
    let a = Path::new("/scratch/a.yaml");
    let b = Path::new("/scratch/b.yaml");
    assert_eq!(
        argv_strings(&check_jsonschema_check(
            Path::new(BIN),
            Path::new("/scratch/refs.json"),
            Path::new("/cfg/s.json"),
            "yaml",
            &[a, b]
        )),
        vec![
            BIN,
            "--dx-ref-map",
            "/scratch/refs.json",
            "--schemafile",
            "/cfg/s.json",
            "--no-cache",
            "--output-format",
            "json",
            "--verbose",
            "--verbose",
            "--force-filetype",
            "yaml",
            "--",
            "/scratch/a.yaml",
            "/scratch/b.yaml"
        ]
    );
    assert_eq!(transport_for("check_jsonschema"), ArgTransport::Batched);
}

#[test]
fn luals_check_keeps_every_write_below_the_work_directory() {
    let invocation = luals_check(
        Path::new(BIN),
        Path::new("/s"),
        Path::new("/s/.dx-luals"),
        Path::new("/s/.dx-luals/check.json"),
        Path::new("/s/.dx-luals/luarc.json"),
    );
    assert_eq!(
        argv_strings(&invocation),
        vec![
            BIN,
            "--check=/s",
            "--checklevel=Warning",
            "--check_format=json",
            "--check_out_path=/s/.dx-luals/check.json",
            "--logpath=/s/.dx-luals/log",
            "--metapath=/s/.dx-luals/meta",
            "--configpath=/s/.dx-luals/luarc.json",
        ]
    );
}

#[test]
fn luals_luarc_lets_our_keys_win_over_the_consumer_config() {
    let consumer: serde_json::Value = serde_json::from_str(
        r#"{"$schema":"x","runtime.version":"LuaJIT","workspace.library":["/elsewhere"],"diagnostics.globals":["vim"]}"#,
    )
    .unwrap();
    let text = luals_luarc(
        Some(&consumer),
        Some(Path::new("/s/dx-luals/mod")),
        &["dx-luals"],
    );
    let parsed: serde_json::Value = serde_json::from_str(&text).unwrap();
    assert_eq!(parsed["runtime.version"], "Lua 5.4");
    assert_eq!(
        parsed["workspace.library"],
        serde_json::json!(["/s/dx-luals/mod"])
    );
    assert_eq!(parsed["diagnostics.globals"], serde_json::json!(["vim"]));
    assert_eq!(parsed["diagnostics.libraryFiles"], "Disable");
    assert_eq!(parsed["workspace.checkThirdParty"], false);
    assert_eq!(
        parsed["workspace.ignoreDir"],
        serde_json::json!(["dx-luals"])
    );
    assert!(parsed.get("$schema").is_none());
    let bare: serde_json::Value = serde_json::from_str(&luals_luarc(None, None, &[])).unwrap();
    assert_eq!(bare["workspace.library"], serde_json::json!([]));
}

#[test]
fn capnp_check_compiles_to_stdout_inside_one_import_root() {
    let invocation = capnp_check(
        Path::new(BIN),
        Path::new("pkg/x.capnp"),
        Path::new("/scratch"),
    );
    assert_eq!(
        argv_strings(&invocation),
        vec![
            BIN,
            "compile",
            "--no-standard-import",
            "-I",
            "/scratch",
            "--src-prefix",
            "/scratch",
            "-o-",
            "pkg/x.capnp"
        ]
    );
    assert_eq!(invocation.cwd_rel, "");
}

#[test]
fn sorbet_check_ignores_the_source_tree_config_and_takes_every_path_after_a_separator() {
    let invocation = sorbet_check(
        Path::new(BIN),
        &[Path::new("/s/a.rb"), Path::new("/s/dx-sorbet/dep/b.rb")],
    );
    assert_eq!(
        argv_strings(&invocation),
        vec![
            BIN,
            "--no-config",
            "--color=never",
            "--no-error-sections",
            "--silence-dev-message",
            "--",
            "/s/a.rb",
            "/s/dx-sorbet/dep/b.rb",
        ]
    );
    assert!(!argv_strings(&invocation)
        .iter()
        .any(|arg| arg.contains("cache-dir")));
}

#[test]
fn jet_check_writes_only_the_report() {
    let invocation = jet_check(
        Path::new(BIN),
        Path::new("/s/dx-jet/config.toml"),
        Path::new("/s"),
        Path::new("/s/.dx-jet/report.json"),
    );
    assert_eq!(
        argv_strings(&invocation),
        vec![
            BIN,
            "--config",
            "/s/dx-jet/config.toml",
            "--scratch",
            "/s",
            "--report",
            "/s/.dx-jet/report.json",
        ]
    );
}

#[test]
fn thrift_check_is_strict_verbose_and_writes_only_to_out() {
    let invocation = thrift_check(
        Path::new(BIN),
        Path::new("/scratch/pkg/x.thrift"),
        Path::new("/scratch"),
        Path::new("/scratch/.dx-thrift-out"),
    );
    assert_eq!(
        argv_strings(&invocation),
        vec![
            BIN,
            "-strict",
            "-v",
            "-I",
            "/scratch",
            "-out",
            "/scratch/.dx-thrift-out",
            "--gen",
            "json",
            "/scratch/pkg/x.thrift"
        ]
    );
    assert_eq!(invocation.cwd_rel, "");
}
