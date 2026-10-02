use std::ffi::OsString;
use std::path::Path;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Invocation {
    pub argv: Vec<OsString>,
    pub cwd_rel: String,
}

const BUILDIFIER_CHECK_ARGS: &[&str] = &["--mode=check", "--format=json", "--lint=warn"];
const BUILDIFIER_CHECK_BARE_ARGS: &[&str] = &[
    "--mode=check",
    "--format=json",
    "--lint=warn",
    "--config=off",
];
const BUILDIFIER_FIX_ARGS: &[&str] = &["--mode=fix", "--lint=fix"];
const BUILDIFIER_FIX_BARE_ARGS: &[&str] = &["--mode=fix", "--lint=fix", "--config=off"];
const RUSTFMT_PREFIX: &[&str] = &["--edition"];
const RUSTFMT_MIDDLE: &[&str] = &["--color=never", "--config-path"];
const RUSTFMT_CHECK_FLAG: &[&str] = &["--check"];
const TAPLO_LINT_BASE: &[&str] = &["lint", "--colors=never", "--no-auto-config", "--no-schema"];
const TAPLO_FORMAT_BASE: &[&str] = &["format", "--colors=never", "--no-auto-config"];
const TAPLO_FORMAT_CHECK_FLAG: &[&str] = &["--check"];
const TAPLO_CONFIG_FLAG: &[&str] = &["-c"];
const VALE_FIXED: &[&str] = &["--output=JSON", "--no-color", "--no-global", "--no-wrap"];
const RUFF_CHECK_SUBCOMMAND: &[&str] = &["check"];
const RUFF_FORMAT_SUBCOMMAND: &[&str] = &["format"];
const RUFF_HERMETIC_TAIL: &[&str] = &["--no-cache", "--no-respect-gitignore"];
const RUFF_CHECK_TAIL: &[&str] = &["--output-format", "json"];
const RUFF_FIX_TAIL: &[&str] = &["--fix"];
const RUFF_FORMAT_CHECK_TAIL: &[&str] = &["--check", "--output-format", "json"];
const TY_BASE: &[&str] = &[
    "check",
    "--output-format",
    "concise",
    "--no-progress",
    "--no-respect-ignore-files",
];
const PYDOCLINT_ARGS: &[&str] = &["--quiet"];
const FLAKE8_ARGS: &[&str] = &[
    "--isolated",
    "--color=never",
    "--jobs=1",
    "--format",
    "%(path)s:%(row)s:%(col)s:%(code)s:%(text)s",
];
const PYLINT_ARGS: &[&str] = &[
    "--persistent=n",
    "--reports=n",
    "--score=n",
    "--output-format=json",
    "--jobs=1",
];
const BIOME_LINT_SUBCOMMAND: &[&str] = &["lint"];
const BIOME_FORMAT_SUBCOMMAND: &[&str] = &["format"];
const BIOME_BASE_TAIL: &[&str] = &["--reporter=json", "--colors=off", "--config-path"];
const BIOME_LINT_TAIL: &[&str] = &["--error-on-warnings", "--vcs-enabled=false"];
const BIOME_FORMAT_FIX_TAIL: &[&str] = &["--write"];
const ESLINT_MIDDLE: &[&str] = &["-f", "json"];
const ESLINT_FIX_SUFFIX: &[&str] = &["-f", "json", "--fix"];
const PRETTIER_BASE: &[&str] = &["--no-config", "--no-editorconfig"];
const PRETTIER_CHECK_TAIL: &[&str] = &["--check"];
const PRETTIER_FIX_TAIL: &[&str] = &["--write"];
const SCALAFMT_CHECK_PREFIX: &[&str] = &["--check"];
const SCALAFMT_EMPTY: &[&str] = &[];
const CSHARPIER_CHECK_PREFIX: &[&str] = &["check"];
const CSHARPIER_FIX_PREFIX: &[&str] = &["format"];
const FANTOMAS_CHECK_ARGS: &[&str] = &["check", "--json"];
const FANTOMAS_FIX_ARGS: &[&str] = &[];
const GOFUMPT_CHECK_ARGS: &[&str] = &["-d"];
const GOFUMPT_FIX_ARGS: &[&str] = &["-w"];
const STATICCHECK_ARGS: &[&str] = &["-f", "json"];
const EMPTY_ARGS: &[&str] = &[];
const ERROR_PRONE_ARGS: &[&str] = &["-Xplugin:ErrorProne"];
const GOOGLE_JAVA_FORMAT_CHECK_ARGS: &[&str] = &["--dry-run", "--set-exit-if-changed"];
const GOOGLE_JAVA_FORMAT_FIX_ARGS: &[&str] = &["--replace"];
const KTFMT_CHECK_ARGS: &[&str] = &["--kotlinlang-style", "--dry-run", "--set-exit-if-changed"];
const KTFMT_FIX_ARGS: &[&str] = &["--kotlinlang-style"];
const CHECKSTYLE_MIDDLE: &[&str] = &["-f", "sarif"];
const PMD_PREFIX: &[&str] = &["check"];
const PMD_MIDDLE: &[&str] = &["--format", "sarif", "--rulesets"];
const PMD_DEFAULT_RULESET: &[&str] = &["rulesets/java/quickstart.xml"];
const SPOTBUGS_ARGS: &[&str] = &["-textui", "-effort:default", "-sarif"];
const KTLINT_CHECK_ARGS: &[&str] = &["--relative", "--log-level=none", "--reporter=sarif"];
const KTLINT_FIX_ARGS: &[&str] = &["--relative", "--format"];
const BUF_LINT_ARGS: &[&str] = &["lint", "--error-format=json"];
const BUF_FORMAT_CHECK_ARGS: &[&str] = &["format", "--diff", "--exit-code"];
const BUF_FORMAT_FIX_ARGS: &[&str] = &["format", "--write"];
const QMLFORMAT_CHECK_ARGS: &[&str] = &["--check"];
const QMLFORMAT_FIX_ARGS: &[&str] = &["-i"];
const QMLLINT_ARGS: &[&str] = &["--json", "-"];
const CUE_CHECK_ARGS: &[&str] = &["fmt", "--check", "--diff"];
const CUE_FIX_ARGS: &[&str] = &["fmt", "--write"];
const JSONNETFMT_CHECK_ARGS: &[&str] = &["--test"];
const JSONNETFMT_FIX_ARGS: &[&str] = &["-i"];
const PKL_CHECK_ARGS: &[&str] = &["format", "--diff-name-only"];
const PKL_FIX_ARGS: &[&str] = &["format", "-w"];
const MODFMT_CHECK_ARGS: &[&str] = &["-c", "-l"];
const MODFMT_FIX_ARGS: &[&str] = &["-w"];
const TERRAFORM_CHECK_ARGS: &[&str] = &["fmt", "-check"];
const TERRAFORM_FIX_ARGS: &[&str] = &["fmt"];
const YAMLFMT_CHECK_ARGS: &[&str] = &["-lint", "-q"];
const YAMLFMT_FIX_ARGS: &[&str] = &[];
const SHFMT_CHECK_ARGS: &[&str] = &["-d"];
const SHFMT_FIX_ARGS: &[&str] = &["-w"];
const STANDARDRB_CHECK_ARGS: &[&str] = &["--format", "json"];
const STANDARDRB_FIX_ARGS: &[&str] = &["--fix"];
const DJLINT_FORMAT_CHECK_ARGS: &[&str] = &["--reformat", "--check"];
const DJLINT_FORMAT_FIX_ARGS: &[&str] = &["--reformat"];
const DJLINT_CHECK_PREFIX: &[&str] = &["--lint"];
const STYLELINT_PREFIX: &[&str] = &["--formatter", "json"];
const RUBOCOP_ARGS: &[&str] = &["--format", "json"];
const YAMLLINT_PREFIX: &[&str] = &["-f", "parsable"];
const SHELLCHECK_ARGS: &[&str] = &["--format=gcc"];
const CLANG_FORMAT_CHECK_PREFIX: &[&str] = &["--dry-run", "--Werror"];
const CLANG_FORMAT_FIX_PREFIX: &[&str] = &["-i"];
const CLANG_TIDY_PREFIX: &[&str] = &["--quiet"];
const CPPCHECK_PREFIX: &[&str] = &["--xml", "--xml-version=2"];
const FSHARPLINT_PROJECT_FLAG: &[&str] = &["--project"];
const FSHARPLINT_CONFIG_FLAG: &[&str] = &["--config"];
const SCALAFIX_SOURCEROOT_FLAG: &[&str] = &["--sourceroot"];
const SCALAFIX_CLASSPATH_FLAG: &[&str] = &["--classpath"];
const SCALAFIX_SEMANTICDB_FLAG: &[&str] = &["--semanticdb-targetroots"];

fn invocation(binary: &Path, args: &[&str], files: &[&Path], cwd_rel: &str) -> Invocation {
    let mut argv = Vec::with_capacity(1 + args.len() + files.len());
    argv.push(binary.as_os_str().to_owned());
    argv.extend(args.iter().map(OsString::from));
    argv.extend(files.iter().map(|path| path.as_os_str().to_owned()));
    Invocation {
        argv,
        cwd_rel: cwd_rel.to_owned(),
    }
}

fn assemble(_binary: &Path, head: Vec<OsString>, files: &[&Path], cwd_rel: &str) -> Invocation {
    let mut argv = head;
    argv.reserve(files.len());
    argv.extend(files.iter().map(|path| path.as_os_str().to_owned()));
    Invocation {
        argv,
        cwd_rel: cwd_rel.to_owned(),
    }
}

fn fixed(binary: &Path, args: &[&str], files: &[&Path], cwd_rel: &str) -> Invocation {
    invocation(binary, args, files, cwd_rel)
}

fn optional_flag(
    binary: &Path,
    prefix: &[&str],
    flag: &str,
    config: Option<&Path>,
    suffix: &[&str],
    files: &[&Path],
    cwd_rel: &str,
) -> Invocation {
    let mut head = Vec::with_capacity(1 + prefix.len() + suffix.len() + 2);
    head.push(binary.as_os_str().to_owned());
    head.extend(prefix.iter().map(OsString::from));
    if let Some(path) = config {
        head.push(OsString::from(flag));
        head.push(path.as_os_str().to_owned());
    }
    head.extend(suffix.iter().map(OsString::from));
    assemble(binary, head, files, cwd_rel)
}

fn optional_joined(
    binary: &Path,
    prefix: &[&str],
    joined: &str,
    config: Option<&Path>,
    suffix: &[&str],
    files: &[&Path],
    cwd_rel: &str,
) -> Invocation {
    let mut head = Vec::with_capacity(1 + prefix.len() + suffix.len() + 1);
    head.push(binary.as_os_str().to_owned());
    head.extend(prefix.iter().map(OsString::from));
    if let Some(path) = config {
        head.push(OsString::from(format!(
            "{joined}{}",
            path.to_string_lossy()
        )));
    }
    head.extend(suffix.iter().map(OsString::from));
    assemble(binary, head, files, cwd_rel)
}

fn required_flag(
    binary: &Path,
    prefix: &[&str],
    flag: &str,
    config: &Path,
    suffix: &[&str],
    files: &[&Path],
    cwd_rel: &str,
) -> Invocation {
    let mut head = Vec::with_capacity(1 + prefix.len() + suffix.len() + 2);
    head.push(binary.as_os_str().to_owned());
    head.extend(prefix.iter().map(OsString::from));
    head.push(OsString::from(flag));
    head.push(config.as_os_str().to_owned());
    head.extend(suffix.iter().map(OsString::from));
    assemble(binary, head, files, cwd_rel)
}

fn cwd_invocation(
    binary: &Path,
    hinted: &[&str],
    bare: &[&str],
    files: &[&Path],
    config_dir_rel: Option<&str>,
) -> Invocation {
    match config_dir_rel {
        Some(dir) => invocation(binary, hinted, files, dir),
        None => invocation(binary, bare, files, ""),
    }
}

pub fn buildifier_check(
    binary: &Path,
    files: &[&Path],
    config_dir_rel: Option<&str>,
) -> Invocation {
    cwd_invocation(
        binary,
        BUILDIFIER_CHECK_ARGS,
        BUILDIFIER_CHECK_BARE_ARGS,
        files,
        config_dir_rel,
    )
}

pub fn buildifier_fix(binary: &Path, files: &[&Path], config_dir_rel: Option<&str>) -> Invocation {
    cwd_invocation(
        binary,
        BUILDIFIER_FIX_ARGS,
        BUILDIFIER_FIX_BARE_ARGS,
        files,
        config_dir_rel,
    )
}

pub fn rustfmt(
    binary: &Path,
    files: &[&Path],
    config: &Path,
    edition: &str,
    check: bool,
) -> Invocation {
    let mut head = vec![
        binary.as_os_str().to_owned(),
        OsString::from(RUSTFMT_PREFIX[0]),
        OsString::from(edition),
        OsString::from(RUSTFMT_MIDDLE[0]),
        OsString::from(RUSTFMT_MIDDLE[1]),
        config.as_os_str().to_owned(),
    ];
    if check {
        head.push(OsString::from(RUSTFMT_CHECK_FLAG[0]));
    }
    assemble(binary, head, files, "")
}

pub fn taplo_lint(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(
        binary,
        TAPLO_LINT_BASE,
        TAPLO_CONFIG_FLAG[0],
        config,
        EMPTY_ARGS,
        files,
        "",
    )
}

pub fn taplo_format(
    binary: &Path,
    files: &[&Path],
    config: Option<&Path>,
    check: bool,
) -> Invocation {
    let mut head = vec![binary.as_os_str().to_owned()];
    head.extend(TAPLO_FORMAT_BASE.iter().map(OsString::from));
    if check {
        head.extend(TAPLO_FORMAT_CHECK_FLAG.iter().map(OsString::from));
    }
    if let Some(path) = config {
        head.extend(TAPLO_CONFIG_FLAG.iter().map(OsString::from));
        head.push(path.as_os_str().to_owned());
    }
    assemble(binary, head, files, "")
}

pub fn vale_check(binary: &Path, ini: &Path, files: &[&Path], ini_dir_rel: &str) -> Invocation {
    let mut head = vec![
        binary.as_os_str().to_owned(),
        OsString::from("--config"),
        ini.as_os_str().to_owned(),
    ];
    head.extend(VALE_FIXED.iter().map(OsString::from));
    assemble(binary, head, files, ini_dir_rel)
}

fn ruff_base(binary: &Path, subcommand: &str, config: Option<&Path>) -> Vec<OsString> {
    let mut argv = vec![binary.as_os_str().to_owned(), OsString::from(subcommand)];
    if let Some(path) = config {
        argv.push(OsString::from("--config"));
        argv.push(path.as_os_str().to_owned());
    } else {
        argv.push(OsString::from("--isolated"));
    }
    argv.extend(RUFF_HERMETIC_TAIL.iter().map(OsString::from));
    argv
}

pub fn ruff_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    let mut head = ruff_base(binary, RUFF_CHECK_SUBCOMMAND[0], config);
    head.extend(RUFF_CHECK_TAIL.iter().map(OsString::from));
    assemble(binary, head, files, "")
}

pub fn ruff_fix(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    let mut head = ruff_base(binary, RUFF_CHECK_SUBCOMMAND[0], config);
    head.extend(RUFF_FIX_TAIL.iter().map(OsString::from));
    assemble(binary, head, files, "")
}

pub fn ruff_format_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    let mut head = ruff_base(binary, RUFF_FORMAT_SUBCOMMAND[0], config);
    head.extend(RUFF_FORMAT_CHECK_TAIL.iter().map(OsString::from));
    assemble(binary, head, files, "")
}

pub fn ruff_format_fix(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    let head = ruff_base(binary, RUFF_FORMAT_SUBCOMMAND[0], config);
    assemble(binary, head, files, "")
}

pub fn ty_check(binary: &Path, files: &[&Path], search_paths: &[&Path]) -> Invocation {
    let mut head = Vec::with_capacity(1 + TY_BASE.len() + 2 * search_paths.len());
    head.push(binary.as_os_str().to_owned());
    head.extend(TY_BASE.iter().map(OsString::from));
    for dir in search_paths {
        head.push(OsString::from("--extra-search-path"));
        head.push(dir.as_os_str().to_owned());
    }
    assemble(binary, head, files, "")
}

pub fn pydoclint_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, PYDOCLINT_ARGS, files, "")
}

pub fn flake8_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, FLAKE8_ARGS, files, "")
}

pub fn pylint_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, PYLINT_ARGS, files, "")
}

fn biome_base(binary: &Path, subcommand: &str, config_dir: &Path) -> Vec<OsString> {
    let mut head = vec![binary.as_os_str().to_owned(), OsString::from(subcommand)];
    head.extend(BIOME_BASE_TAIL[..2].iter().map(OsString::from));
    head.push(OsString::from(BIOME_BASE_TAIL[2]));
    head.push(config_dir.as_os_str().to_owned());
    head
}

pub fn biome_lint_check(binary: &Path, files: &[&Path], config_dir: &Path) -> Invocation {
    let mut head = biome_base(binary, BIOME_LINT_SUBCOMMAND[0], config_dir);
    head.extend(BIOME_LINT_TAIL.iter().map(OsString::from));
    assemble(binary, head, files, "")
}

pub fn biome_format_check(binary: &Path, files: &[&Path], config_dir: &Path) -> Invocation {
    let head = biome_base(binary, BIOME_FORMAT_SUBCOMMAND[0], config_dir);
    assemble(binary, head, files, "")
}

pub fn biome_format_fix(binary: &Path, files: &[&Path], config_dir: &Path) -> Invocation {
    let mut head = biome_base(binary, BIOME_FORMAT_SUBCOMMAND[0], config_dir);
    head.extend(BIOME_FORMAT_FIX_TAIL.iter().map(OsString::from));
    assemble(binary, head, files, "")
}

pub fn eslint_check(binary: &Path, files: &[&Path], config: &Path) -> Invocation {
    required_flag(
        binary,
        SCALAFMT_EMPTY,
        "-c",
        config,
        ESLINT_MIDDLE,
        files,
        "",
    )
}

pub fn eslint_fix(binary: &Path, files: &[&Path], config: &Path) -> Invocation {
    required_flag(
        binary,
        SCALAFMT_EMPTY,
        "-c",
        config,
        ESLINT_FIX_SUFFIX,
        files,
        "",
    )
}

fn prettier_base(binary: &Path) -> Vec<OsString> {
    let mut head = vec![binary.as_os_str().to_owned()];
    head.extend(PRETTIER_BASE.iter().map(OsString::from));
    head
}

pub fn prettier_check(binary: &Path, files: &[&Path]) -> Invocation {
    let mut head = prettier_base(binary);
    head.extend(PRETTIER_CHECK_TAIL.iter().map(OsString::from));
    assemble(binary, head, files, "")
}

pub fn prettier_fix(binary: &Path, files: &[&Path]) -> Invocation {
    let mut head = prettier_base(binary);
    head.extend(PRETTIER_FIX_TAIL.iter().map(OsString::from));
    assemble(binary, head, files, "")
}

pub fn markdown_check(
    binary: &Path,
    sources: &[(&str, &Path)],
    siblings: &[(&str, &Path)],
) -> Invocation {
    let mut argv = Vec::with_capacity(1 + 2 * (sources.len() + siblings.len()));
    argv.push(binary.as_os_str().to_owned());
    for (workspace, absolute) in sources {
        argv.push(OsString::from("--source"));
        let mut mapping = OsString::from(workspace);
        mapping.push(OsString::from("="));
        mapping.push(absolute.as_os_str());
        argv.push(mapping);
    }
    for (workspace, absolute) in siblings {
        argv.push(OsString::from("--sibling"));
        let mut mapping = OsString::from(workspace);
        mapping.push(OsString::from("="));
        mapping.push(absolute.as_os_str());
        argv.push(mapping);
    }
    Invocation {
        argv,
        cwd_rel: String::new(),
    }
}

pub fn scalafmt_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(
        binary,
        SCALAFMT_CHECK_PREFIX,
        "--config",
        config,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

pub fn scalafmt_fix(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(
        binary,
        SCALAFMT_EMPTY,
        "--config",
        config,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

pub fn scalafix_check(
    binary: &Path,
    files: &[&Path],
    sourceroot: Option<&Path>,
    classpath: Option<&str>,
    semanticdb_targetroots: Option<&Path>,
) -> Invocation {
    let mut head = vec![binary.as_os_str().to_owned()];
    if let Some(root) = sourceroot {
        head.extend(SCALAFIX_SOURCEROOT_FLAG.iter().map(OsString::from));
        head.push(root.as_os_str().to_owned());
    }
    if let Some(cp) = classpath {
        head.extend(SCALAFIX_CLASSPATH_FLAG.iter().map(OsString::from));
        head.push(OsString::from(cp));
    }
    if let Some(roots) = semanticdb_targetroots {
        head.extend(SCALAFIX_SEMANTICDB_FLAG.iter().map(OsString::from));
        head.push(roots.as_os_str().to_owned());
    }
    assemble(binary, head, files, "")
}

pub fn csharpier_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(
        binary,
        CSHARPIER_CHECK_PREFIX,
        "--config-path",
        config,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

pub fn csharpier_fix(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(
        binary,
        CSHARPIER_FIX_PREFIX,
        "--config-path",
        config,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

pub fn fantomas_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, FANTOMAS_CHECK_ARGS, files, "")
}

pub fn fantomas_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, FANTOMAS_FIX_ARGS, files, "")
}

pub fn fsharplint_check(
    binary: &Path,
    files: &[&Path],
    project: Option<&Path>,
    config: Option<&Path>,
) -> Invocation {
    let mut head = vec![binary.as_os_str().to_owned()];
    if let Some(proj) = project {
        head.extend(FSHARPLINT_PROJECT_FLAG.iter().map(OsString::from));
        head.push(proj.as_os_str().to_owned());
    }
    if let Some(cfg) = config {
        head.extend(FSHARPLINT_CONFIG_FLAG.iter().map(OsString::from));
        head.push(cfg.as_os_str().to_owned());
    }
    assemble(binary, head, files, "")
}

pub fn clang_format_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_joined(
        binary,
        CLANG_FORMAT_CHECK_PREFIX,
        "--style=file:",
        config,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

pub fn clang_format_fix(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_joined(
        binary,
        CLANG_FORMAT_FIX_PREFIX,
        "--style=file:",
        config,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

pub fn gofumpt_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, GOFUMPT_CHECK_ARGS, files, "")
}

pub fn gofumpt_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, GOFUMPT_FIX_ARGS, files, "")
}

pub fn clang_tidy_check(
    binary: &Path,
    files: &[&Path],
    config: Option<&Path>,
    compile_commands_dir: Option<&Path>,
) -> Invocation {
    let mut head = vec![binary.as_os_str().to_owned()];
    head.extend(CLANG_TIDY_PREFIX.iter().map(OsString::from));
    if let Some(cfg) = config {
        head.push(OsString::from("--config-file"));
        head.push(cfg.as_os_str().to_owned());
    }
    if let Some(dir) = compile_commands_dir {
        head.push(OsString::from("-p"));
        head.push(dir.as_os_str().to_owned());
    }
    assemble(binary, head, files, "")
}

pub fn cppcheck_check(binary: &Path, files: &[&Path], suppressions: Option<&Path>) -> Invocation {
    optional_joined(
        binary,
        CPPCHECK_PREFIX,
        "--suppressions-list=",
        suppressions,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

pub fn staticcheck_check(
    binary: &Path,
    files: &[&Path],
    config_dir_rel: Option<&str>,
) -> Invocation {
    cwd_invocation(
        binary,
        STATICCHECK_ARGS,
        STATICCHECK_ARGS,
        files,
        config_dir_rel,
    )
}

pub fn govet_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, EMPTY_ARGS, files, "")
}

pub fn errcheck_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, EMPTY_ARGS, files, "")
}

pub const ERROR_PRONE_PATCH_FILE: &str = "error-prone.patch";

pub fn error_prone_check(javac: &Path, files: &[&Path]) -> Invocation {
    fixed(javac, ERROR_PRONE_ARGS, files, "")
}

pub fn google_java_format_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, GOOGLE_JAVA_FORMAT_CHECK_ARGS, files, "")
}

pub fn google_java_format_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, GOOGLE_JAVA_FORMAT_FIX_ARGS, files, "")
}

pub fn ktfmt_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, KTFMT_CHECK_ARGS, files, "")
}

pub fn ktfmt_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, KTFMT_FIX_ARGS, files, "")
}

pub fn checkstyle_check(binary: &Path, files: &[&Path], config: &Path) -> Invocation {
    required_flag(
        binary,
        SCALAFMT_EMPTY,
        "-c",
        config,
        CHECKSTYLE_MIDDLE,
        files,
        "",
    )
}

pub fn pmd_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    let mut head = vec![binary.as_os_str().to_owned()];
    head.extend(PMD_PREFIX.iter().map(OsString::from));
    for file in files {
        head.push(OsString::from("--dir"));
        head.push(file.as_os_str().to_owned());
    }
    head.extend(PMD_MIDDLE.iter().map(OsString::from));
    match config {
        Some(path) => head.push(path.as_os_str().to_owned()),
        None => head.extend(PMD_DEFAULT_RULESET.iter().map(OsString::from)),
    }
    Invocation {
        argv: head,
        cwd_rel: String::new(),
    }
}

pub fn spotbugs_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, SPOTBUGS_ARGS, files, "")
}

pub fn ktlint_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, KTLINT_CHECK_ARGS, files, "")
}

pub fn ktlint_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, KTLINT_FIX_ARGS, files, "")
}

pub fn error_prone_patch(
    javac: &Path,
    files: &[&Path],
    patch_dir: &Path,
    checks: &str,
) -> Invocation {
    let head = vec![
        javac.as_os_str().to_owned(),
        OsString::from(ERROR_PRONE_ARGS[0]),
        OsString::from(format!("-XepPatchChecks:{checks}")),
        OsString::from(format!("-XepPatchLocation:{}", patch_dir.to_string_lossy())),
    ];
    assemble(javac, head, files, "")
}

pub fn buf_lint_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, BUF_LINT_ARGS, files, "")
}

pub fn buf_format_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, BUF_FORMAT_CHECK_ARGS, files, "")
}

pub fn buf_format_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, BUF_FORMAT_FIX_ARGS, files, "")
}

pub fn qmlformat_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, QMLFORMAT_CHECK_ARGS, files, "")
}

pub fn qmlformat_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, QMLFORMAT_FIX_ARGS, files, "")
}

pub fn qmllint_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, QMLLINT_ARGS, files, "")
}

pub fn cue_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, CUE_CHECK_ARGS, files, "")
}

pub fn cue_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, CUE_FIX_ARGS, files, "")
}

pub fn jsonnetfmt_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, JSONNETFMT_CHECK_ARGS, files, "")
}

pub fn jsonnetfmt_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, JSONNETFMT_FIX_ARGS, files, "")
}

pub fn pkl_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, PKL_CHECK_ARGS, files, "")
}

pub fn pkl_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, PKL_FIX_ARGS, files, "")
}

pub fn modfmt_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, MODFMT_CHECK_ARGS, files, "")
}

pub fn modfmt_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, MODFMT_FIX_ARGS, files, "")
}

pub fn terraform_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, TERRAFORM_CHECK_ARGS, files, "")
}

pub fn terraform_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, TERRAFORM_FIX_ARGS, files, "")
}

pub fn yamlfmt_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, YAMLFMT_CHECK_ARGS, files, "")
}

pub fn yamlfmt_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, YAMLFMT_FIX_ARGS, files, "")
}

pub fn shfmt_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, SHFMT_CHECK_ARGS, files, "")
}

pub fn shfmt_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, SHFMT_FIX_ARGS, files, "")
}

pub fn standardrb_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, STANDARDRB_CHECK_ARGS, files, "")
}

pub fn standardrb_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, STANDARDRB_FIX_ARGS, files, "")
}

pub fn djlint_format_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, DJLINT_FORMAT_CHECK_ARGS, files, "")
}

pub fn djlint_format_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, DJLINT_FORMAT_FIX_ARGS, files, "")
}

pub fn djlint_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(
        binary,
        DJLINT_CHECK_PREFIX,
        "--configuration",
        config,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

pub fn stylelint_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(
        binary,
        STYLELINT_PREFIX,
        "--config",
        config,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

pub fn rubocop_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, RUBOCOP_ARGS, files, "")
}

pub fn psscriptanalyzer_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, EMPTY_ARGS, files, "")
}

pub fn yamllint_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(
        binary,
        YAMLLINT_PREFIX,
        "-c",
        config,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

pub fn shellcheck_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, SHELLCHECK_ARGS, files, "")
}

pub fn keep_sorted_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, EMPTY_ARGS, files, "")
}

#[path = "commands_tests.rs"]
#[cfg(test)]
mod commands_tests;
