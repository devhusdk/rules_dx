use std::ffi::{OsStr, OsString};
use std::path::{Path, PathBuf};

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
/// Config discovery, the git ignore rules and the default excludes are all off, and
/// `--force-exclude` stays off: with it an explicit file under a directory such as `build/` is
/// skipped and reported clean.
const FORTITUDE_CHECK_ARGS: &[&str] = &[
    "check",
    "--isolated",
    "--no-respect-gitignore",
    "--progress-bar",
    "off",
    "--output-format",
    "json",
];
// ffmt has no `--config` flag: it reads `ffmt.toml`, `.ffmt.toml` or `pyproject.toml` from the
// working directory and its parents, so the runner pins the working directory to a staged config.
// `--no-cache` keeps it from writing a cache, `--no-ignore` stops `.gitignore` files from hiding
// an explicitly named file, and `--color never` keeps ANSI codes out of the path list.
const FFMT_CHECK_ARGS: &[&str] = &["--check", "--no-cache", "--no-ignore", "--color", "never"];
const FFMT_FIX_ARGS: &[&str] = &["--no-cache", "--no-ignore", "--color", "never"];
/// A well-formedness check only: no output tree, no network and no catalogs, so a document type
/// or an entity can never reach outside the staged files. `--noent`, `--loaddtd` and `--valid`
/// stay off, which keeps external entities and DTDs unread.
const XMLLINT_CHECK_ARGS: &[&str] = &["--noout", "--nonet", "--nocatalogs"];
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
const DIALYZER_ARGS: &[&str] = &[
    "--no_check_plt",
    "--no_indentation",
    "--error_location",
    "column",
    "--fullpath",
];

const PHPCS_ARGS: &[&str] = &["--report=json", "-q", "--no-colors", "--no-cache"];
const LUALS_ARGS: &[&str] = &["--checklevel=Warning", "--check_format=json"];
/// `--no-config` keeps `sorbet/config` of the source tree out; no `--cache-dir` is given, so
/// nothing is written outside the work directory.
const SORBET_ARGS: &[&str] = &[
    "--no-config",
    "--color=never",
    "--no-error-sections",
    "--silence-dev-message",
];
const PHPSTAN_ARGS: &[&str] = &[
    "analyse",
    "--no-progress",
    "--no-interaction",
    "--error-format=json",
];
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
const CUE_VET_ARGS: &[&str] = &["vet", "-c=false", "-E"];
const CUE_FIX_ARGS: &[&str] = &["fmt"];
const JSONNETFMT_CHECK_ARGS: &[&str] = &["--test"];
const JSONNETFMT_FIX_ARGS: &[&str] = &["-i"];
const TYPSTYLE_CHECK_ARGS: &[&str] = &["--check"];
const TYPSTYLE_FIX_ARGS: &[&str] = &["-i"];
const LATEXINDENT_CHECK_ARGS: &[&str] = &["-k"];
const LATEXINDENT_FIX_ARGS: &[&str] = &["-w"];
const STYLUA_CHECK_ARGS: &[&str] = &["--no-editorconfig", "--check", "--output-format=json"];
const STYLUA_FIX_ARGS: &[&str] = &["--no-editorconfig"];
const STYLUA_CONFIG_FLAG: &str = "--config-path";
const NIXFMT_CHECK_ARGS: &[&str] = &["--check"];
const ORMOLU_CHECK_ARGS: &[&str] = &[
    "--mode",
    "check",
    "--no-cabal",
    "--no-dot-ormolu",
    "--color",
    "never",
];
const ORMOLU_FIX_ARGS: &[&str] = &[
    "--mode",
    "inplace",
    "--no-cabal",
    "--no-dot-ormolu",
    "--color",
    "never",
];
const PKL_CHECK_ARGS: &[&str] = &["format", "--diff-name-only"];
/// Offline, cache-free evaluation: only `pkl:` and `file:` modules load, so `package:` and
/// `https:` imports fail instead of reaching the network, and `--no-project` ignores any PklProject.
const PKL_EVAL_ARGS: &[&str] = &[
    "eval",
    "--no-cache",
    "--no-project",
    "--allowed-modules",
    "pkl:,file:",
    "--allowed-resources",
    "prop:,env:,file:",
];
/// Load-only evaluation of abstract modules: `-x 1` links imports and checks type names without
/// forcing values, which an abstract module leaves unset by design.
const PKL_LOAD_ONLY_ARGS: &[&str] = &[
    "eval",
    "--no-cache",
    "--no-project",
    "--allowed-modules",
    "pkl:,file:,repl:",
    "--allowed-resources",
    "prop:,env:,file:",
];
const PKL_FIX_ARGS: &[&str] = &["format", "-w"];
const BICEP_FORMAT_CHECK_ARGS: &[&str] = &["format", "--newline-kind", "LF", "--stdout"];
const BICEP_FORMAT_FIX_ARGS: &[&str] = &["format", "--newline-kind", "LF"];
const BICEP_LINT_ARGS: &[&str] = &["lint", "--no-restore", "--diagnostics-format", "sarif"];
const OPA_FMT_CHECK_ARGS: &[&str] = &["fmt"];
const OPA_FMT_FIX_ARGS: &[&str] = &["fmt", "-w"];
const OPA_CHECK_ARGS: &[&str] = &["check", "--format", "json", "--max-errors=-1"];
const ZIG_AST_CHECK_ARGS: &[&str] = &["ast-check", "--color", "off"];
const ZIG_FMT_CHECK_ARGS: &[&str] = &["fmt", "--check", "--color", "off"];
/// The scratch directory holding the `.editorconfig` dfmt is pointed at; dfmt otherwise searches
/// upward from each source file.
pub const DFMT_CONFIG_DIR: &str = "dx-dfmt-default";
const DFMT_CHECK_ARGS: &[&str] = &["--config", DFMT_CONFIG_DIR];
const DFMT_FIX_ARGS: &[&str] = &["--config", DFMT_CONFIG_DIR, "--inplace"];
/// The scratch path of the `dscanner.ini` dscanner is pointed at; with `--config` it never reads
/// `$HOME/.config/dscanner` or a `dscanner.ini` in a parent directory.
pub const DSCANNER_CONFIG_REL: &str = "dx-dscanner-default/dscanner.ini";
/// The sentinel, then tab-separated fields; the message is last because upstream substitutes it
/// before `{type}` and `{name}`.
pub const DSCANNER_FORMAT: &str =
    "dx\t{filepath}\t{line}\t{column}\t{endLine}\t{endColumn}\t{type}\t{name}\t{message}";
const DSCANNER_CHECK_ARGS: &[&str] = &[
    "--styleCheck",
    "--config",
    DSCANNER_CONFIG_REL,
    "--errorFormat",
    DSCANNER_FORMAT,
];
const DART_FORMAT_CHECK_ARGS: &[&str] = &[
    "--suppress-analytics",
    "format",
    "--output=none",
    "--set-exit-if-changed",
];
const DART_FORMAT_FIX_ARGS: &[&str] = &["--suppress-analytics", "format", "--output=write"];
const JULIA_FORMAT_CHECK_ARGS: &[&str] = &["--check"];
const JULIA_FORMAT_FIX_ARGS: &[&str] = &["--apply"];
/// Always first: it stops perltidy reading `~/.perltidyrc`, `./.perltidyrc` and `$PERLTIDY`.
const PERLTIDY_CHECK_ARGS: &[&str] = &[
    "--noprofile",
    "--standard-output",
    "--standard-error-output",
    "--output-line-ending=unix",
];
const PERLTIDY_FIX_ARGS: &[&str] = &[
    "--noprofile",
    "-b",
    "-bext=/",
    "--standard-error-output",
    "--output-line-ending=unix",
];
/// The sentinel, then tab-separated fields; the message is last. `%g` and `%L` are the physical
/// file and line, so a `#line` directive cannot redirect a finding; `%c` is the visual column.
pub const PERLCRITIC_FORMAT: &str = "dx\t%g\t%L\t%c\t%s\t%p\t%m\n";
/// `--noprofile` is always first: it stops perlcritic reading `~/.perlcriticrc`, `./.perlcriticrc`
/// and `$PERLCRITIC`. Severity 5 is the only level dx asks for.
const PERLCRITIC_CHECK_ARGS: &[&str] = &[
    "--noprofile",
    "--severity=5",
    "--nocolor",
    "--quiet",
    "--verbose",
    PERLCRITIC_FORMAT,
];
const ERLFMT_CHECK_ARGS: &[&str] = &["--check"];
const ERLFMT_FIX_ARGS: &[&str] = &["--write"];
const MIX_FORMAT_CHECK_ARGS: &[&str] = &["--check-formatted"];
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
/// The `-i` value names no real path, so gplint never reads a `.gplintignore` from its cwd.
const GPLINT_PREFIX: &[&str] = &["-f", "json", "-i", "dx-gplint-no-ignore"];
const STYLELINT_PREFIX: &[&str] = &["--formatter", "json"];
const RUBOCOP_ARGS: &[&str] = &["--format", "json", "--cache", "false"];
const YAMLLINT_PREFIX: &[&str] = &["-f", "parsable"];
const SHELLCHECK_ARGS: &[&str] = &["--format=gcc"];
const SHELLCHECK_NORC: &[&str] = &["--norc"];
const CHECKMAKE_PREFIX: &[&str] = &["--debug", "--output", "json"];
const CHECKMAKE_TAIL: &[&str] = &["--"];
const CMAKELINT_TAIL: &[&str] = &["--"];
// `--no-cache`: gersemi otherwise keeps a cache under the home directory.
const GERSEMI_CHECK_PREFIX: &[&str] = &["--check", "--diff", "--no-cache", "--no-color", "-w", "1"];
const GERSEMI_FIX_PREFIX: &[&str] = &["-i", "--no-cache", "--no-color", "-w", "1"];
const GERSEMI_TAIL: &[&str] = &["--"];
// No `--diff` (it exits 0 for files that would change), no `--validate` (it runs the host
// `make`), and never `update`.
const MBAKE_CHECK_PREFIX: &[&str] = &["format", "--check"];
const MBAKE_FIX_PREFIX: &[&str] = &["format"];
const REGAL_LINT_PREFIX: &[&str] = &[
    "lint",
    "--format",
    "json",
    "--no-color",
    "--fail-level",
    "warning",
];
const HADOLINT_TAIL: &[&str] = &["--no-color", "-f", "json", "--"];
const ELVIS_TAIL: &[&str] = &["--output-format", "parsable", "rock"];
const STATIX_CHECK_PREFIX: &[&str] = &["check", "-o", "json"];
const STATIX_FIX_PREFIX: &[&str] = &["fix"];
const END_OF_FLAGS: &[&str] = &["--"];
const DEADNIX_CHECK_ARGS: &[&str] = &["--fail", "-o", "json", "--"];
const DEADNIX_FIX_ARGS: &[&str] = &["--edit", "--"];
const KEEP_SORTED_CHECK_ARGS: &[&str] = &["--mode", "lint"];
const KEEP_SORTED_FIX_ARGS: &[&str] = &["--mode", "fix"];
const NOTEBOOK_OUTPUTS_CHECK_ARGS: &[&str] = &["outputs"];
const NOTEBOOK_OUTPUTS_FIX_ARGS: &[&str] = &["outputs", "--fix"];
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

/// Inline `src` override so sibling modules next to the checked files classify as
/// first party even though `--isolated` disables config discovery.
fn ruff_src_override(files: &[&Path]) -> Option<OsString> {
    let mut dirs: Vec<String> = files
        .iter()
        .filter_map(|file| file.parent())
        .map(|dir| dir.to_string_lossy().into_owned())
        .filter(|dir| !dir.is_empty())
        .collect();
    dirs.sort();
    dirs.dedup();
    if dirs.is_empty() {
        return None;
    }
    let quoted: Vec<String> = dirs
        .iter()
        .map(|dir| format!("\"{}\"", dir.replace('\\', "\\\\").replace('"', "\\\"")))
        .collect();
    Some(OsString::from(format!("src = [{}]", quoted.join(", "))))
}

fn ruff_base(
    binary: &Path,
    subcommand: &str,
    config: Option<&Path>,
    files: &[&Path],
) -> Vec<OsString> {
    let mut argv = vec![binary.as_os_str().to_owned(), OsString::from(subcommand)];
    if let Some(path) = config {
        argv.push(OsString::from("--config"));
        argv.push(path.as_os_str().to_owned());
    } else {
        argv.push(OsString::from("--isolated"));
        if let Some(src) = ruff_src_override(files) {
            argv.push(OsString::from("--config"));
            argv.push(src);
        }
    }
    argv.extend(RUFF_HERMETIC_TAIL.iter().map(OsString::from));
    argv
}

pub fn ruff_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    let mut head = ruff_base(binary, RUFF_CHECK_SUBCOMMAND[0], config, files);
    head.extend(RUFF_CHECK_TAIL.iter().map(OsString::from));
    assemble(binary, head, files, "")
}

pub fn ruff_fix(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    let mut head = ruff_base(binary, RUFF_CHECK_SUBCOMMAND[0], config, files);
    head.extend(RUFF_FIX_TAIL.iter().map(OsString::from));
    assemble(binary, head, files, "")
}

pub fn ruff_format_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    let mut head = ruff_base(binary, RUFF_FORMAT_SUBCOMMAND[0], config, files);
    head.extend(RUFF_FORMAT_CHECK_TAIL.iter().map(OsString::from));
    assemble(binary, head, files, "")
}

pub fn ruff_format_fix(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    let head = ruff_base(binary, RUFF_FORMAT_SUBCOMMAND[0], config, files);
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

/// dx-private flag the Prettier shim strips and turns into `--language` for the SQL plugin.
pub const PRETTIER_DX_SQL_DIALECT: &str = "--dx-sql-dialect=";

fn prettier_args(config: Option<&Path>, dialect: Option<&str>, tail: &[&str]) -> Vec<OsString> {
    let mut args = Vec::with_capacity(tail.len() + 3);
    if let Some(cfg) = config {
        let mut flag = OsString::from(PRETTIER_DX_CONFIG);
        flag.push(cfg.as_os_str());
        args.push(flag);
    }
    if let Some(dialect) = dialect {
        args.push(OsString::from(format!(
            "{PRETTIER_DX_SQL_DIALECT}{dialect}"
        )));
    }
    args.push(OsString::from(PRETTIER_NO_EDITORCONFIG));
    args.extend(tail.iter().map(OsString::from));
    args
}

pub fn prettier_check(
    launch: &ResolvedTool,
    files: &[&Path],
    config: Option<&Path>,
    dialect: Option<&str>,
) -> Result<Invocation, LaunchError> {
    launch.resolve(&prettier_args(config, dialect, PRETTIER_CHECK_TAIL), files)
}

pub fn prettier_fix(
    launch: &ResolvedTool,
    files: &[&Path],
    config: Option<&Path>,
    dialect: Option<&str>,
) -> Result<Invocation, LaunchError> {
    launch.resolve(&prettier_args(config, dialect, PRETTIER_FIX_TAIL), files)
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

/// `dialyzer` over compiled beams against pre-built PLTs. The PLT is trusted as is
/// (`--no_check_plt`), and every beam, owned or auxiliary, is analysed.
pub fn dialyzer_check(
    binary: &Path,
    code_dirs: &[&Path],
    plts: &[&Path],
    beams: &[&Path],
) -> Invocation {
    let mut argv = vec![binary.as_os_str().to_owned()];
    for dir in code_dirs {
        argv.push(OsString::from("-pa"));
        argv.push(dir.as_os_str().to_owned());
    }
    if !plts.is_empty() {
        argv.push(OsString::from("--plts"));
        argv.extend(plts.iter().map(|p| p.as_os_str().to_owned()));
    }
    argv.extend(DIALYZER_ARGS.iter().map(OsString::from));
    argv.push(OsString::from("--"));
    argv.extend(beams.iter().map(|b| b.as_os_str().to_owned()));
    Invocation {
        argv,
        cwd_rel: String::new(),
    }
}

/// `phpcs` over the owned sources, JSON on stdout. `standard` is a ruleset file or a bundled
/// standard name; it is always passed, so phpcs never searches the working directory for a
/// `phpcs.xml`.
pub fn phpcs_check(binary: &Path, standard: &OsStr, paths: &[&Path]) -> Invocation {
    let mut argv = vec![binary.as_os_str().to_owned()];
    argv.extend(PHPCS_ARGS.iter().map(OsString::from));
    let mut flag = OsString::from("--standard=");
    flag.push(standard);
    argv.push(flag);
    argv.push(OsString::from("--"));
    argv.extend(paths.iter().map(|p| p.as_os_str().to_owned()));
    Invocation {
        argv,
        cwd_rel: String::new(),
    }
}

/// `lua-language-server --check` over a workspace root; the JSON report goes to `report`.
/// The log and the generated meta files go below `work`, never into the install tree.
pub fn luals_check(
    binary: &Path,
    root: &Path,
    work: &Path,
    report: &Path,
    luarc: &Path,
) -> Invocation {
    let flag = |name: &str, path: &Path| {
        let mut arg = OsString::from(name);
        arg.push(path.as_os_str());
        arg
    };
    let mut argv = vec![binary.as_os_str().to_owned(), flag("--check=", root)];
    argv.extend(LUALS_ARGS.iter().map(OsString::from));
    argv.push(flag("--check_out_path=", report));
    argv.push(flag("--logpath=", &work.join("log")));
    argv.push(flag("--metapath=", &work.join("meta")));
    argv.push(flag("--configpath=", luarc));
    Invocation {
        argv,
        cwd_rel: String::new(),
    }
}

/// The generated LuaLS config. The consumer's settings (a flat `.luarc.json` object) come
/// first; the keys below always win: the Lua version, the library directory, no diagnostics
/// in libraries and the ignored staging directories. Without a consumer config the
/// diagnostics are LuaLS's defaults.
pub fn luals_luarc(
    consumer: Option<&serde_json::Value>,
    library: Option<&Path>,
    ignore_dirs: &[&str],
) -> String {
    let mut object = match consumer {
        Some(serde_json::Value::Object(map)) => map.clone(),
        _ => serde_json::Map::new(),
    };
    object.remove("$schema");
    object.insert("runtime.version".to_owned(), "Lua 5.4".into());
    let libraries: Vec<serde_json::Value> = library
        .map(|dir| vec![dir.to_string_lossy().into_owned().into()])
        .unwrap_or_default();
    object.insert("workspace.library".to_owned(), libraries.into());
    object.insert("workspace.checkThirdParty".to_owned(), false.into());
    object.insert("diagnostics.libraryFiles".to_owned(), "Disable".into());
    object.insert("workspace.ignoreDir".to_owned(), ignore_dirs.into());
    object.insert(
        "runtime.path".to_owned(),
        vec!["?.lua", "?/init.lua"].into(),
    );
    serde_json::Value::Object(object).to_string()
}

/// `sorbet` over the owned sources and the staged dependency sources; the errors go to stderr.
pub fn sorbet_check(binary: &Path, paths: &[&Path]) -> Invocation {
    let mut argv = vec![binary.as_os_str().to_owned()];
    argv.extend(SORBET_ARGS.iter().map(OsString::from));
    argv.push(OsString::from("--"));
    argv.extend(paths.iter().map(|p| p.as_os_str().to_owned()));
    Invocation {
        argv,
        cwd_rel: String::new(),
    }
}

/// `phpstan analyse` over the owned sources with a generated config, JSON on stdout.
pub fn phpstan_check(binary: &Path, neon: &Path, paths: &[&Path]) -> Invocation {
    let mut argv = vec![binary.as_os_str().to_owned()];
    argv.extend(PHPSTAN_ARGS.iter().map(OsString::from));
    argv.push(OsString::from("-c"));
    argv.push(neon.as_os_str().to_owned());
    argv.push(OsString::from("--"));
    argv.extend(paths.iter().map(|p| p.as_os_str().to_owned()));
    Invocation {
        argv,
        cwd_rel: String::new(),
    }
}

/// The generated PHPStan config: the consumer's, when there is one, is included first so the
/// values below (the cache directory, the scan directories, one process) always win.
/// Without a consumer config the analysis runs at `level`.
pub fn phpstan_neon(
    consumer: Option<&Path>,
    level: u8,
    tmp_dir: &Path,
    scan_dirs: &[&Path],
) -> String {
    let quote = |path: &Path| {
        let text = path
            .to_string_lossy()
            .replace('\\', "\\\\")
            .replace('"', "\\\"");
        format!("\"{text}\"")
    };
    let mut out = String::new();
    if let Some(consumer) = consumer {
        out.push_str("includes:\n    - ");
        out.push_str(&quote(consumer));
        out.push('\n');
    }
    out.push_str("parameters:\n");
    if consumer.is_none() {
        out.push_str(&format!("    level: {level}\n"));
    }
    out.push_str("    tmpDir: ");
    out.push_str(&quote(tmp_dir));
    out.push('\n');
    if !scan_dirs.is_empty() {
        out.push_str("    scanDirectories:\n");
        for dir in scan_dirs {
            out.push_str("        - ");
            out.push_str(&quote(dir));
            out.push('\n');
        }
    }
    out.push_str("    parallel:\n        maximumNumberOfProcesses: 1\n");
    out
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

/// Validates one schema with `capnp compile -o-`, which parses it and its
/// imports and writes a CodeGeneratorRequest to stdout only on success.
/// `rel_file` is relative to `root` and the run starts there, so the standard
/// import path is off and `root` is the only import root (`..` and absolute
/// imports cannot leave it).
pub fn capnp_check(binary: &Path, rel_file: &Path, root: &Path) -> Invocation {
    Invocation {
        argv: vec![
            binary.as_os_str().to_owned(),
            OsString::from("compile"),
            OsString::from("--no-standard-import"),
            OsString::from("-I"),
            root.as_os_str().to_owned(),
            OsString::from("--src-prefix"),
            root.as_os_str().to_owned(),
            OsString::from("-o-"),
            rel_file.as_os_str().to_owned(),
        ],
        cwd_rel: String::new(),
    }
}

/// Validates one schema with `flatc -M --cpp`, which parses it, prints its
/// dependency list and writes nothing. Includes resolve next to the schema
/// first, then under `root`.
pub fn flatc_check(binary: &Path, file: &Path, root: &Path, out: &Path) -> Invocation {
    Invocation {
        argv: vec![
            binary.as_os_str().to_owned(),
            OsString::from("-M"),
            OsString::from("--cpp"),
            OsString::from("-o"),
            out.as_os_str().to_owned(),
            OsString::from("-I"),
            root.as_os_str().to_owned(),
            file.as_os_str().to_owned(),
        ],
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

/// One vet of the files of a single package. Imports resolve from the module at `cwd_rel`.
/// `-c=false` keeps schema-only (non-concrete) packages from failing as incomplete.
pub fn cue_vet(binary: &Path, files: &[&Path], cwd_rel: &str) -> Invocation {
    fixed(binary, CUE_VET_ARGS, files, cwd_rel)
}

pub fn cue_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, CUE_FIX_ARGS, files, "")
}

/// One batched lint. It prints nothing when every file is clean.
pub fn jsonnet_lint_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, &[], files, "")
}

/// One batched lint over explicit free-form files. It prints `[]` when every file is clean.
pub fn fortitude_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, FORTITUDE_CHECK_ARGS, files, "")
}

/// `ffmt --check` prints one unformatted path per line and exits 1; `config_dir_rel` is the
/// directory that holds the staged `ffmt.toml`.
pub fn ffmt_check(binary: &Path, files: &[&Path], config_dir_rel: &str) -> Invocation {
    fixed(binary, FFMT_CHECK_ARGS, files, config_dir_rel)
}

pub fn ffmt_fix(binary: &Path, files: &[&Path], config_dir_rel: &str) -> Invocation {
    fixed(binary, FFMT_FIX_ARGS, files, config_dir_rel)
}

/// One batched well-formedness check. It prints nothing when every file is clean.
pub fn xmllint_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, XMLLINT_CHECK_ARGS, files, "")
}

pub fn jsonnetfmt_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, JSONNETFMT_CHECK_ARGS, files, "")
}

pub fn jsonnetfmt_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, JSONNETFMT_FIX_ARGS, files, "")
}

/// `typstyle --check`; no `-q`, which would hide the `Would reformat` lines.
pub fn typstyle_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, TYPSTYLE_CHECK_ARGS, files, "")
}

pub fn typstyle_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, TYPSTYLE_FIX_ARGS, files, "")
}

/// `latexindent -k` on one file. The log and the backups go to `cruft` (an absolute directory in
/// the scratch tree) so nothing lands next to the source. The config is passed as `-l=<file>`
/// only when bound: a bare `-l` would auto-load a `localSettings.yaml` from the source directory.
pub fn latexindent_check(
    binary: &Path,
    file: &Path,
    cruft: &Path,
    config: Option<&Path>,
) -> Invocation {
    latexindent_invocation(binary, LATEXINDENT_CHECK_ARGS, file, cruft, config)
}

/// `latexindent -w` on one file; the backup `.bak` goes to `cruft`.
pub fn latexindent_fix(
    binary: &Path,
    file: &Path,
    cruft: &Path,
    config: Option<&Path>,
) -> Invocation {
    latexindent_invocation(binary, LATEXINDENT_FIX_ARGS, file, cruft, config)
}

fn latexindent_invocation(
    binary: &Path,
    mode: &[&str],
    file: &Path,
    cruft: &Path,
    config: Option<&Path>,
) -> Invocation {
    let mut argv: Vec<OsString> = vec![binary.as_os_str().to_owned()];
    argv.extend(mode.iter().map(OsString::from));
    argv.push(flag_value("-g=", &cruft.join("indent.log")));
    let mut dir = cruft.as_os_str().to_owned();
    dir.push("/");
    let mut cruft_flag = OsString::from("-c=");
    cruft_flag.push(dir);
    argv.push(cruft_flag);
    if let Some(path) = config {
        argv.push(flag_value("-l=", path));
    }
    argv.push(file.as_os_str().to_owned());
    Invocation {
        argv,
        cwd_rel: String::new(),
    }
}

fn flag_value(flag: &str, path: &Path) -> OsString {
    let mut value = OsString::from(flag);
    value.push(path.as_os_str());
    value
}

/// `zig ast-check` takes exactly one file per process.
pub fn zig_ast_check(binary: &Path, file: &Path) -> Invocation {
    fixed(binary, ZIG_AST_CHECK_ARGS, &[file], "")
}

pub fn zig_fmt_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, ZIG_FMT_CHECK_ARGS, files, "")
}

pub fn zig_fmt_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, ZIG_FMT_FIX_ARGS, files, "")
}

/// dfmt has no check mode: one file without `--inplace` writes the formatted text to stdout.
/// Pass exactly one file, since two or more switch dfmt to in-place editing. The relative
/// `--config` makes the cwd the scratch root.
pub fn dfmt_check(binary: &Path, file: &Path) -> Invocation {
    fixed(binary, DFMT_CHECK_ARGS, &[file], "")
}

pub fn dfmt_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, DFMT_FIX_ARGS, files, "")
}

/// One batched style check. The relative `--config` makes the cwd the scratch root.
pub fn dscanner_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, DSCANNER_CHECK_ARGS, files, "")
}

/// `dart format` reads each file's language version from the `.dart_tool/package_config.json`
/// staged at the scratch root, and `formatter:` options from the `analysis_options.yaml` beside it.
pub fn dart_format_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, DART_FORMAT_CHECK_ARGS, files, "")
}

/// One analyzer run over the options file and the sources, all absolute. The options file is
/// listed so a broken `include:` or unknown lint surfaces, and `--cache` keeps the analyzer off
/// the (unset) HOME.
pub fn dart_analyze(
    binary: &Path,
    cache_dir: &Path,
    options: &Path,
    files: &[&Path],
) -> Invocation {
    let mut argv = vec![
        binary.as_os_str().to_owned(),
        OsString::from("--suppress-analytics"),
        OsString::from("analyze"),
        OsString::from("--format=json"),
        OsString::from(format!("--cache={}", cache_dir.display())),
        options.as_os_str().to_owned(),
    ];
    argv.extend(files.iter().map(|file| file.as_os_str().to_owned()));
    Invocation {
        argv,
        cwd_rel: String::new(),
    }
}

pub fn dart_format_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, DART_FORMAT_FIX_ARGS, files, "")
}

pub fn julia_format_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, JULIA_FORMAT_CHECK_ARGS, files, "")
}

pub fn julia_format_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, JULIA_FORMAT_FIX_ARGS, files, "")
}

/// perltidy `--standard-output` takes one file and writes the formatted text to stdout.
pub fn perltidy_check(binary: &Path, file: &Path) -> Invocation {
    fixed(binary, PERLTIDY_CHECK_ARGS, &[file], "")
}

/// `-b -bext=/` rewrites in place without a backup; `-se` keeps `.ERR` files from appearing.
pub fn perltidy_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, PERLTIDY_FIX_ARGS, files, "")
}

/// One batched check; Perl::Critic has no autofix.
pub fn perlcritic_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, PERLCRITIC_CHECK_ARGS, files, "")
}

pub fn erlfmt_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, ERLFMT_CHECK_ARGS, files, "")
}

pub fn erlfmt_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, ERLFMT_FIX_ARGS, files, "")
}

/// Always `--config-file <config>`: Credo otherwise reads `.credo.exs` from every parent
/// directory. The dx driver adds `--format json` and picks the exit code.
pub fn credo_check(binary: &Path, files: &[&Path], config: &Path) -> Invocation {
    let mut argv = vec![
        binary.as_os_str().to_owned(),
        OsString::from("--config-file"),
        config.as_os_str().to_owned(),
    ];
    argv.extend(files.iter().map(|file| file.as_os_str().to_owned()));
    Invocation {
        argv,
        cwd_rel: String::new(),
    }
}

/// Always `--config <config>`: elvis checks only the files its config globs match, from the cwd.
pub fn elvis_check(binary: &Path, files: &[&Path], config: &Path) -> Invocation {
    optional_flag(
        binary,
        EMPTY_ARGS,
        "--config",
        Some(config),
        ELVIS_TAIL,
        files,
        "",
    )
}

pub fn mix_format_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, MIX_FORMAT_CHECK_ARGS, files, "")
}

pub fn mix_format_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, &[], files, "")
}

/// Explicit owned `.nix` paths only: the default RFC style at width 100, no `--follow-symlinks`
/// (which would write through a link into the source tree) and no style flags.
pub fn nixfmt_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, NIXFMT_CHECK_ARGS, files, "")
}

pub fn nixfmt_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, &[], files, "")
}

pub fn ormolu_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, ORMOLU_CHECK_ARGS, files, "")
}

pub fn ormolu_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, ORMOLU_FIX_ARGS, files, "")
}

pub fn pkl_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, PKL_CHECK_ARGS, files, "")
}

/// One evaluation of a single module with file access limited to `root`. Only stderr carries
/// findings. `load_only` evaluates the constant `1` instead of rendering the module.
pub fn pkl_eval(binary: &Path, module: &Path, root: &Path, load_only: bool) -> Invocation {
    let mut head = vec![binary.as_os_str().to_owned()];
    let args = if load_only {
        PKL_LOAD_ONLY_ARGS
    } else {
        PKL_EVAL_ARGS
    };
    head.extend(args.iter().map(OsString::from));
    head.push(OsString::from("--root-dir"));
    head.push(root.as_os_str().to_owned());
    if load_only {
        head.push(OsString::from("-x"));
        head.push(OsString::from("1"));
    }
    assemble(binary, head, &[module], "")
}

pub fn pkl_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, PKL_FIX_ARGS, files, "")
}

pub fn opa_fmt_check(binary: &Path, file: &Path) -> Invocation {
    fixed(binary, OPA_FMT_CHECK_ARGS, &[file], "")
}

pub fn opa_fmt_fix(binary: &Path, file: &Path) -> Invocation {
    fixed(binary, OPA_FMT_FIX_ARGS, &[file], "")
}

pub fn opa_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, OPA_CHECK_ARGS, files, "")
}

pub fn stylua_check(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(
        binary,
        STYLUA_CHECK_ARGS,
        STYLUA_CONFIG_FLAG,
        config,
        SCALAFMT_EMPTY,
        files,
        "",
    )
}

/// Always `--config <config>`: gersemi otherwise searches upward from each file for a `.gersemirc`.
pub fn gersemi_check(binary: &Path, files: &[&Path], config: &Path) -> Invocation {
    optional_flag(
        binary,
        GERSEMI_CHECK_PREFIX,
        "--config",
        Some(config),
        GERSEMI_TAIL,
        files,
        "",
    )
}

pub fn gersemi_fix(binary: &Path, files: &[&Path], config: &Path) -> Invocation {
    optional_flag(
        binary,
        GERSEMI_FIX_PREFIX,
        "--config",
        Some(config),
        GERSEMI_TAIL,
        files,
        "",
    )
}

/// Always `--config <config>`: mbake otherwise reads `~/.bake.toml` or the working directory's
/// `.bake.toml`.
pub fn mbake_check(binary: &Path, files: &[&Path], config: &Path) -> Invocation {
    optional_flag(
        binary,
        MBAKE_CHECK_PREFIX,
        "--config",
        Some(config),
        &[],
        files,
        "",
    )
}

pub fn mbake_fix(binary: &Path, files: &[&Path], config: &Path) -> Invocation {
    optional_flag(
        binary,
        MBAKE_FIX_PREFIX,
        "--config",
        Some(config),
        &[],
        files,
        "",
    )
}

pub fn stylua_fix(binary: &Path, files: &[&Path], config: Option<&Path>) -> Invocation {
    optional_flag(
        binary,
        STYLUA_FIX_ARGS,
        STYLUA_CONFIG_FLAG,
        config,
        SCALAFMT_EMPTY,
        files,
        "",
    )
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

/// One tflint spawn for one module directory, run from the mirror root.
///
/// `--chdir` makes tflint report workspace-relative paths, `--filter` (relative to
/// `--chdir`) narrows the report to the owned files, and `--config` is always passed so
/// `./.tflint.hcl` and `~/.tflint.hcl` never load. `--call-module-type=none` stops it
/// reading local module sources outside the mirror. Exit codes (v0.65.0 `cmd/cli.go`):
/// 0 clean, 1 failure (JSON `errors`), 2 issues found.
pub fn tflint_check(binary: &Path, dir_rel: &str, files: &[&Path], config: &Path) -> Invocation {
    let mut argv = vec![
        binary.as_os_str().to_owned(),
        OsString::from("--config"),
        config.as_os_str().to_owned(),
        OsString::from(format!("--chdir={dir_rel}")),
        OsString::from("--format=json"),
        OsString::from("--no-color"),
        OsString::from("--call-module-type=none"),
    ];
    for file in files {
        let mut flag = OsString::from("--filter=");
        flag.push(file.as_os_str());
        argv.push(flag);
    }
    Invocation {
        argv,
        cwd_rel: String::new(),
    }
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

pub fn gplint_check(
    launch: &ResolvedTool,
    files: &[&Path],
    config: &Path,
) -> Result<Invocation, LaunchError> {
    let mut args: Vec<OsString> = GPLINT_PREFIX.iter().map(OsString::from).collect();
    args.push(OsString::from("-c"));
    args.push(config.as_os_str().to_owned());
    launch.resolve(&args, files)
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

/// Everything an ansible-lint invocation needs besides the binary and the files.
pub struct AnsibleLintContext<'a> {
    pub config: Option<&'a Path>,
    pub collections: &'a [&'a Path],
    pub project_dir: &'a Path,
}

/// `--offline --nocolor -f sarif --project-dir <dir>`, an optional `-c <config>`, and a leading `--dx-collections-path` when collection roots are staged.
pub fn ansible_lint_check(
    binary: &Path,
    files: &[&Path],
    context: &AnsibleLintContext<'_>,
) -> Invocation {
    let mut argv = vec![binary.as_os_str().to_owned()];
    if !context.collections.is_empty() {
        let joined = std::env::join_paths(context.collections.iter().copied())
            .unwrap_or_else(|_| OsString::new());
        argv.push(OsString::from("--dx-collections-path"));
        argv.push(joined);
    }
    argv.extend(
        ["--offline", "--nocolor", "-f", "sarif", "--project-dir"]
            .iter()
            .map(OsString::from),
    );
    argv.push(context.project_dir.as_os_str().to_owned());
    if let Some(config) = context.config {
        argv.push(OsString::from("-c"));
        argv.push(config.as_os_str().to_owned());
    }
    argv.extend(files.iter().map(|file| file.as_os_str().to_owned()));
    Invocation {
        argv,
        cwd_rel: String::new(),
    }
}

/// `--offline --nocolor --fix=all --project-dir <dir>`, an optional `-c <config>`, and a leading `--dx-collections-path` when collection roots are staged.
pub fn ansible_lint_fix(
    binary: &Path,
    files: &[&Path],
    context: &AnsibleLintContext<'_>,
) -> Invocation {
    let mut argv = vec![binary.as_os_str().to_owned()];
    if !context.collections.is_empty() {
        let joined = std::env::join_paths(context.collections.iter().copied())
            .unwrap_or_else(|_| OsString::new());
        argv.push(OsString::from("--dx-collections-path"));
        argv.push(joined);
    }
    argv.extend(
        ["--offline", "--nocolor", "--fix=all", "--project-dir"]
            .iter()
            .map(OsString::from),
    );
    argv.push(context.project_dir.as_os_str().to_owned());
    if let Some(config) = context.config {
        argv.push(OsString::from("-c"));
        argv.push(config.as_os_str().to_owned());
    }
    argv.extend(files.iter().map(|file| file.as_os_str().to_owned()));
    Invocation {
        argv,
        cwd_rel: String::new(),
    }
}

/// `format --newline-kind LF --stdout FILE`: prints the formatted file; Bicep has no check mode.
pub fn bicep_format_check(binary: &Path, file: &Path) -> Invocation {
    fixed(binary, BICEP_FORMAT_CHECK_ARGS, &[file], "")
}

/// `format --newline-kind LF FILE` rewrites the file in place.
pub fn bicep_format_fix(binary: &Path, file: &Path) -> Invocation {
    fixed(binary, BICEP_FORMAT_FIX_ARGS, &[file], "")
}

/// `lint --no-restore --diagnostics-format sarif FILE`; `--no-restore` keeps a `br:` module
/// from being fetched, so an unrestored reference is an error diagnostic.
pub fn bicep_lint_check(binary: &Path, file: &Path) -> Invocation {
    fixed(binary, BICEP_LINT_ARGS, &[file], "")
}

/// `--format json --non-zero-exit-code informational --config-file <config> --` before the files; the config is mandatory so no ambient `.cfnlintrc` is read.
pub fn cfn_lint_check(binary: &Path, files: &[&Path], config: &Path) -> Invocation {
    let mut argv = vec![binary.as_os_str().to_owned()];
    argv.extend(
        [
            "--format",
            "json",
            "--non-zero-exit-code",
            "informational",
            "--config-file",
        ]
        .iter()
        .map(OsString::from),
    );
    argv.push(config.as_os_str().to_owned());
    argv.push(OsString::from("--"));
    argv.extend(files.iter().map(|file| file.as_os_str().to_owned()));
    Invocation {
        argv,
        cwd_rel: String::new(),
    }
}

/// The rules `sqlfluff format` applies; the format check lints with exactly these.
pub const SQLFLUFF_FORMAT_RULES: &str = "capitalisation,layout,ambiguous.union,convention.not_equal,convention.coalesce,convention.select_trailing_comma,convention.is_null,jinja.padding,structure.distinct";

/// `<verb-args> --config <config> --ignore-local-config ... --` before the files.
///
/// The config is mandatory and the only source of the dialect, so no ambient `.sqlfluff`, home or
/// app-dir config is read. Ignore files are disregarded because dx already owns the file set, and
/// one process keeps the batched run deterministic. Lint, format check and format fix share this
/// so they cannot drift apart.
fn sqlfluff_invocation(binary: &Path, verb: &[&str], files: &[&Path], config: &Path) -> Invocation {
    let mut argv = vec![binary.as_os_str().to_owned()];
    argv.extend(verb.iter().map(OsString::from));
    argv.push(OsString::from("--config"));
    argv.push(config.as_os_str().to_owned());
    argv.extend(
        [
            "--ignore-local-config",
            "--disregard-sqlfluffignores",
            "--nocolor",
            "--disable-progress-bar",
            "--processes",
            "1",
            "--",
        ]
        .iter()
        .map(OsString::from),
    );
    argv.extend(files.iter().map(|file| file.as_os_str().to_owned()));
    Invocation {
        argv,
        cwd_rel: String::new(),
    }
}

/// `sqlfluff lint --format json` with every rule.
pub fn sqlfluff_check(binary: &Path, files: &[&Path], config: &Path) -> Invocation {
    sqlfluff_invocation(binary, &["lint", "--format", "json"], files, config)
}

/// `sqlfluff lint --format json` limited to the rules `sqlfluff format` applies.
pub fn sqlfluff_format_check(binary: &Path, files: &[&Path], config: &Path) -> Invocation {
    sqlfluff_invocation(
        binary,
        &["lint", "--format", "json", "--rules", SQLFLUFF_FORMAT_RULES],
        files,
        config,
    )
}

/// `sqlfluff format` in place.
pub fn sqlfluff_format_fix(binary: &Path, files: &[&Path], config: &Path) -> Invocation {
    sqlfluff_invocation(binary, &["format"], files, config)
}

/// `check-jsonschema` for one (schema, filetype) group, offline.
///
/// `--dx-ref-map` is read by dx's wrapper, which serves pinned `$ref` targets from disk and
/// refuses every other download. `-vv` makes the JSON report list every checked file.
pub fn check_jsonschema_check(
    binary: &Path,
    ref_map: &Path,
    schema: &Path,
    filetype: &str,
    files: &[&Path],
) -> Invocation {
    let head = vec![
        binary.as_os_str().to_owned(),
        OsString::from("--dx-ref-map"),
        ref_map.as_os_str().to_owned(),
        OsString::from("--schemafile"),
        schema.as_os_str().to_owned(),
        OsString::from("--no-cache"),
        OsString::from("--output-format"),
        OsString::from("json"),
        OsString::from("--verbose"),
        OsString::from("--verbose"),
        OsString::from("--force-filetype"),
        OsString::from(filetype),
        OsString::from("--"),
    ];
    assemble(binary, head, files, "")
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

/// Always `--config-file <config>`: without it regal searches upward for `.regal/`, which is not hermetic.
pub fn regal_lint(binary: &Path, files: &[&Path], config: &Path) -> Invocation {
    required_flag(
        binary,
        REGAL_LINT_PREFIX,
        "--config-file",
        config,
        END_OF_FLAGS,
        files,
        "",
    )
}

/// Always `--hint=<config>`: without it hlint searches the cwd and its parents for `.hlint.yaml`.
/// A config holding `[]` keeps the built-in hints.
pub fn hlint_check(binary: &Path, files: &[&Path], config: &Path) -> Invocation {
    let mut flag = OsString::from("--hint=");
    flag.push(config.as_os_str());
    let head = vec![
        binary.as_os_str().to_owned(),
        OsString::from("--json"),
        flag,
        OsString::from("--"),
    ];
    assemble(binary, head, files, "")
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

/// Always `--config <config>`: without it statix searches upward from the cwd for `statix.toml`.
/// statix takes one target per call.
pub fn statix_check(binary: &Path, file: &Path, config: &Path) -> Invocation {
    required_flag(
        binary,
        STATIX_CHECK_PREFIX,
        "--config",
        config,
        END_OF_FLAGS,
        &[file],
        "",
    )
}

/// Always `--config <config>`: checkmake otherwise falls back to `$HOME/checkmake.ini`.
/// `--debug` makes it log the config it loaded, which the parser requires.
pub fn checkmake_check(binary: &Path, files: &[&Path], config: &Path) -> Invocation {
    optional_flag(
        binary,
        CHECKMAKE_PREFIX,
        "--config",
        Some(config),
        CHECKMAKE_TAIL,
        files,
        "",
    )
}

/// Always `--config=<config>`: cmakelint otherwise reads `$HOME/.cmakelintrc`. No `--spaces`,
/// `--linelength` or `--filter` flag is passed, because flags override the user's config.
pub fn cmakelint_check(binary: &Path, files: &[&Path], config: &Path) -> Invocation {
    let mut flag = OsString::from("--config=");
    flag.push(config.as_os_str());
    let mut argv = vec![binary.as_os_str().to_owned(), flag];
    argv.extend(CMAKELINT_TAIL.iter().map(OsString::from));
    argv.extend(files.iter().map(|file| file.as_os_str().to_owned()));
    Invocation {
        argv,
        cwd_rel: String::new(),
    }
}

pub fn statix_fix(binary: &Path, file: &Path, config: &Path) -> Invocation {
    required_flag(
        binary,
        STATIX_FIX_PREFIX,
        "--config",
        config,
        END_OF_FLAGS,
        &[file],
        "",
    )
}

pub fn deadnix_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, DEADNIX_CHECK_ARGS, files, "")
}

pub fn deadnix_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, DEADNIX_FIX_ARGS, files, "")
}

/// The suffix of a kubeconform config file.
pub const KUBECONFORM_CONFIG_SUFFIX: &str = ".kubeconform.json";
const KUBECONFORM_CORE_TEMPLATE: &str = "{{.ResourceKind}}{{.KindSuffix}}.json";
const KUBECONFORM_CRD_TEMPLATE: &str = "{{.Group}}/{{.ResourceKind}}_{{.ResourceAPIVersion}}.json";

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct KubeconformConfigFile {
    schema_version: u32,
    kubernetes_version: String,
    #[serde(default)]
    strict: bool,
    #[serde(default)]
    skip_kinds: Vec<String>,
    core_dir: String,
    crd_dir: String,
}

/// A parsed `<name>.kubeconform.json` with its schema directories resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KubeconformConfig {
    pub kubernetes_version: String,
    pub strict: bool,
    pub skip_kinds: Vec<String>,
    pub core_dir: PathBuf,
    pub crd_dir: PathBuf,
}

fn remote_location(location: &str) -> bool {
    location == "default" || location.contains("://")
}

fn schema_dir(base: &Path, key: &str, rel: &str) -> Result<PathBuf, String> {
    if remote_location(rel) {
        return Err(format!(
            "kubeconform {key} must be a local directory, got {rel:?}"
        ));
    }
    let bad = rel.is_empty()
        || rel.starts_with('/')
        || rel.contains('\\')
        || rel.contains(':')
        || rel
            .split('/')
            .any(|part| part.is_empty() || part == "." || part == "..");
    if bad {
        return Err(format!(
            "kubeconform {key} must be a relative forward-slash path below the config, got {rel:?}"
        ));
    }
    Ok(rel
        .split('/')
        .fold(base.to_path_buf(), |dir, part| dir.join(part)))
}

/// Parses a kubeconform config; its schema directories resolve against the config's directory.
pub fn kubeconform_config(config: &Path, bytes: &[u8]) -> Result<KubeconformConfig, String> {
    let name = config
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default();
    if !name.ends_with(KUBECONFORM_CONFIG_SUFFIX) {
        return Err(format!(
            "kubeconform config {} must end with {KUBECONFORM_CONFIG_SUFFIX}",
            config.display()
        ));
    }
    let raw: KubeconformConfigFile = serde_json::from_slice(bytes)
        .map_err(|err| format!("kubeconform config {}: {err}", config.display()))?;
    if raw.schema_version != 1 {
        return Err(format!(
            "kubeconform config schema_version must be 1, got {}",
            raw.schema_version
        ));
    }
    let version = &raw.kubernetes_version;
    if version.is_empty()
        || !version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-'))
    {
        return Err(format!(
            "kubeconform kubernetes_version is not a version: {version:?}"
        ));
    }
    for kind in &raw.skip_kinds {
        if kind.is_empty() || kind.contains(',') || kind.chars().any(char::is_whitespace) {
            return Err(format!(
                "kubeconform skip_kinds entry is not a kind: {kind:?}"
            ));
        }
    }
    let base = config.parent().unwrap_or(Path::new(""));
    Ok(KubeconformConfig {
        core_dir: schema_dir(base, "core_dir", &raw.core_dir)?,
        crd_dir: schema_dir(base, "crd_dir", &raw.crd_dir)?,
        kubernetes_version: raw.kubernetes_version,
        strict: raw.strict,
        skip_kinds: raw.skip_kinds,
    })
}

fn schema_location(dir: &Path, template: &str) -> Result<String, String> {
    let dir = dir
        .to_str()
        .ok_or_else(|| format!("kubeconform schema dir is not UTF-8: {}", dir.display()))?;
    let location = format!("{}/{template}", dir.trim_end_matches(['/', '\\']));
    if dir.is_empty() || remote_location(dir) {
        return Err(format!(
            "kubeconform schema location must be local, got {location:?}"
        ));
    }
    Ok(location)
}

/// Exactly two local schema locations; never `-ignore-missing-schemas`.
pub fn kubeconform_check(
    binary: &Path,
    config: &KubeconformConfig,
    files: &[&Path],
) -> Result<Invocation, String> {
    let mut head = vec![
        binary.as_os_str().to_owned(),
        OsString::from("-output"),
        OsString::from("json"),
        OsString::from("-summary=false"),
        OsString::from("-kubernetes-version"),
        OsString::from(&config.kubernetes_version),
    ];
    if config.strict {
        head.push(OsString::from("-strict"));
    }
    if !config.skip_kinds.is_empty() {
        head.push(OsString::from("-skip"));
        head.push(OsString::from(config.skip_kinds.join(",")));
    }
    for (dir, template) in [
        (&config.core_dir, KUBECONFORM_CORE_TEMPLATE),
        (&config.crd_dir, KUBECONFORM_CRD_TEMPLATE),
    ] {
        head.push(OsString::from("-schema-location"));
        head.push(OsString::from(schema_location(dir, template)?));
    }
    Ok(assemble(binary, head, files, ""))
}

/// `helm lint <chart> --quiet --kube-version V -f <values>...`; never fetches dependencies.
pub fn helm_lint(
    binary: &Path,
    chart_dir: &Path,
    values: &[&Path],
    kube_version: &str,
) -> Invocation {
    let mut head = vec![
        binary.as_os_str().to_owned(),
        OsString::from("lint"),
        chart_dir.as_os_str().to_owned(),
        OsString::from("--quiet"),
        OsString::from("--kube-version"),
        OsString::from(kube_version),
    ];
    helm_values(&mut head, values);
    assemble(binary, head, &[], "")
}

/// `helm template <release> <chart> --namespace NS --kube-version V -f <values>...`.
pub fn helm_template(
    binary: &Path,
    release: &str,
    namespace: &str,
    chart_dir: &Path,
    values: &[&Path],
    kube_version: &str,
) -> Invocation {
    let mut head = vec![
        binary.as_os_str().to_owned(),
        OsString::from("template"),
        OsString::from(release),
        chart_dir.as_os_str().to_owned(),
        OsString::from("--namespace"),
        OsString::from(namespace),
        OsString::from("--kube-version"),
        OsString::from(kube_version),
    ];
    helm_values(&mut head, values);
    assemble(binary, head, &[], "")
}

/// Values files in declared order; later files override earlier ones.
fn helm_values(head: &mut Vec<OsString>, values: &[&Path]) {
    for path in values {
        head.push(OsString::from("-f"));
        head.push(path.as_os_str().to_owned());
    }
}

pub fn keep_sorted_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, KEEP_SORTED_CHECK_ARGS, files, "")
}

pub fn notebook_outputs_check(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, NOTEBOOK_OUTPUTS_CHECK_ARGS, files, "")
}

pub fn notebook_outputs_fix(binary: &Path, files: &[&Path]) -> Invocation {
    fixed(binary, NOTEBOOK_OUTPUTS_FIX_ARGS, files, "")
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
        "keep_sorted" | "notebook_outputs" | "ruff" | "flake8" | "pydoclint" | "buildifier"
        | "deadnix" | "check_jsonschema" | "xmllint" => ArgTransport::Batched,
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
