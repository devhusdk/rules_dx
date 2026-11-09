//! Every tool with config metadata reads only the config dx binds. A tool that is added without a
//! row here fails `every_configured_tool_has_a_row`.

use super::*;
use std::collections::BTreeSet;

const CFG: &str = "/scratch/bound/cfg";
const BIN: &str = "/scratch/bin/tool";
const FILE: &str = "/scratch/src/file";

enum How {
    /// The bound config path is on the command line.
    Arg(fn(&Path, &Path, &Path) -> Invocation),
    /// The invocation carries a flag that turns config discovery off.
    NoAutoConfig(fn(&Path, &Path) -> Invocation, &'static str),
    /// The runner stages the bound config alone in the scratch tree, or passes it through a
    /// launcher the adapter cannot build here; the runner tests cover it.
    Staged(&'static str),
}

use How::{Arg, NoAutoConfig, Staged};

fn table() -> Vec<(&'static str, How)> {
    vec![
        ("ansible_lint", Staged("config in the staged context")),
        (
            "bicep",
            Staged("bicepconfig.json staged at the scratch root"),
        ),
        (
            "biome",
            Staged("runs with --config-path at the staged config dir"),
        ),
        ("buf", Staged("buf.yaml staged at the module root")),
        (
            "buildifier",
            Staged("runs from the staged config dir, --config=off otherwise"),
        ),
        ("cfn_lint", Arg(|b, f, c| cfn_lint_check(b, &[f], c))),
        ("checkmake", Arg(|b, f, c| checkmake_check(b, &[f], c))),
        (
            "clang_format",
            Arg(|b, f, c| clang_format_check(b, &[f], Some(c))),
        ),
        (
            "clang_tidy",
            Staged("config staged beside the compile database"),
        ),
        ("clippy", Staged("tier rendered as internal flags")),
        ("cmakelint", Arg(|b, f, c| cmakelint_check(b, &[f], c))),
        ("credo", Arg(|b, f, c| credo_check(b, &[f], c))),
        (
            "csharpier",
            Arg(|b, f, c| csharpier_check(b, &[f], Some(c))),
        ),
        (
            "dart",
            Staged("analysis_options.yaml staged at the scratch root"),
        ),
        ("deadnix", Staged("no config file; flags only")),
        (
            "detekt",
            Arg(|b, f, c| detekt_check(b, &[f], c, Path::new("/scratch/report"))),
        ),
        ("djlint", Arg(|b, f, c| djlint_check(b, &[f], Some(c)))),
        ("elvis", Arg(|b, f, c| elvis_check(b, &[f], c))),
        ("errcheck", Staged("no config file; flags only")),
        ("eslint", Staged("flat config passed by the launcher")),
        ("ffmt", Staged("runs in the staged config dir")),
        (
            "fortitude",
            Arg(|b, f, c| fortitude_check(b, &[f], Some(c))),
        ),
        ("fsharplint", Staged("config staged in the context dir")),
        ("gersemi", Arg(|b, f, c| gersemi_check(b, &[f], c))),
        ("govet", Staged("no config file; flags only")),
        ("gplint", Staged("config passed by the launcher")),
        ("graphql_eslint", Staged("config passed by the launcher")),
        ("hadolint", Arg(|b, f, c| hadolint_check(b, &[f], c))),
        (
            "helmfmt",
            Staged("bound config staged as .helmfmt in the working directory"),
        ),
        ("hlint", Arg(|b, f, c| hlint_check(b, &[f], c))),
        (
            "julialint",
            Arg(|b, f, c| julialint_check(b, Path::new("/s"), Path::new("/r"), c, &[f])),
        ),
        ("kubeconform", Staged("schema config rendered into flags")),
        (
            "latexindent",
            Arg(|b, f, c| latexindent_check(b, f, Path::new("/scratch/cruft"), Some(c))),
        ),
        ("luals", Staged(".luarc.json rendered into the work dir")),
        (
            "lychee",
            Arg(|b, f, c| lychee_check(b, &[f], Path::new("/scratch"), false, Some(c))),
        ),
        (
            "markdown_check",
            Staged("rules file staged and passed by the runner"),
        ),
        ("docstrfmt", Arg(|b, f, c| docstrfmt_check(b, &[f], c))),
        ("mbake", Arg(|b, f, c| mbake_check(b, &[f], c))),
        (
            "mix_format",
            Arg(|b, f, c| mix_format_check(b, &[f], Some(c))),
        ),
        (
            "perlcritic",
            Arg(|b, f, c| perlcritic_check(b, &[f], Some(c))),
        ),
        ("perltidy", Arg(|b, f, c| perltidy_check(b, f, Some(c)))),
        ("phpcs", Staged("standard passed as a staged ruleset")),
        ("phpstan", Staged("neon rendered into the scratch tree")),
        ("pmd", Arg(|b, f, c| pmd_check(b, &[f], c))),
        (
            "prettier",
            NoAutoConfig(|b, f| prettier_flags(b, f), "--no-editorconfig"),
        ),
        (
            "psscriptanalyzer",
            Staged("settings passed by the runner script"),
        ),
        (
            "pydoclint",
            Arg(|b, f, c| pydoclint_check(b, &[f], Some(c))),
        ),
        (
            "qmlformat",
            Arg(|b, f, c| qmlformat_check(b, &[f], Some(c))),
        ),
        ("qmllint", Staged("settings staged at the scratch root")),
        ("redocly", Staged("config passed by the launcher")),
        ("regal", Arg(|b, f, c| regal_lint(b, &[f], c))),
        ("remark", Staged("settings passed by the launcher")),
        ("roslyn", Staged("globalconfig staged at the scratch root")),
        ("rubocop", Arg(|b, f, c| rubocop_check(b, &[f], Some(c)))),
        ("rustfmt", Arg(|b, f, c| rustfmt(b, &[f], c, "2021", true))),
        ("scalafix", Staged("config in the staged context")),
        ("scalafmt", Arg(|b, f, c| scalafmt_check(b, &[f], Some(c)))),
        ("selene", Arg(|b, f, c| selene_check(b, &[f], Some(c)))),
        (
            "shellcheck",
            Arg(|b, f, c| shellcheck_check(b, &[f], Some(c))),
        ),
        (
            "shfmt",
            Staged("bound config staged as the only .editorconfig, with root = true"),
        ),
        ("spotbugs", Staged("exclude filter staged")),
        (
            "sqlfluff",
            Arg(|b, f, c| sqlfluff_check(b, &[f], c, "ansi")),
        ),
        ("staticcheck", Staged("runs from the staged config dir")),
        ("stylelint", Staged("config passed by the launcher")),
        (
            "stylua",
            NoAutoConfig(|b, f| stylua_check(b, &[f], None), "--no-editorconfig"),
        ),
        (
            "swift_format",
            Arg(|b, f, c| swift_format_check(b, f, Some(c))),
        ),
        ("swift_lint", Arg(|b, f, c| swift_lint(b, &[f], Some(c)))),
        (
            "taplo",
            NoAutoConfig(|b, f| taplo_lint(b, &[f], None), "--no-auto-config"),
        ),
        ("tflint", Arg(|b, f, c| tflint_check(b, "src", &[f], c))),
        ("trivy_config", Staged("config staged in the cache dir")),
        ("tsc", Staged("tsconfig staged at the scratch root")),
        ("ty", Arg(|b, f, c| ty_check(b, &[f], &[], Some(c)))),
        ("typos", Arg(|b, f, c| typos_check(b, &[f], Some(c)))),
        ("vale", Staged("runs from the staged config dir")),
        ("yamlfmt", Arg(|b, f, c| yamlfmt_check(b, &[f], Some(c)))),
        ("yamllint", Arg(|b, f, c| yamllint_check(b, &[f], Some(c)))),
        ("zanuda", Staged("no config file; flags only")),
        ("zizmor", Staged("no config file; flags only")),
        ("ruff", Arg(|b, f, c| ruff_check(b, &[f], Some(c), ""))),
    ]
}

fn prettier_flags(_binary: &Path, _file: &Path) -> Invocation {
    Invocation {
        argv: prettier_args(None, PRETTIER_CHECK_TAIL),
        cwd_rel: String::new(),
    }
}

fn configured_tools() -> BTreeSet<String> {
    let path = std::env::var("DX_TOOL_CONFIGS").expect("DX_TOOL_CONFIGS must name tools_gen.rs");
    std::fs::read_to_string(dx_testing::resolve_runfiles(&path))
        .expect("read tools_gen.rs")
        .lines()
        .filter_map(|line| line.trim().strip_prefix("id: \""))
        .map(|id| id.trim_end_matches("\",").to_owned())
        .collect()
}

#[test]
fn every_configured_tool_has_a_row() {
    let rows: BTreeSet<String> = table().iter().map(|(id, _)| (*id).to_owned()).collect();
    let configured = configured_tools();
    let missing: Vec<_> = configured.difference(&rows).collect();
    let stale: Vec<_> = rows.difference(&configured).collect();
    assert!(
        missing.is_empty() && stale.is_empty(),
        "add a hermetic config row for {missing:?}; remove rows for {stale:?}"
    );
}

#[test]
fn tools_read_only_their_bound_config() {
    let (bin, file, cfg) = (Path::new(BIN), Path::new(FILE), Path::new(CFG));
    for (id, how) in table() {
        match how {
            Arg(build) => {
                let argv = argv_of(&build(bin, file, cfg));
                assert!(
                    argv.iter().any(|arg| arg.contains(CFG)),
                    "{id} must pass the bound config: {argv:?}"
                );
            }
            NoAutoConfig(build, flag) => {
                let argv = argv_of(&build(bin, file));
                assert!(
                    argv.iter().any(|arg| arg == flag),
                    "{id} needs {flag}: {argv:?}"
                );
            }
            Staged(reason) => assert!(!reason.is_empty(), "{id}"),
        }
    }
}

fn argv_of(invocation: &Invocation) -> Vec<String> {
    invocation
        .argv
        .iter()
        .map(|arg| arg.to_string_lossy().into_owned())
        .collect()
}
