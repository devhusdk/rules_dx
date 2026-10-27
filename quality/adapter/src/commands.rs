use std::ffi::{OsStr, OsString};
use std::path::Path;

use super::launch::{LaunchError, ResolvedTool};

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
const RUSTFMT_MIDDLE: &[&str] = &[
    "--color=never",
    "--config",
    "skip_children=true",
    "--config-path",
];
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
const PRETTIER_NO_EDITORCONFIG: &str = "--no-editorconfig";
const PRETTIER_CHECK_TAIL: &[&str] = &["--check"];
const PRETTIER_FIX_TAIL: &[&str] = &["--write"];
const REMARK_CHECK_TAIL: &[&str] = &["--check"];
const REMARK_FIX_TAIL: &[&str] = &["--write"];
const SCALAFMT_CHECK_PREFIX: &[&str] = &["--list"];
const SCALAFMT_EMPTY: &[&str] = &[];
const CSHARPIER_CHECK_PREFIX: &[&str] = &["check"];
const CSHARPIER_FIX_PREFIX: &[&str] = &["format", "--no-cache"];
const FANTOMAS_CHECK_ARGS: &[&str] = &["check", "--json"];
const FANTOMAS_FIX_ARGS: &[&str] = &[];
const GOFUMPT_CHECK_ARGS: &[&str] = &["-d"];
const GOFUMPT_FIX_ARGS: &[&str] = &["-w"];
const STATICCHECK_ARGS: &[&str] = &["-f", "json"];
const EMPTY_ARGS: &[&str] = &[];
const PSSCRIPTANALYZER_FORMAT_CHECK_ARGS: &[&str] = &["--format-check"];
const PSSCRIPTANALYZER_FORMAT_WRITE_ARGS: &[&str] = &["--format-write"];
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
const BUF_FORMAT_CHECK_ARGS: &[&str] = &["format"];
const BUF_FORMAT_FIX_ARGS: &[&str] = &["format", "--write"];
const BUF_PATH_FLAG: &str = "--path";
const BUF_CONFIG_FLAG: &str = "--config";
const QMLFORMAT_CHECK_ARGS: &[&str] = &[];
const QMLFORMAT_FIX_ARGS: &[&str] = &["-i"];
const QMLLINT_ARGS: &[&str] = &["-W", "0", "--json", "-"];
const QMLLINT_IGNORE_SETTINGS: &str = "--ignore-settings";
const CUE_CHECK_ARGS: &[&str] = &["fmt", "--check", "--diff"];
const CUE_FIX_ARGS: &[&str] = &["fmt"];
const JSONNETFMT_CHECK_ARGS: &[&str] = &["--test"];
const JSONNETFMT_FIX_ARGS: &[&str] = &["-i"];
const PKL_CHECK_ARGS: &[&str] = &["format", "--diff-name-only"];
const PKL_FIX_ARGS: &[&str] = &["format", "-w"];
const ZIG_FMT_CHECK_ARGS: &[&str] = &["fmt", "--check", "--color", "off"];
const ERLFMT_CHECK_ARGS: &[&str] = &["--check"];
const ERLFMT_FIX_ARGS: &[&str] = &["--write"];
const MIX_FORMAT_CHECK_ARGS: &[&str] = &["--check-formatted"];
const GLEAM_FORMAT_CHECK_ARGS: &[&str] = &["--check"];
const ZIG_FMT_FIX_ARGS: &[&str] = &["fmt", "--color", "off"];
const MODFMT_CHECK_ARGS: &[&str] = &["-c", "-l"];
const MODFMT_FIX_ARGS: &[&str] = &["-w"];
const TERRAFORM_CHECK_ARGS: &[&str] = &["fmt", "-check", "-no-color"];
const TERRAFORM_FIX_ARGS: &[&str] = &["fmt", "-no-color"];
const YAMLFMT_CHECK_ARGS: &[&str] = &["-lint", "-q"];
const YAMLFMT_FIX_ARGS: &[&str] = &[];
const SHFMT_CHECK_ARGS: &[&str] = &["-d"];
const SHFMT_FIX_ARGS: &[&str] = &["-w"];
const STANDARDRB_CHECK_ARGS: &[&str] = &["--format", "json", "--cache", "false"];
const STANDARDRB_FIX_ARGS: &[&str] = &["--fix", "--cache", "false"];
const DJLINT_FORMAT_CHECK_ARGS: &[&str] = &["--check"];
const DJLINT_FORMAT_FIX_ARGS: &[&str] = &["--reformat", "--quiet"];
const DJLINT_CHECK_PREFIX: &[&str] = &[
    "--lint",
    "--quiet",
    "--linter-output-format",
    "{filename}:{line}: {code} {message}",
];
const DJLINT_PREFER_CONFIG: &[&str] = &["--prefer-configuration"];
const STYLELINT_PREFIX: &[&str] = &["--formatter", "json"];
const RUBOCOP_ARGS: &[&str] = &["--format", "json", "--cache", "false"];
const YAMLLINT_PREFIX: &[&str] = &["-f", "parsable"];
const SHELLCHECK_ARGS: &[&str] = &["--format=gcc"];
const SHELLCHECK_NORC: &[&str] = &["--norc"];
const HADOLINT_TAIL: &[&str] = &["--no-color", "-f", "json", "--"];
const KEEP_SORTED_CHECK_ARGS: &[&str] = &["--mode", "lint"];
const KEEP_SORTED_FIX_ARGS: &[&str] = &["--mode", "fix"];
const CLANG_FORMAT_CHECK_PREFIX: &[&str] = &["--dry-run", "--Werror"];
const CLANG_FORMAT_FIX_PREFIX: &[&str] = &["-i"];
const CLANG_TIDY_PREFIX: &[&str] = &["--quiet"];
const CPPCHECK_PREFIX: &[&str] = &["--xml", "--xml-version=2"];
const FSHARPLINT_CONFIG_FLAG: &[&str] = &["--config"];
const SCALAFIX_CHECK_FLAGS: &[&str] = &["--check", "--no-stale-semanticdb"];

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
    ];
    head.extend(RUSTFMT_MIDDLE.iter().map(OsString::from));
    head.push(config.as_os_str().to_owned());
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

pub fn eslint_check(
    launch: &ResolvedTool,
    files: &[&Path],
    config: &Path,
) -> Result<Invocation, LaunchError> {
    let mut args = vec![OsString::from("-c"), config.as_os_str().to_owned()];
    args.extend(ESLINT_MIDDLE.iter().map(OsString::from));
    launch.resolve(&args, files)
}

pub fn eslint_fix(
    launch: &ResolvedTool,
    files: &[&Path],
    config: &Path,
) -> Result<Invocation, LaunchError> {
    let mut args = vec![OsString::from("-c"), config.as_os_str().to_owned()];
    args.extend(ESLINT_FIX_SUFFIX.iter().map(OsString::from));
    launch.resolve(&args, files)
}

/// dx-private flag the Prettier shim strips: a bound config replaces dx's default options.
/// The shim owns `--config`, `--no-config` and `--ignore-path`, so dx never sends those.
pub const PRETTIER_DX_CONFIG: &str = "--dx-config=";

fn prettier_args(config: Option<&Path>, tail: &[&str]) -> Vec<OsString> {
    let mut args = Vec::with_capacity(tail.len() + 2);
    if let Some(cfg) = config {
        let mut flag = OsString::from(PRETTIER_DX_CONFIG);
        flag.push(cfg.as_os_str());
        args.push(flag);
    }
    args.push(OsString::from(PRETTIER_NO_EDITORCONFIG));
    args.extend(tail.iter().map(OsString::from));
    args
}

pub fn prettier_check(
    launch: &ResolvedTool,
    files: &[&Path],
    config: Option<&Path>,
) -> Result<Invocation, LaunchError> {
    launch.resolve(&prettier_args(config, PRETTIER_CHECK_TAIL), files)
}

pub fn prettier_fix(
    launch: &ResolvedTool,
    files: &[&Path],
    config: Option<&Path>,
) -> Result<Invocation, LaunchError> {
    launch.resolve(&prettier_args(config, PRETTIER_FIX_TAIL), files)
}

fn remark_args(tail: &[&str], settings: Option<&Path>) -> Vec<OsString> {
    let mut args = Vec::with_capacity(2 + tail.len());
    if let Some(settings) = settings {
        args.push(OsString::from("--settings"));
        args.push(settings.as_os_str().to_owned());
    }
    args.extend(tail.iter().map(OsString::from));
    args
}

pub fn remark_check(
    launch: &ResolvedTool,
    files: &[&Path],
    settings: Option<&Path>,
) -> Result<Invocation, LaunchError> {
    launch.resolve(&remark_args(REMARK_CHECK_TAIL, settings), files)
}

pub fn remark_fix(
    launch: &ResolvedTool,
    files: &[&Path],
    settings: Option<&Path>,
) -> Result<Invocation, LaunchError> {
    launch.resolve(&remark_args(REMARK_FIX_TAIL, settings), files)
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

pub struct ScalafixContext<'a> {
    pub config: &'a Path,
    pub sourceroot: &'a Path,
    pub classpath: &'a [&'a Path],
}

fn scalafix_invocation(
    binary: &Path,
    files: &[&Path],
    context: &ScalafixContext<'_>,
    check: bool,
) -> Result<Invocation, std::env::JoinPathsError> {
    let mut head = vec![binary.as_os_str().to_owned()];
    if check {
        head.extend(SCALAFIX_CHECK_FLAGS.iter().map(OsString::from));
    }
    head.push(OsString::from("--config"));
    head.push(context.config.as_os_str().to_owned());
    head.push(OsString::from("--sourceroot"));
    head.push(context.sourceroot.as_os_str().to_owned());
    head.push(OsString::from("--classpath"));
    head.push(std::env::join_paths(context.classpath)?);
    Ok(assemble(binary, head, files, ""))
}

pub fn scalafix_check(
    binary: &Path,
    files: &[&Path],
    context: &ScalafixContext<'_>,
) -> Result<Invocation, std::env::JoinPathsError> {
    scalafix_invocation(binary, files, context, true)
}

pub fn scalafix_fix(
    binary: &Path,
    files: &[&Path],
    context: &ScalafixContext<'_>,
) -> Result<Invocation, std::env::JoinPathsError> {
    scalafix_invocation(binary, files, context, false)
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

pub struct FsharplintContext<'a> {
    pub compile: &'a [&'a Path],
    pub refs: &'a [&'a Path],
    pub defines: &'a [String],
    pub langversion: &'a str,
}

pub fn fsharplint_check(
    binary: &Path,
    files: &[&Path],
    context: &FsharplintContext<'_>,
    config: Option<&Path>,
) -> Invocation {
    let mut head = vec![binary.as_os_str().to_owned()];
    for source in context.compile {
        head.push(OsString::from("--compile"));
        head.push(source.as_os_str().to_owned());
    }
    for reference in context.refs {
        head.push(OsString::from("--ref"));
        head.push(reference.as_os_str().to_owned());
    }
    for define in context.defines {
        head.push(OsString::from("--define"));
        head.push(OsString::from(define));
    }
    if !context.langversion.is_empty() {
        head.push(OsString::from("--langversion"));
        head.push(OsString::from(context.langversion));
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

pub fn gofumpt_check(binary: &Path, files: &[&Path], lang: &str) -> Invocation {
    gofumpt(binary, GOFUMPT_CHECK_ARGS, files, lang)
}

pub fn gofumpt_fix(binary: &Path, files: &[&Path], lang: &str) -> Invocation {
    gofumpt(binary, GOFUMPT_FIX_ARGS, files, lang)
}

/// Pins `-lang` so the sandboxed check and the in-place fix never fall back to go.mod discovery.
fn gofumpt(binary: &Path, args: &[&str], files: &[&Path], lang: &str) -> Invocation {
    let mut head = vec![
        binary.as_os_str().to_owned(),
        OsString::from(format!("-lang={lang}")),
    ];
    head.extend(args.iter().map(OsString::from));
    assemble(binary, head, files, "")
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

pub fn spotbugs_check(
    binary: &Path,
    files: &[&Path],
    auxiliary: &[&Path],
) -> Result<Invocation, std::env::JoinPathsError> {
    let mut argv = vec![binary.as_os_str().to_owned()];
    argv.extend(SPOTBUGS_ARGS.iter().map(OsString::from));
    if !auxiliary.is_empty() {
        let joined = std::env::join_paths(auxiliary)?;
        argv.push(OsString::from("-auxclasspath"));
        argv.push(joined);
    }
    argv.extend(files.iter().map(|f| f.as_os_str().to_owned()));
    Ok(Invocation {
        argv,
        cwd_rel: String::new(),
    })
}

pub fn ktlint_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(
        binary,
        KTLINT_CHECK_ARGS,
        "--editorconfig",
        config,
        &[],
        files,
        "",
    )
}

pub fn ktlint_fix(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(
        binary,
        KTLINT_FIX_ARGS,
        "--editorconfig",
        config,
        &[],
        files,
        "",
    )
}

fn buf(
    binary: &Path,
    args: &[&str],
    config: Option<&Path>,
    root: &Path,
    files: &[&Path],
) -> Invocation {
    let mut argv = Vec::with_capacity(4 + args.len() + 2 * files.len());
    argv.push(binary.as_os_str().to_owned());
    argv.extend(args.iter().map(OsString::from));
    if let Some(config) = config {
        argv.push(OsString::from(BUF_CONFIG_FLAG));
        argv.push(config.as_os_str().to_owned());
    }
    for file in files {
        argv.push(OsString::from(BUF_PATH_FLAG));
        argv.push(file.as_os_str().to_owned());
    }
    argv.push(root.as_os_str().to_owned());
    Invocation {
        argv,
        cwd_rel: String::new(),
    }
}

pub fn buf_lint_check(
    binary: &Path,
    files: &[&Path],
    root: &Path,
    config: Option<&Path>,
) -> Invocation {
    buf(binary, BUF_LINT_ARGS, config, root, files)
}

pub fn buf_format_check(
    binary: &Path,
    files: &[&Path],
    root: &Path,
    config: Option<&Path>,
) -> Invocation {
    buf(binary, BUF_FORMAT_CHECK_ARGS, config, root, files)
}

pub fn buf_format_fix(binary: &Path, files: &[&Path], root: &Path) -> Invocation {
    buf(binary, BUF_FORMAT_FIX_ARGS, None, root, files)
}

pub fn qmlformat_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(binary, QMLFORMAT_CHECK_ARGS, "-s", config, &[], files, "")
}

pub fn qmlformat_fix(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(binary, QMLFORMAT_FIX_ARGS, "-s", config, &[], files, "")
}

pub fn qmllint_check(
    binary: &Path,
    files: &[&Path],
    import_dirs: &[&Path],
    use_settings: bool,
) -> Invocation {
    let mut head = Vec::with_capacity(2 + QMLLINT_ARGS.len() + 2 * import_dirs.len());
    head.push(binary.as_os_str().to_owned());
    head.extend(QMLLINT_ARGS.iter().map(OsString::from));
    if !use_settings {
        head.push(OsString::from(QMLLINT_IGNORE_SETTINGS));
    }
    for dir in import_dirs {
        head.push(OsString::from("-I"));
        head.push(dir.as_os_str().to_owned());
    }
    assemble(binary, head, files, "")
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

pub fn zig_fmt_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, ZIG_FMT_CHECK_ARGS, files, "")
}

pub fn zig_fmt_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, ZIG_FMT_FIX_ARGS, files, "")
}

pub fn erlfmt_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, ERLFMT_CHECK_ARGS, files, "")
}

pub fn erlfmt_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, ERLFMT_FIX_ARGS, files, "")
}

pub fn gleam_format_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, GLEAM_FORMAT_CHECK_ARGS, files, "")
}

pub fn gleam_format_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, &[], files, "")
}

pub fn mix_format_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, MIX_FORMAT_CHECK_ARGS, files, "")
}

pub fn mix_format_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, &[], files, "")
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

pub fn yamlfmt_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(
        binary,
        YAMLFMT_CHECK_ARGS,
        "-conf",
        config,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

pub fn yamlfmt_fix(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(
        binary,
        YAMLFMT_FIX_ARGS,
        "-conf",
        config,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

pub fn shfmt_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, SHFMT_CHECK_ARGS, files, "")
}

pub fn shfmt_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, SHFMT_FIX_ARGS, files, "")
}

pub fn standardrb_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(
        binary,
        STANDARDRB_CHECK_ARGS,
        "--config",
        config,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

pub fn standardrb_fix(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(
        binary,
        STANDARDRB_FIX_ARGS,
        "--config",
        config,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

pub fn djlint_format_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    djlint(binary, DJLINT_FORMAT_CHECK_ARGS, files, config)
}

pub fn djlint_format_fix(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    djlint(binary, DJLINT_FORMAT_FIX_ARGS, files, config)
}

pub fn djlint_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    djlint(binary, DJLINT_CHECK_PREFIX, files, config)
}

fn djlint(binary: &Path, prefix: &[&str], files: &[&Path], config: Option<&Path>) -> Invocation {
    let suffix = if config.is_some() {
        DJLINT_PREFER_CONFIG
    } else {
        SCALAFMT_EMPTY
    };
    optional_flag(binary, prefix, "--configuration", config, suffix, files, "")
}

pub fn stylelint_check(
    launch: &ResolvedTool,
    files: &[&Path],
    config: &Path,
) -> Result<Invocation, LaunchError> {
    let mut args: Vec<OsString> = STYLELINT_PREFIX.iter().map(OsString::from).collect();
    args.push(OsString::from("--config"));
    args.push(config.as_os_str().to_owned());
    launch.resolve(&args, files)
}

pub fn graphql_eslint_check(
    launch: &ResolvedTool,
    files: &[&Path],
    config: &Path,
    root: &Path,
) -> Result<Invocation, LaunchError> {
    let args = vec![
        OsString::from("--config"),
        config.as_os_str().to_owned(),
        OsString::from("--root"),
        root.as_os_str().to_owned(),
        OsString::from("--"),
    ];
    launch.resolve(&args, files)
}

/// The problem cap passed to Redocly, far above any real report so totals stay comparable.
pub const REDOCLY_MAX_PROBLEMS: &str = "1000000";

/// One `redocly lint` over a single API root, with a JSON report on stdout.
pub fn redocly_check(
    launch: &ResolvedTool,
    root: &Path,
    config: &Path,
) -> Result<Invocation, LaunchError> {
    let args = vec![
        OsString::from("lint"),
        OsString::from("--format=json"),
        OsString::from(format!("--max-problems={REDOCLY_MAX_PROBLEMS}")),
        OsString::from("--lint-config=error"),
        OsString::from("--config"),
        config.as_os_str().to_owned(),
    ];
    launch.resolve(&args, &[root])
}

pub fn rubocop_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(
        binary,
        RUBOCOP_ARGS,
        "--config",
        config,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

pub fn psscriptanalyzer_check(
    binary: &Path,
    files: &[&Path],
    settings: Option<&Path>,
) -> Invocation {
    optional_flag(
        binary,
        EMPTY_ARGS,
        "--settings",
        settings,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

/// `Invoke-Formatter` in report mode: one JSON entry per file, never a write.
pub fn psscriptanalyzer_format_check(
    binary: &Path,
    files: &[&Path],
    settings: Option<&Path>,
) -> Invocation {
    optional_flag(
        binary,
        PSSCRIPTANALYZER_FORMAT_CHECK_ARGS,
        "--settings",
        settings,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

/// `Invoke-Formatter` in write mode: rewrites changed files and exits non-zero on any unparsed file.
pub fn psscriptanalyzer_format_fix(
    binary: &Path,
    files: &[&Path],
    settings: Option<&Path>,
) -> Invocation {
    optional_flag(
        binary,
        PSSCRIPTANALYZER_FORMAT_WRITE_ARGS,
        "--settings",
        settings,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

pub fn yamllint_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    match config {
        Some(_) => optional_flag(
            binary,
            YAMLLINT_PREFIX,
            "-c",
            config,
            SCALAFMT_EMPTY,
            files,
            "",
        ),
        None => optional_flag(
            binary,
            YAMLLINT_PREFIX,
            "-d",
            Some(Path::new("default")),
            SCALAFMT_EMPTY,
            files,
            "",
        ),
    }
}

/// `--rcfile=<config>` when one is declared, else `--norc`; never both, as `--norc` drops an explicit rcfile.
pub fn shellcheck_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    let suffix = if config.is_some() {
        &[][..]
    } else {
        SHELLCHECK_NORC
    };
    optional_joined(
        binary,
        SHELLCHECK_ARGS,
        "--rcfile=",
        config,
        suffix,
        files,
        "",
    )
}

/// Always `--config <config>`: without it hadolint reads the cwd, XDG and HOME configs.
pub fn hadolint_check(binary: &Path, files: &[&Path], config: &Path) -> Invocation {
    optional_flag(
        binary,
        EMPTY_ARGS,
        "--config",
        Some(config),
        HADOLINT_TAIL,
        files,
        "",
    )
}

pub fn keep_sorted_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, KEEP_SORTED_CHECK_ARGS, files, "")
}

pub fn keep_sorted_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, KEEP_SORTED_FIX_ARGS, files, "")
}

/// Maximum argv length one tool spawn may carry, in bytes and UTF-16 code units.
///
/// Windows refuses command lines past 32,767 characters while Linux allows megabytes, so
/// one conservative budget keeps every platform below its native limit.
pub const ARGV_BATCH_BUDGET: usize = 24_000;

/// Batch budget for a `.bat` or `.cmd` launcher, which cmd.exe re-parses under its 8,191 limit.
pub const LAUNCHER_BATCH_BUDGET: usize = 6_000;

/// cmd.exe command-line limit of 8,191 characters, less room for quoting.
pub const LAUNCHER_COMMAND_LINE_LIMIT: usize = 8_000;

/// CreateProcess command-line limit of 32,767 characters, less room for quoting.
pub const WINDOWS_COMMAND_LINE_LIMIT: usize = 32_000;

/// How a tool carries its input files across the process boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ArgTransport {
    Single,
    Batched,
}

/// The transport a tool proved: Batched tools report each file independently of the others.
pub fn transport_for(tool_id: &str) -> ArgTransport {
    match tool_id {
        "keep_sorted" | "ruff" | "flake8" | "pydoclint" | "buildifier" => ArgTransport::Batched,
        _ => ArgTransport::Single,
    }
}

/// Whether a program is a `.bat` or `.cmd` launcher that runs through cmd.exe.
pub fn is_batch_launcher(program: &OsStr) -> bool {
    Path::new(program)
        .extension()
        .and_then(OsStr::to_str)
        .is_some_and(|ext| ext.eq_ignore_ascii_case("bat") || ext.eq_ignore_ascii_case("cmd"))
}

/// The batch budget for spawns of one program.
pub fn argv_budget(program: &OsStr) -> usize {
    if is_batch_launcher(program) {
        LAUNCHER_BATCH_BUDGET
    } else {
        ARGV_BATCH_BUDGET
    }
}

/// The Windows command-line limit for spawns of one program.
pub fn windows_argv_limit(program: &OsStr) -> Option<usize> {
    if is_batch_launcher(program) {
        Some(LAUNCHER_COMMAND_LINE_LIMIT)
    } else {
        Some(WINDOWS_COMMAND_LINE_LIMIT)
    }
}

/// The host's command-line limit for one program: the Windows rule on Windows, none elsewhere.
pub fn native_argv_limit(program: &OsStr) -> Option<usize> {
    if cfg!(windows) {
        windows_argv_limit(program)
    } else {
        None
    }
}

/// The argv cost of one argument in bytes and UTF-16 code units.
///
/// Byte length bounds Unix argv while UTF-16 units bound the Windows command line. Lossy
/// text keeps the estimate on the safe side for names outside Unicode.
pub fn argv_sizes(arg: &OsStr) -> (usize, usize) {
    let bytes = arg.as_encoded_bytes().len();
    let units = arg.to_string_lossy().encode_utf16().count();
    (bytes, units)
}

/// The argv cost of a whole command line in bytes and UTF-16 code units, one separator each.
pub fn argv_cost(argv: &[OsString]) -> (usize, usize) {
    let mut bytes = 0usize;
    let mut units = 0usize;
    for arg in argv {
        let (argument_bytes, argument_units) = argv_sizes(arg.as_os_str());
        bytes += argument_bytes + 1;
        units += argument_units + 1;
    }
    (bytes, units)
}

/// Splits input files into spawns that each fit the program's batch budget.
///
/// Batches keep input order, pass files as separate argv entries with no shell, and put an
/// oversized single file in a batch of its own. An empty selection keeps one empty batch so
/// callers spawn exactly as before.
pub fn plan_batches<'a>(head: &[OsString], files: &[&'a Path]) -> Vec<Vec<&'a Path>> {
    let budget = head
        .first()
        .map_or(ARGV_BATCH_BUDGET, |program| argv_budget(program));
    plan_batches_with_budget(head, files, budget)
}

/// Splits input files against an explicit budget for tests.
pub fn plan_batches_with_budget<'a>(
    head: &[OsString],
    files: &[&'a Path],
    budget: usize,
) -> Vec<Vec<&'a Path>> {
    let (head_bytes, head_units) = argv_cost(head);
    let mut batches: Vec<Vec<&Path>> = Vec::new();
    let mut current: Vec<&Path> = Vec::new();
    let mut current_bytes = head_bytes;
    let mut current_units = head_units;
    for file in files.iter().copied() {
        let (file_bytes, file_units) = argv_sizes(file.as_os_str());
        let fits = current.is_empty()
            || (current_bytes + file_bytes < budget && current_units + file_units < budget);
        if !fits {
            batches.push(std::mem::take(&mut current));
            current_bytes = head_bytes;
            current_units = head_units;
        }
        current.push(file);
        current_bytes += file_bytes + 1;
        current_units += file_units + 1;
    }
    if !current.is_empty() || batches.is_empty() {
        batches.push(current);
    }
    batches
}

/// Splits input files into spawns whose built invocations each fit the program's budget.
///
/// Each file costs what it adds to the built argv, so per-file flags count against the
/// budget. Batches keep input order, an oversized file runs alone, and an empty selection
/// keeps one empty batch.
pub fn plan_invocation_batches<'a, E>(
    files: &[&'a Path],
    build: impl Fn(&[&Path]) -> Result<Invocation, E>,
) -> Result<Vec<Vec<&'a Path>>, E> {
    let head = build(&[])?;
    let budget = head
        .argv
        .first()
        .map_or(ARGV_BATCH_BUDGET, |program| argv_budget(program));
    plan_invocation_batches_with_budget(files, build, budget)
}

/// Splits input files against an explicit budget for tests.
pub fn plan_invocation_batches_with_budget<'a, E>(
    files: &[&'a Path],
    build: impl Fn(&[&Path]) -> Result<Invocation, E>,
    budget: usize,
) -> Result<Vec<Vec<&'a Path>>, E> {
    let (head_bytes, head_units) = argv_cost(&build(&[])?.argv);
    let mut batches: Vec<Vec<&Path>> = Vec::new();
    let mut current: Vec<&Path> = Vec::new();
    let mut current_bytes = head_bytes;
    let mut current_units = head_units;
    for file in files.iter().copied() {
        let (one_bytes, one_units) = argv_cost(&build(&[file])?.argv);
        let file_bytes = one_bytes.saturating_sub(head_bytes);
        let file_units = one_units.saturating_sub(head_units);
        let fits = current.is_empty()
            || (current_bytes + file_bytes <= budget && current_units + file_units <= budget);
        if !fits {
            batches.push(std::mem::take(&mut current));
            current_bytes = head_bytes;
            current_units = head_units;
        }
        current.push(file);
        current_bytes += file_bytes;
        current_units += file_units;
    }
    if !current.is_empty() || batches.is_empty() {
        batches.push(current);
    }
    Ok(batches)
}

#[path = "commands_tests.rs"]
#[cfg(test)]
mod commands_tests;
