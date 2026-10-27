use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::io;
use std::path::{Path, PathBuf};

use quality_adapter::commands::{self, Invocation};
use quality_adapter::exec::{self, ChildOutput, MirrorFile, Scratch};
use quality_adapter::launch;
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
    "erlfmt",
    "errcheck",
    "eslint",
    "fantomas",
    "flake8",
    "fsharplint",
    "gleam_format",
    "gofumpt",
    "google_java_format",
    "govet",
    "graphql_eslint",
    "hadolint",
    "jsonnetfmt",
    "keep_sorted",
    "ktfmt",
    "ktlint",
    "markdown_check",
    "mix_format",
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
    "zig_fmt",
];

const RUSTFMT_DEFAULTS_REL: &str = "dx-rustfmt-default.toml";

const KTLINT_DEFAULTS_REL: &str = "dx-ktlint-default/.editorconfig";
const KTLINT_DEFAULTS_BYTES: &[u8] = b"root = true

[*.{kt,kts}]
ktlint_code_style = intellij_idea
ij_kotlin_imports_layout = *
ktlint_standard_function-signature = disabled
ktlint_standard_trailing-comma-on-call-site = disabled
ktlint_standard_trailing-comma-on-declaration-site = disabled
";
const HADOLINT_DEFAULTS_REL: &str = "dx-hadolint-default/.hadolint.yaml";
/// hadolint rejects an empty config, so the default spells its empty ignore list.
const HADOLINT_DEFAULTS_BYTES: &[u8] = b"ignored: []\n";
const BIOME_DEFAULTS_REL: &str = "dx-biome-default/biome.json";
const BIOME_DEFAULTS_BYTES: &[u8] = b"{}";
const STYLELINT_DEFAULTS_REL: &str = "dx-stylelint-default/stylelint.json";
const STYLELINT_DEFAULTS_BYTES: &[u8] = br#"{
  "rules": {
    "block-no-empty": true,
    "color-no-invalid-hex": true,
    "comment-no-empty": true,
    "declaration-block-no-duplicate-custom-properties": true,
    "declaration-block-no-duplicate-properties": [
      true,
      {
        "ignore": [
          "consecutive-duplicates-with-different-values"
        ]
      }
    ],
    "declaration-block-no-shorthand-property-overrides": true,
    "font-family-no-duplicate-names": true,
    "function-calc-no-unspaced-operator": true,
    "keyframe-block-no-duplicate-selectors": true,
    "keyframe-declaration-no-important": true,
    "media-feature-name-no-unknown": true,
    "media-query-no-invalid": true,
    "named-grid-areas-no-invalid": true,
    "no-duplicate-at-import-rules": true,
    "no-duplicate-selectors": true,
    "no-empty-source": true,
    "no-invalid-position-at-import-rule": true,
    "no-irregular-whitespace": true,
    "property-no-unknown": [
      true,
      {
        "ignoreProperties": [
          "composes",
          "compose-with"
        ],
        "ignoreSelectors": [
          ":export",
          "/^:import/"
        ]
      }
    ],
    "selector-pseudo-class-no-unknown": [
      true,
      {
        "ignorePseudoClasses": [
          "export",
          "import",
          "global",
          "local",
          "external"
        ]
      }
    ],
    "selector-pseudo-element-no-unknown": true,
    "string-no-newline": true,
    "unit-no-unknown": true
  },
  "overrides": [
    {
      "files": [
        "*.scss"
      ],
      "customSyntax": "postcss-scss"
    },
    {
      "files": [
        "*.less"
      ],
      "customSyntax": "postcss-less"
    }
  ]
}
"#;

pub struct RealTool {
    pub binary: PathBuf,
    pub extra_env: Vec<(String, String)>,
    pub config_rel: Option<String>,
    pub edition: Option<String>,
    pub tool_files: Vec<(String, Vec<u8>)>,
    pub upstream_diagnostics: Vec<PathBuf>,
    pub search_paths: Vec<PathBuf>,
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
    argv_limit: fn(&OsStr) -> Option<usize>,
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

/// Whether a launcher finds its runfiles beside its real path, not beside a symlink to it.
fn resolves_launcher(tool_id: &str) -> bool {
    tool_id == "psscriptanalyzer"
}

/// Whether a tool's launcher reads the runfiles manifest beside its own binary.
fn reads_own_manifest(tool_id: &str) -> bool {
    ALWAYS_OWN_MANIFEST.contains(&tool_id)
}

/// Names the manifest beside one resolved JavaScript launcher when it exists.
fn javascript_manifest(binary: &Path) -> Option<(OsString, OsString)> {
    dx_path::manifest_beside(binary).map(|manifest| {
        (
            OsString::from("RUNFILES_MANIFEST_FILE"),
            manifest.into_os_string(),
        )
    })
}

fn execution(tool_id: &str, detail: String) -> RunnerError {
    RunnerError::ToolExecution {
        tool_id: tool_id.to_owned(),
        detail,
    }
}

fn parsed<T>(tool_id: &str, result: Result<T, ParseError>) -> Result<T, RunnerError> {
    result.map_err(|err| match err {
        // An incomplete analysis means the tool could not do its job, not that
        // its output was malformed.
        ParseError::Incomplete { .. } => execution(tool_id, err.to_string()),
        other => RunnerError::ToolOutput {
            tool_id: tool_id.to_owned(),
            detail: other.to_string(),
        },
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
            argv_limit: commands::native_argv_limit,
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
        if reads_own_manifest(tool_id) {
            env.extend(own_runfiles_manifest(&tool.binary));
        } else if launch::policy_for(tool_id) == launch::RunfilesPolicy::OwnManifest {
            if let Some(pair) = javascript_manifest(&tool.binary) {
                env.push(pair);
            }
        }
        let argv = spawn_argv(tool_id, invocation);
        if let Some(max) = argv.first().and_then(|program| (self.argv_limit)(program)) {
            let units = commands::argv_cost(&argv).1 + 2 * argv.len();
            if units > max {
                return Err(execution(
                    tool_id,
                    format!(
                        "selection too large for one {tool_id} spawn: {units} of {max} command-line characters"
                    ),
                ));
            }
        }
        (self.spawn)(&argv, &cwd, &env).map_err(|err| execution(tool_id, format!("spawn: {err}")))
    }
}

/// The argv a tool is spawned with: an absolute binary, resolved for launchers that need it.
fn spawn_argv(tool_id: &str, invocation: &Invocation) -> Vec<OsString> {
    let mut argv = absolute_argv(&invocation.argv);
    if resolves_launcher(tool_id) {
        if let Some(first) = argv.first_mut() {
            if let Ok(real) = std::fs::canonicalize(&*first) {
                *first = real.into_os_string();
            }
        }
    }
    argv
}

mod check;
mod delegated;
mod pipeline;
mod rustfmt;
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
        absolute_argv, javascript_manifest, own_runfiles_manifest, parsed, reads_own_manifest,
        resolves_launcher, spawn_argv, ALWAYS_OWN_MANIFEST, STYLELINT_DEFAULTS_BYTES,
    };
    use quality_adapter::launch::{policy_for, RunfilesPolicy};
    use std::ffi::OsString;
    use std::path::Path;

    use crate::RunnerError;
    use quality_adapter::parsers::ParseError;

    /// An incomplete analysis is an execution failure for every tool, never "invalid tool output".
    #[test]
    fn incomplete_analysis_is_a_tool_execution_failure() {
        for tool in ["buf", "fantomas", "roslyn", "sarif"] {
            let err = parsed::<()>(
                tool,
                Err(ParseError::Incomplete {
                    tool: "tool",
                    detail: "import failed".to_owned(),
                }),
            )
            .expect_err("incomplete fails");
            assert!(matches!(err, RunnerError::ToolExecution { .. }), "{err}");
            let text = err.to_string();
            assert!(!text.contains("invalid tool output"), "{text}");
            assert!(
                text.contains("analysis was incomplete: import failed"),
                "{text}"
            );
        }
    }

    /// Other parse errors stay malformed-output failures.
    #[test]
    fn other_parse_errors_stay_tool_output_failures() {
        let err = parsed::<()>(
            "taplo",
            Err(ParseError::Json {
                tool: "taplo",
                detail: "bad".to_owned(),
            }),
        )
        .expect_err("json fails");
        assert!(matches!(err, RunnerError::ToolOutput { .. }), "{err}");
    }

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

    /// The planned spawn carries the absolute binary the guard and the batches measure.
    #[test]
    fn spawn_argv_makes_the_binary_absolute() {
        let invocation = quality_adapter::commands::Invocation {
            argv: vec![
                OsString::from("bazel-out/bin/ruff"),
                OsString::from("check"),
            ],
            cwd_rel: String::new(),
        };
        let argv = spawn_argv("ruff", &invocation);
        assert!(Path::new(&argv[0]).is_absolute());
        assert!(argv[0].len() > invocation.argv[0].len());
        assert_eq!(argv[1], invocation.argv[1]);
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
        for tool_id in ["pydoclint", "flake8", "pylint"] {
            assert!(reads_own_manifest(tool_id), "{tool_id}");
        }
    }

    /// The PowerShell launcher runs from its real path.
    #[test]
    fn the_powershell_launcher_is_resolved_through_symlinks() {
        assert!(resolves_launcher("psscriptanalyzer"));
        assert!(!resolves_launcher("buildifier"));
    }

    /// The resolved JavaScript launch names its own runfiles on every platform.
    #[test]
    fn javascript_tools_resolve_to_their_own_manifest() {
        for tool_id in ["eslint", "graphql_eslint", "prettier", "stylelint"] {
            assert_eq!(
                policy_for(tool_id),
                RunfilesPolicy::OwnManifest,
                "{tool_id}"
            );
        }
    }

    /// A manifest beside the launcher is named, and an absent one is inherited.
    #[test]
    fn a_javascript_manifest_is_taken_only_when_it_exists() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let binary = dir.path().join("prettier");
        assert_eq!(javascript_manifest(&binary), None);
        let named = dir.path().join("prettier.runfiles_manifest");
        std::fs::write(&named, "").expect("wrote manifest");
        assert_eq!(
            javascript_manifest(&binary),
            Some((
                OsString::from("RUNFILES_MANIFEST_FILE"),
                named.into_os_string()
            ))
        );
    }

    /// A tool sits in one list, because the two reasons do not overlap.
    #[test]
    fn one_tool_has_one_reason_to_read_its_manifest() {
        for tool_id in ALWAYS_OWN_MANIFEST {
            assert_eq!(policy_for(tool_id), RunfilesPolicy::Inherit, "{tool_id}");
        }
        for tool_id in ["eslint", "graphql_eslint", "prettier", "stylelint"] {
            assert!(!reads_own_manifest(tool_id), "{tool_id}");
        }
    }

    /// A tool whose launcher finds its own tree is left alone.
    #[test]
    fn a_tool_that_finds_its_own_tree_is_left_alone() {
        for tool_id in ["buildifier", "biome"] {
            assert!(!reads_own_manifest(tool_id));
            assert_eq!(policy_for(tool_id), RunfilesPolicy::Inherit);
        }
    }

    /// The default stylelint rules accept CSS Modules selectors and `composes`.
    #[test]
    fn the_stylelint_defaults_accept_css_modules() {
        let config: serde_json::Value =
            serde_json::from_slice(STYLELINT_DEFAULTS_BYTES).expect("defaults are JSON");
        let rules = &config["rules"];
        let pseudo = &rules["selector-pseudo-class-no-unknown"];
        assert_eq!(pseudo[0], true);
        for name in ["global", "local", "export", "import"] {
            assert!(
                pseudo[1]["ignorePseudoClasses"]
                    .as_array()
                    .expect("ignorePseudoClasses")
                    .contains(&serde_json::Value::from(name)),
                "{name}"
            );
        }
        let property = &rules["property-no-unknown"];
        assert_eq!(property[0], true);
        assert!(property[1]["ignoreProperties"]
            .as_array()
            .expect("ignoreProperties")
            .contains(&serde_json::Value::from("composes")));
    }
}
