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
    "ansible_lint",
    "biome",
    "buf",
    "buildifier",
    "cfn_lint",
    "checkmake",
    "checkstyle",
    "cmakelint",
    "clang_format",
    "clang_syntax",
    "clang_tidy",
    "clippy",
    "cppcheck",
    "credo",
    "csc",
    "csharpier",
    "cue",
    "dart",
    "deadnix",
    "dfmt",
    "dialyzer",
    "dscanner",
    "djlint",
    "elvis",
    "erlfmt",
    "errcheck",
    "eslint",
    "fantomas",
    "flake8",
    "flatc",
    "fortitude",
    "fsharplint",
    "gersemi",
    "gofumpt",
    "gotype",
    "google_java_format",
    "govet",
    "gplint",
    "graphql_eslint",
    "hadolint",
    "helm",
    "hlint",
    "javac",
    "jsonnet_lint",
    "jsonnetfmt",
    "julia_format",
    "keep_sorted",
    "latexindent",
    "kubeconform",
    "ktfmt",
    "ktlint",
    "luals",
    "markdown_check",
    "mix_format",
    "modfmt",
    "nixfmt",
    "notebook_outputs",
    "nvcc_syntax",
    "ormolu",
    "opa",
    "perlcritic",
    "perltidy",
    "pkl",
    "phpcs",
    "phpstan",
    "pmd",
    "prettier",
    "psscriptanalyzer",
    "pydoclint",
    "pylint",
    "qmlformat",
    "qmllint",
    "redocly",
    "regal",
    "remark",
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
    "sqlfluff",
    "standardrb",
    "staticcheck",
    "statix",
    "stylelint",
    "stylua",
    "taplo",
    "terraform",
    "tflint",
    "tsc",
    "ty",
    "typstyle",
    "vale",
    "yamlfmt",
    "yamllint",
    "zig_ast_check",
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
const TFLINT_DEFAULTS_REL: &str = "dx-tflint-default/.tflint.hcl";
/// Only the bundled terraform ruleset, recommended preset; module calls stay off.
const TFLINT_DEFAULTS_BYTES: &[u8] = b"config {
  call_module_type = \"none\"
}

plugin \"terraform\" {
  enabled = true
  preset  = \"recommended\"
}
";
/// Empty plugin home: tflint never downloads or finds a provider ruleset.
const TFLINT_PLUGIN_DIR_REL: &str = "dx-tflint-plugins";
/// Scratch dir holding one launcher shim per `path_tools` helper; the child's whole `PATH`.
const PATH_TOOLS_DIR_REL: &str = "dx-path-tools";
const CREDO_DEFAULTS_REL: &str = "dx-credo-default/.credo.exs";
/// Credo's own default checks over every staged file: Bazel decides which files are owned, so
/// no excluded path pattern may drop one.
const CREDO_DEFAULTS_BYTES: &[u8] = b"%{
  configs: [
    %{name: \"default\", files: %{included: [\"**/*.{ex,exs}\"], excluded: []}}
  ]
}
";
const CHECKMAKE_DEFAULTS_REL: &str = "dx-checkmake-default/checkmake.ini";
/// An empty ini loads and keeps every rule at its default.
const CHECKMAKE_DEFAULTS_BYTES: &[u8] = b"";
const REGAL_DEFAULTS_REL: &str = ".regal/config.yaml";
const REGAL_DEFAULTS_BYTES: &[u8] = b"";
const GERSEMI_DEFAULTS_REL: &str = "dx-gersemi-default/.gersemirc";
/// An empty config loads and keeps every option at its default.
const GERSEMI_DEFAULTS_BYTES: &[u8] = b"";
const HADOLINT_DEFAULTS_REL: &str = "dx-hadolint-default/.hadolint.yaml";
/// hadolint rejects an empty config, so the default spells its empty ignore list.
const HADOLINT_DEFAULTS_BYTES: &[u8] = b"ignored: []\n";
const HLINT_DEFAULTS_REL: &str = "dx-hlint-default/hlint.yaml";
/// An empty hint list keeps the built-in hints and stops hlint from searching for `.hlint.yaml`.
const HLINT_DEFAULTS_BYTES: &[u8] = b"[]\n";
/// The stock rulesets for every staged `.erl` and `.hrl`, at any depth.
const ELVIS_DEFAULTS_REL: &str = "dx-elvis-default/elvis.config";
const ELVIS_DEFAULTS_BYTES: &[u8] = b"[{config, [
    #{files => [\"**/*.erl\"], ruleset => erl_files, allow_no_files => true},
    #{files => [\"**/*.hrl\"], ruleset => hrl_files, allow_no_files => true}
]}].
";
/// dfmt is always pointed at this directory (see `commands::DFMT_CONFIG_DIR`): `root = true`
/// stops its upward search for `.editorconfig`, so a home-directory file never reaches it.
const DFMT_DEFAULTS_FILE: &str = ".editorconfig";
const DFMT_DEFAULTS_BYTES: &[u8] = b"root = true\n\n[*.d]\nend_of_line = lf\n";
/// dscanner is always pointed at this file (see `commands::DSCANNER_CONFIG_REL`): the header alone
/// keeps every upstream default check on.
const DSCANNER_DEFAULTS_BYTES: &[u8] = b"[analysis.config.StaticAnalysisConfig]\n";
const STATIX_DEFAULTS_REL: &str = "dx-statix-default/statix.toml";
/// An empty `statix.toml` keeps every lint on and stops statix searching upward for one.
const STATIX_DEFAULTS_BYTES: &[u8] = b"";
const BIOME_DEFAULTS_REL: &str = "dx-biome-default/biome.json";
const BIOME_DEFAULTS_BYTES: &[u8] = b"{}";
const CUE_STATE_REL: &str = ".dx-cue-state";
const REDOCLY_DEFAULTS_REL: &str = "dx-redocly-default/redocly.yaml";
/// The curated Redocly policy: spec structure plus the correctness rules, all as errors.
const REDOCLY_DEFAULTS_BYTES: &[u8] = b"extends:
  - spec
rules:
  no-unresolved-refs: error
  operation-operationId-unique: error
  path-parameters-defined: error
  no-identical-paths: error
  operation-parameters-unique: error
telemetry: off
";
/// The curated gplint rules cover meaning only. Layout belongs to prettier, and rules that
/// compare files (`no-dupe-feature-names`, `no-dupe-scenario-names` in `anywhere` mode) would
/// make a result depend on which files share a batch, so none of those are listed.
const GPLINT_DEFAULTS_REL: &str = "dx-gplint-default/gplint.json";
const GPLINT_DEFAULTS_BYTES: &[u8] = br#"{
  "no-background-only-scenario": "error",
  "no-dupe-scenario-names": ["error", "in-feature"],
  "no-duplicate-tags": "error",
  "no-empty-background": "error",
  "no-empty-file": "error",
  "no-examples-in-scenarios": "error",
  "no-files-without-scenarios": "error",
  "no-homogenous-tags": "error",
  "no-scenario-outlines-without-examples": "error",
  "no-superfluous-tags": "error",
  "no-unnamed-features": "error",
  "no-unnamed-scenarios": "error",
  "no-unused-variables": "error"
}
"#;
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
    pub dialect: Option<String>,
    pub tool_files: Vec<(String, Vec<u8>)>,
    pub upstream_diagnostics: Vec<PathBuf>,
    pub search_paths: Vec<PathBuf>,
    /// Helper executables placed on the child's `PATH` as shims: `(name, binary)`.
    pub path_tools: Vec<(String, PathBuf)>,
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
const ALWAYS_OWN_MANIFEST: &[&str] = &["pydoclint", "flake8", "pylint", "rst2html"];

/// Whether a launcher finds its runfiles beside its real path, not beside a symlink to it.
fn resolves_launcher(tool_id: &str) -> bool {
    matches!(tool_id, "psscriptanalyzer" | "julia_format")
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

/// The runfiles env one launcher needs, keyed by the tool the launcher belongs to.
fn launcher_env(tool_id: &str, binary: &Path) -> Vec<(OsString, OsString)> {
    if reads_own_manifest(tool_id) {
        return own_runfiles_manifest(binary);
    }
    if launch::policy_for(tool_id) == launch::RunfilesPolicy::OwnManifest {
        return javascript_manifest(binary).into_iter().collect();
    }
    Vec::new()
}

/// A `/bin/sh` shim that sets the launcher's env, then execs its absolute path.
fn unix_shim(binary: &str, env: &[(String, String)]) -> String {
    fn quote(text: &str) -> String {
        format!("'{}'", text.replace('\'', "'\\''"))
    }
    let mut out = String::from("#!/bin/sh\n");
    for (key, value) in env {
        out.push_str(&format!("{key}={}\nexport {key}\n", quote(value)));
    }
    // The child PATH is only the shim dir; launchers that start with
    // `#!/usr/bin/env bash` need the system default search path back.
    out.push_str("unset PATH\n");
    out.push_str(&format!("exec {} \"$@\"\n", quote(binary)));
    out
}

/// A `.cmd` shim with CRLF endings. Characters `cmd.exe` would reinterpret are refused.
fn windows_shim(binary: &str, env: &[(String, String)]) -> Result<String, String> {
    let bad = |text: &str| {
        text.chars()
            .any(|c| matches!(c, '%' | '"' | '^' | '!' | '\n' | '\r'))
    };
    let mut out = String::from("@echo off\r\n");
    for (key, value) in env {
        if bad(key) || bad(value) {
            return Err(format!("unsafe character in {key} value"));
        }
        out.push_str(&format!("set \"{key}={value}\"\r\n"));
    }
    if bad(binary) {
        return Err("unsafe character in launcher path".to_owned());
    }
    out.push_str("set \"PATH=\"\r\n");
    out.push_str(&format!("\"{binary}\" %*\r\n"));
    Ok(out)
}

fn write_path_shim(
    dir: &Path,
    name: &str,
    binary: &Path,
    env: &[(OsString, OsString)],
) -> Result<(), String> {
    let env: Vec<(String, String)> = env
        .iter()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            )
        })
        .collect();
    let binary = binary.to_string_lossy();
    if cfg!(windows) {
        let body = windows_shim(&binary, &env)?;
        std::fs::write(dir.join(format!("{name}.cmd")), body).map_err(|err| err.to_string())
    } else {
        let path = dir.join(name);
        std::fs::write(&path, unix_shim(&binary, &env)).map_err(|err| err.to_string())?;
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))
                .map_err(|err| err.to_string())?;
        }
        Ok(())
    }
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

/// Adds the last lines of a tool's stderr to its output error.
fn with_stderr(err: RunnerError, stderr: &[u8]) -> RunnerError {
    let text = String::from_utf8_lossy(stderr);
    let lines: Vec<&str> = text.trim().lines().collect();
    let tail = lines[lines.len().saturating_sub(20)..].join("\n");
    match err {
        RunnerError::ToolOutput { tool_id, detail } if !tail.is_empty() => {
            RunnerError::ToolOutput {
                tool_id,
                detail: format!("{detail}: {tail}"),
            }
        }
        other => other,
    }
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
        env.extend(launcher_env(tool_id, &tool.binary));
        if !tool.path_tools.is_empty() {
            let dir = scratch.root().join(PATH_TOOLS_DIR_REL);
            std::fs::create_dir_all(&dir)
                .map_err(|err| execution(tool_id, format!("path tools dir: {err}")))?;
            for (name, binary) in &tool.path_tools {
                let shim_env = launcher_env(name, binary);
                write_path_shim(&dir, name, binary, &shim_env)
                    .map_err(|err| execution(tool_id, format!("path tool {name}: {err}")))?;
            }
            env.retain(|(key, _)| !key.to_string_lossy().eq_ignore_ascii_case("PATH"));
            env.push((OsString::from("PATH"), dir.into_os_string()));
        }
        if tool_id == "tflint" {
            let plugins = scratch.root().join(TFLINT_PLUGIN_DIR_REL);
            std::fs::create_dir_all(&plugins)
                .map_err(|err| execution(tool_id, format!("plugin dir: {err}")))?;
            env.retain(|(key, _)| key != "TFLINT_CONFIG_FILE");
            env.push((
                OsString::from("TFLINT_PLUGIN_DIR"),
                plugins.into_os_string(),
            ));
            env.push((OsString::from("TFLINT_LOG"), OsString::new()));
            if cfg!(unix) {
                // The bundled ruleset talks over a unix socket made in TMPDIR, and
                // a long scratch path overflows the socket path limit.
                let short = short_socket_dir()
                    .map_err(|err| execution(tool_id, format!("socket dir: {err}")))?;
                for (key, value) in &mut env {
                    if key == "TMPDIR" || key == "TEMP" || key == "TMP" {
                        *value = short.path().as_os_str().to_owned();
                    }
                }
                let result = (self.spawn)(&spawn_argv(tool_id, invocation), &cwd, &env)
                    .map_err(|err| execution(tool_id, format!("spawn: {err}")));
                drop(short);
                return result;
            }
        }
        if tool_id == "typstyle" {
            env.push((OsString::from("NO_COLOR"), OsString::from("1")));
        }
        if tool_id == "cue" {
            // HOME is unset; module loads need a cache and config dir, kept in the scratch.
            let state = scratch.root().join(CUE_STATE_REL);
            env.push((
                OsString::from("CUE_CACHE_DIR"),
                state.join("cache").into_os_string(),
            ));
            env.push((
                OsString::from("CUE_CONFIG_DIR"),
                state.join("config").into_os_string(),
            ));
        }
        if tool_id == "dart" {
            // The host HOME stays unset (dart format needs none); analytics stay off.
            env.push((
                OsString::from("DART_SUPPRESS_ANALYTICS"),
                OsString::from("true"),
            ));
        }
        if tool_id == "redocly" {
            env.push((OsString::from("REDOCLY_TELEMETRY"), OsString::from("off")));
            env.push((
                OsString::from("REDOCLY_SUPPRESS_UPDATE_NOTICE"),
                OsString::from("true"),
            ));
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

/// A short-named temp directory, removed on drop, for tools that bind unix sockets there.
struct SocketDir(PathBuf);

impl SocketDir {
    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for SocketDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn short_socket_dir() -> io::Result<SocketDir> {
    static NEXT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);
    let base = if Path::new("/tmp").is_dir() {
        PathBuf::from("/tmp")
    } else {
        std::env::temp_dir()
    };
    loop {
        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        let dir = base.join(format!("dxtfl-{}-{n}", std::process::id()));
        match std::fs::create_dir(&dir) {
            Ok(()) => return Ok(SocketDir(dir)),
            Err(err) if err.kind() == io::ErrorKind::AlreadyExists => {}
            Err(err) => return Err(err),
        }
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

mod cfn_lint;
mod check;
mod cmakelint;
mod delegated;
mod helm;
mod kubeconform;
mod pipeline;
mod redocly;
mod rustfmt;
mod sqlfluff;
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
#[path = "real_helm.rs"]
mod real_helm;
#[cfg(test)]
#[path = "real_tools.rs"]
mod real_tools;

#[cfg(test)]
mod tests {
    use super::{
        absolute_argv, javascript_manifest, own_runfiles_manifest, parsed, reads_own_manifest,
        resolves_launcher, spawn_argv, ALWAYS_OWN_MANIFEST, GPLINT_DEFAULTS_BYTES, REAL_TOOLS,
        STYLELINT_DEFAULTS_BYTES,
    };
    use crate::RunnerError;
    use quality_adapter::launch::{policy_for, RunfilesPolicy};
    use std::ffi::OsString;
    use std::path::{Path, PathBuf};

    fn out() -> PathBuf {
        std::env::temp_dir().join("out")
    }

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

    #[test]
    fn an_output_error_carries_the_tool_stderr_tail() {
        let error = || RunnerError::ToolOutput {
            tool_id: "fsharplint".to_owned(),
            detail: "exit 1 (want 0)".to_owned(),
        };
        let stderr: String = (1..=25).map(|n| format!("line {n}\n")).collect();
        let RunnerError::ToolOutput { detail, .. } = super::with_stderr(error(), stderr.as_bytes())
        else {
            panic!("kind changed");
        };
        assert!(detail.starts_with("exit 1 (want 0): line 6\n"), "{detail}");
        assert!(detail.ends_with("line 25"), "{detail}");
        assert_eq!(
            super::with_stderr(error(), b" \n").to_string(),
            error().to_string()
        );
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
        let already = [out().join("bin").join("tool").into_os_string()];
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
        let binary = out().join("bin").join("pydoclint");
        let env = own_runfiles_manifest(&binary);
        assert_eq!(
            env,
            vec![(
                OsString::from("RUNFILES_MANIFEST_FILE"),
                OsString::from(format!("{}.runfiles_manifest", dx_path::native(&binary)))
            )]
        );
    }

    /// Neither manifest is on disk yet, so the executable's own name is the answer.
    #[test]
    fn a_windows_launcher_keeps_its_own_suffix() {
        for name in ["prettier_/prettier.bat", "prettier.exe"] {
            let binary = out().join("bin").join(name);
            let env = own_runfiles_manifest(&binary);
            assert_eq!(
                env,
                vec![(
                    OsString::from("RUNFILES_MANIFEST_FILE"),
                    OsString::from(format!("{}.runfiles_manifest", dx_path::native(&binary)))
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

    #[test]
    fn unix_shims_export_env_then_exec_the_quoted_launcher() {
        let body = super::unix_shim(
            "/w/it's/bin",
            &[("RUNFILES_MANIFEST_FILE".to_owned(), "/w/m".to_owned())],
        );
        assert_eq!(
            body,
            "#!/bin/sh\nRUNFILES_MANIFEST_FILE='/w/m'\nexport RUNFILES_MANIFEST_FILE\nunset PATH\nexec '/w/it'\\''s/bin' \"$@\"\n"
        );
    }

    #[test]
    fn windows_shims_use_crlf_and_refuse_cmd_metacharacters() {
        let body = super::windows_shim("C:\\t\\bin.bat", &[("K".to_owned(), "V".to_owned())])
            .expect("shim");
        assert_eq!(
            body,
            "@echo off\r\nset \"K=V\"\r\nset \"PATH=\"\r\n\"C:\\t\\bin.bat\" %*\r\n"
        );
        for bad in ["a%b", "a\"b", "a^b", "a!b", "a\nb"] {
            assert!(super::windows_shim(bad, &[]).is_err(), "{bad}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn a_path_shim_runs_the_launcher_with_its_env() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("dir");
        let launcher = dir.path().join("fake");
        std::fs::write(&launcher, "#!/bin/sh\necho \"$1:$K\"\n").expect("write");
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755)).expect("mode");
        super::write_path_shim(
            dir.path(),
            "rst2html",
            &launcher,
            &[(OsString::from("K"), OsString::from("v"))],
        )
        .expect("shim");
        let out = std::process::Command::new(dir.path().join("rst2html"))
            .arg("arg")
            .output()
            .expect("run");
        assert_eq!(String::from_utf8_lossy(&out.stdout), "arg:v\n");
    }

    #[cfg(unix)]
    #[test]
    fn a_path_shim_lets_env_shebang_launchers_find_their_interpreter() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().expect("dir");
        let launcher = dir.path().join("fake");
        std::fs::write(&launcher, "#!/usr/bin/env sh\necho ok\n").expect("write");
        std::fs::set_permissions(&launcher, std::fs::Permissions::from_mode(0o755)).expect("mode");
        super::write_path_shim(dir.path(), "tool", &launcher, &[]).expect("shim");
        let body = std::fs::read_to_string(dir.path().join("tool")).expect("read");
        assert!(body.contains("unset PATH\n"), "{body}");
        let out = std::process::Command::new(dir.path().join("tool"))
            .env("PATH", dir.path())
            .output()
            .expect("run");
        assert_eq!(String::from_utf8_lossy(&out.stdout), "ok\n");
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
        for tool_id in [
            "eslint",
            "gplint",
            "graphql_eslint",
            "prettier",
            "redocly",
            "remark",
            "stylelint",
        ] {
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
        for tool_id in [
            "eslint",
            "gplint",
            "graphql_eslint",
            "prettier",
            "redocly",
            "remark",
            "stylelint",
        ] {
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

    #[test]
    fn keep_sorted_runner_edge_is_present() {
        assert!(REAL_TOOLS.contains(&"keep_sorted"));
    }

    /// The gplint defaults parse and leave layout to prettier and cross-file checks out.
    #[test]
    fn the_gplint_defaults_hold_only_per_file_meaning_rules() {
        let config: serde_json::Value =
            serde_json::from_slice(GPLINT_DEFAULTS_BYTES).expect("defaults are JSON");
        let rules = config.as_object().expect("rules");
        assert!(!rules.is_empty());
        for banned in [
            "indentation",
            "table-align",
            "no-trailing-spaces",
            "new-line-at-eof",
            "no-multiple-empty-lines",
            "one-space-between-tags",
            "no-dupe-feature-names",
            "file-name",
        ] {
            assert!(!rules.contains_key(banned), "{banned}");
        }
        assert_eq!(
            rules["no-dupe-scenario-names"][1], "in-feature",
            "scenario names compare within one feature"
        );
    }

    /// The default stylelint rules accept CSS Modules selectors and `composes`.
    #[test]
    fn the_stylelint_defaults_accept_css_modules() {
        let config: serde_json::Value =
            serde_json::from_slice(STYLELINT_DEFAULTS_BYTES).expect("defaults are JSON");
        let rules = &config["rules"];
        let pseudo = &rules["selector-pseudo-class-no-unknown"];
        assert_eq!(pseudo[0], true);
        for name in ["global", "local", "export", "import", "external"] {
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
        for name in ["composes", "compose-with"] {
            assert!(
                property[1]["ignoreProperties"]
                    .as_array()
                    .expect("ignoreProperties")
                    .contains(&serde_json::Value::from(name)),
                "{name}"
            );
        }
        for name in [":export", "/^:import/"] {
            assert!(
                property[1]["ignoreSelectors"]
                    .as_array()
                    .expect("ignoreSelectors")
                    .contains(&serde_json::Value::from(name)),
                "{name}"
            );
        }
    }
}
