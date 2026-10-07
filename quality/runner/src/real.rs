use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io;
use std::path::{Path, PathBuf};

use quality_adapter::commands::Invocation;
use quality_adapter::exec::{self, ChildOutput, MirrorFile, Scratch};
use quality_adapter::parsers::ParseError;

use crate::RunnerError;

pub const REAL_TOOLS: &[&str] = &[
    "biome",
    "buf",
    "buildifier",
    "checkstyle",
    "clang_format",
    "clang_tidy",
    "clippy",
    "cppcheck",
    "csharpier",
    "cue",
    "djlint",
    "errcheck",
    "eslint",
    "fantomas",
    "flake8",
    "fsharplint",
    "gofumpt",
    "google_java_format",
    "govet",
    "jsonnetfmt",
    "keep_sorted",
    "ktfmt",
    "ktlint",
    "markdown_check",
    "modfmt",
    "pkl",
    "pmd",
    "prettier",
    "psscriptanalyzer",
    "pydoclint",
    "pylint",
    "qmlformat",
    "qmllint",
    "roslyn",
    "rubocop",
    "ruff",
    "rustc",
    "rustfmt",
    "scalafix",
    "scalafmt",
    "shellcheck",
    "shfmt",
    "spotbugs",
    "standardrb",
    "staticcheck",
    "stylelint",
    "taplo",
    "terraform",
    "ty",
    "vale",
    "yamlfmt",
    "yamllint",
];

const RUSTFMT_DEFAULTS_REL: &str = "dx-rustfmt-default.toml";

const BIOME_DEFAULTS_REL: &str = "dx-biome-default/biome.json";
const BIOME_DEFAULTS_BYTES: &[u8] = b"{}";

pub struct RealTool {
    pub binary: PathBuf,
    pub extra_env: Vec<(String, String)>,
    pub config_rel: Option<String>,
    pub edition: Option<String>,
    pub tool_files: Vec<(String, Vec<u8>)>,
    pub upstream_diagnostics: Vec<PathBuf>,
}

pub type SpawnFn = fn(&[OsString], &Path, &[(OsString, OsString)]) -> io::Result<ChildOutput>;

type StagedPair = (String, PathBuf);

struct StagedScratch {
    scratch: Scratch,
    pairs: Vec<StagedPair>,
    sibling_pairs: Vec<StagedPair>,
    resolve_pairs: Vec<StagedPair>,
}

pub struct RealBackend {
    tools: BTreeMap<String, RealTool>,
    scratch_parent: PathBuf,
    spawn: SpawnFn,
}

pub fn real_spawn(
    argv: &[OsString],
    cwd: &Path,
    env: &[(OsString, OsString)],
) -> io::Result<ChildOutput> {
    exec::spawn(argv, cwd, env)
}

/// The invocation with its binary spelled as an absolute path.
///
/// A tool reads its own runfiles from `argv[0]`, and the runner starts every tool in a
/// scratch directory it has just created. A relative binary therefore names its runfiles
/// relative to that scratch directory, where nothing was ever built. Windows leans on this
/// hardest: a batch launcher resolves `%~f0` against the working directory it was handed.
fn absolute_argv(argv: &[OsString]) -> Vec<OsString> {
    let mut out = argv.to_vec();
    if let Some(first) = out.first_mut() {
        let path = PathBuf::from(&*first);
        if !path.is_absolute() {
            if let Ok(cwd) = std::env::current_dir() {
                *first = cwd.join(path).into_os_string();
            }
        }
    }
    out
}

/// The runfiles manifest that sits beside a tool.
///
/// A launcher that guesses its own manifest can land on an unrelated runfiles tree, so
/// name the file that describes this tool. Which of the two names Bazel wrote depends on
/// whether the executable carries a suffix, so take whichever one is there.
///
/// `RUNFILES_DIR` is never rewritten: pointing it at the runner's own tree would break
/// every tool whose runfiles the runner does not carry.
fn own_runfiles_manifest(binary: &Path) -> Vec<(OsString, OsString)> {
    let path = if binary.is_absolute() {
        binary.to_owned()
    } else {
        std::env::current_dir().unwrap_or_default().join(binary)
    };
    vec![(
        OsString::from("RUNFILES_MANIFEST_FILE"),
        dx_path::manifest_for(&path)
            .to_string_lossy()
            .into_owned()
            .into(),
    )]
}

/// Launchers that read the manifest beside their own binary on every platform.
const ALWAYS_OWN_MANIFEST: &[&str] = &["pydoclint", "flake8", "pylint"];

/// Launchers that need the manifest only where there is no runfiles tree.
const WINDOWS_OWN_MANIFEST: &[&str] = &["eslint", "prettier"];

/// Whether a tool's launcher reads the runfiles manifest beside its own binary.
fn reads_own_manifest(tool_id: &str, windows: bool) -> bool {
    ALWAYS_OWN_MANIFEST.contains(&tool_id) || (windows && WINDOWS_OWN_MANIFEST.contains(&tool_id))
}

fn execution(tool_id: &str, detail: String) -> RunnerError {
    RunnerError::ToolExecution {
        tool_id: tool_id.to_owned(),
        detail,
    }
}

fn parsed<T>(tool_id: &str, result: Result<T, ParseError>) -> Result<T, RunnerError> {
    result.map_err(|err| RunnerError::ToolOutput {
        tool_id: tool_id.to_owned(),
        detail: err.to_string(),
    })
}

fn reanchor(
    tool_id: &str,
    pairs: &[(String, PathBuf)],
    workspace: &str,
) -> Result<PathBuf, RunnerError> {
    pairs
        .iter()
        .find(|(known, _)| known == workspace)
        .map(|(_, absolute)| absolute.clone())
        .ok_or_else(|| RunnerError::UnplaceableFinding {
            tool_id: tool_id.to_owned(),
            detail: format!("finding names unstaged file: {workspace}"),
        })
}

fn fresh_scratch(parent: &Path, tool_id: &str) -> Result<Scratch, RunnerError> {
    Scratch::create(parent).map_err(|err| execution(tool_id, format!("scratch: {err}")))
}

fn write_all(scratch: &Scratch, tool_id: &str, mirrors: &[MirrorFile]) -> Result<(), RunnerError> {
    scratch
        .materialize(mirrors)
        .map(|_| ())
        .map_err(|err| execution(tool_id, format!("materialize: {err}")))
}

fn cleaned<T>(tool_id: &str, scratch: Scratch, value: T) -> Result<T, RunnerError> {
    scratch
        .close()
        .map_err(|err| execution(tool_id, format!("scratch cleanup: {err}")))?;
    Ok(value)
}

fn hint_dir<'a>(config_rel: Option<&'a str>, cwd_rel: &'a str) -> Option<&'a str> {
    config_rel.map(|_| cwd_rel)
}

fn parent_rel(rel: &str) -> String {
    Path::new(rel)
        .parent()
        .and_then(|parent| parent.to_str())
        .unwrap_or_default()
        .to_owned()
}

impl RealBackend {
    pub fn new(tools: BTreeMap<String, RealTool>, scratch_parent: PathBuf) -> Self {
        Self {
            tools,
            scratch_parent,
            spawn: real_spawn,
        }
    }

    pub fn supports(&self, tool_id: &str) -> bool {
        self.tools.contains_key(tool_id)
    }

    fn tool(&self, tool_id: &str) -> Result<&RealTool, RunnerError> {
        self.tools
            .get(tool_id)
            .ok_or_else(|| RunnerError::UnknownTool {
                tool_id: tool_id.to_owned(),
            })
    }

    fn run(
        &self,
        tool_id: &str,
        tool: &RealTool,
        invocation: &Invocation,
        scratch: &Scratch,
    ) -> Result<ChildOutput, RunnerError> {
        let cwd = scratch.root().join(&invocation.cwd_rel);
        let extra: Vec<(OsString, OsString)> = tool
            .extra_env
            .iter()
            .map(|(key, value)| (OsString::from(key), OsString::from(value)))
            .collect();
        let ambient: Vec<(OsString, OsString)> = std::env::vars_os().collect();
        let mut env = exec::hermetic_env(scratch.root(), &extra, &ambient);
        if reads_own_manifest(tool_id, cfg!(windows)) {
            env.extend(own_runfiles_manifest(&tool.binary));
        }
        (self.spawn)(&absolute_argv(&invocation.argv), &cwd, &env)
            .map_err(|err| execution(tool_id, format!("spawn: {err}")))
    }
}

mod check;
mod delegated;
mod pipeline;
mod staging;

pub use pipeline::{
    run_real_pipeline, run_real_pipeline_with_resolve, run_real_pipeline_with_siblings,
};

#[path = "real_fix.rs"]
mod real_fix;

#[cfg(test)]
#[path = "real_core.rs"]
mod real_core;
#[cfg(test)]
#[path = "real_delegated.rs"]
mod real_delegated;
#[cfg(test)]
#[path = "real_fixtures.rs"]
mod real_fixtures;
#[cfg(test)]
#[path = "real_tools.rs"]
mod real_tools;

#[cfg(test)]
mod tests {
    use super::{
        absolute_argv, own_runfiles_manifest, reads_own_manifest, ALWAYS_OWN_MANIFEST,
        WINDOWS_OWN_MANIFEST,
    };
    use std::ffi::OsString;
    use std::path::Path;

    /// A tool starts in a scratch directory, so its own name is not enough to find runfiles.
    #[test]
    fn a_tool_is_always_given_its_binary_as_an_absolute_path() {
        let relative = [
            OsString::from("bazel-out/bin/tool"),
            OsString::from("--check"),
        ];
        let absolute = absolute_argv(&relative);
        assert!(Path::new(&absolute[0]).is_absolute());
        assert_eq!(absolute[1], relative[1]);
        let already = [OsString::from("/out/bin/tool")];
        assert_eq!(absolute_argv(&already)[0], already[0]);
        assert!(absolute_argv(&[]).is_empty());
    }

    /// The manifest sits beside the tool that reads it.
    #[test]
    fn the_manifest_sits_beside_the_binary() {
        let env = own_runfiles_manifest(Path::new("/out/bin/quality/tools/python/pydoclint"));
        assert_eq!(
            env,
            vec![(
                OsString::from("RUNFILES_MANIFEST_FILE"),
                OsString::from("/out/bin/quality/tools/python/pydoclint.runfiles_manifest")
            )]
        );
    }

    /// Neither manifest is on disk yet, so the executable's own name is the answer.
    #[test]
    fn a_windows_launcher_keeps_its_own_suffix() {
        for name in ["/out/bin/prettier_/prettier.bat", "/out/bin/prettier.exe"] {
            let env = own_runfiles_manifest(Path::new(name));
            assert_eq!(
                env,
                vec![(
                    OsString::from("RUNFILES_MANIFEST_FILE"),
                    OsString::from(format!("{name}.runfiles_manifest"))
                )]
            );
        }
    }

    /// A suffix Bazel dropped from the executable still names the manifest.
    #[test]
    fn a_manifest_named_without_the_suffix_is_taken_when_it_exists() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let bare = dir.path().join("prettier");
        let named = format!("{}.runfiles_manifest", bare.display());
        std::fs::write(&named, "").expect("wrote manifest");
        assert_eq!(
            own_runfiles_manifest(&dir.path().join("prettier.bat")),
            vec![(
                OsString::from("RUNFILES_MANIFEST_FILE"),
                OsString::from(named)
            )]
        );
    }

    /// The Python launchers read wheels from the manifest on every platform.
    #[test]
    fn python_tools_read_their_own_manifest_everywhere() {
        for windows in [false, true] {
            for tool_id in ["pydoclint", "flake8", "pylint"] {
                assert!(reads_own_manifest(tool_id, windows), "{tool_id}");
            }
        }
    }

    /// The batch launcher reads its own manifest only on Windows.
    #[test]
    fn the_batch_launcher_reads_its_own_manifest_on_windows() {
        for tool_id in ["eslint", "prettier"] {
            assert!(reads_own_manifest(tool_id, true), "{tool_id}");
            assert!(!reads_own_manifest(tool_id, false), "{tool_id}");
        }
    }

    /// A tool sits in one list, because the two reasons do not overlap.
    #[test]
    fn one_tool_has_one_reason_to_read_its_manifest() {
        for tool_id in ALWAYS_OWN_MANIFEST {
            assert!(!WINDOWS_OWN_MANIFEST.contains(tool_id), "{tool_id}");
        }
        for tool_id in WINDOWS_OWN_MANIFEST {
            assert!(!ALWAYS_OWN_MANIFEST.contains(tool_id), "{tool_id}");
        }
    }

    /// A tool whose launcher finds its own tree is left alone.
    #[test]
    fn a_tool_that_finds_its_own_tree_is_left_alone() {
        for windows in [false, true] {
            assert!(!reads_own_manifest("buildifier", windows));
            assert!(!reads_own_manifest("biome", windows));
        }
    }
}
