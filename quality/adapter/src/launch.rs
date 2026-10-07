use std::ffi::OsString;
use std::path::Path;

/// Tools whose launch goes through the explicit description.
///
/// eslint and prettier are the first migrated family. Later families keep
/// their existing wiring until their own migration lands.
const JS_LAUNCH_TOOLS: &[&str] = &["eslint", "prettier"];

/// Launch description resolved from declared tool inputs.
///
/// The program names the explicit executable or runtime. The entry point and
/// runtime arguments stay empty until a family declares a runtime wrapper of
/// its own. Environment holds launch-resolved entries beyond the declared
/// tool environment. No tool-name branching and no host detection happen
/// below this point.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedToolLaunch {
    pub tool_id: String,
    pub program: OsString,
    pub entry: Option<OsString>,
    pub runtime_args: Vec<OsString>,
    pub env: Vec<(OsString, OsString)>,
    pub cwd_rel: String,
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum LaunchError {
    #[error("{tool}: missing program: the aspect declared no binary")]
    MissingProgram { tool: String },
}

impl ResolvedToolLaunch {
    /// Resolves the launch for one tool, or nothing when its family is unmigrated.
    ///
    /// The manifest entry names the runfiles manifest beside the declared
    /// program when one is on disk. Without it the tool keeps its own
    /// discovery, so a tree-only execution never sees a dangling manifest.
    pub fn resolve(tool_id: &str, program: &Path) -> Result<Option<Self>, LaunchError> {
        if !JS_LAUNCH_TOOLS.contains(&tool_id) {
            return Ok(None);
        }
        if program.as_os_str().is_empty() {
            return Err(LaunchError::MissingProgram {
                tool: tool_id.to_owned(),
            });
        }
        let mut env = Vec::new();
        if let Some(manifest) = dx_path::manifest_beside(program) {
            env.push((
                OsString::from("RUNFILES_MANIFEST_FILE"),
                manifest.into_os_string(),
            ));
        }
        Ok(Some(Self {
            tool_id: tool_id.to_owned(),
            program: program.as_os_str().to_owned(),
            entry: None,
            runtime_args: Vec::new(),
            env,
            cwd_rel: String::new(),
            version: None,
        }))
    }

    /// Assembles the spawn argv, keeping every argument byte-identical.
    pub fn argv(&self, tool_args: &[OsString]) -> Vec<OsString> {
        let mut argv = Vec::with_capacity(2 + self.runtime_args.len() + tool_args.len());
        argv.push(self.program.clone());
        argv.extend(self.runtime_args.iter().cloned());
        if let Some(entry) = &self.entry {
            argv.push(entry.clone());
        }
        argv.extend(tool_args.iter().cloned());
        argv
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn os(value: &str) -> OsString {
        OsString::from(value)
    }

    #[test]
    fn unmigrated_families_resolve_to_nothing() {
        for tool_id in ["buildifier", "ruff", "pydoclint", "biome"] {
            assert_eq!(
                ResolvedToolLaunch::resolve(tool_id, Path::new("/fake/bin/tool")),
                Ok(None),
                "{tool_id} keeps its existing wiring"
            );
        }
    }

    #[test]
    fn js_tools_without_a_manifest_beside_the_program_set_no_env() {
        for tool_id in ["eslint", "prettier"] {
            let launch = ResolvedToolLaunch::resolve(tool_id, Path::new("/out/bin/tool"))
                .expect("js launch resolves")
                .expect("js tools are migrated");
            assert_eq!(launch.tool_id, tool_id);
            assert_eq!(launch.program, os("/out/bin/tool"));
            assert_eq!(launch.entry, None);
            assert!(launch.runtime_args.is_empty());
            assert!(
                launch.env.is_empty(),
                "no dangling manifest may shadow the tool discovery"
            );
            assert_eq!(launch.cwd_rel, "");
            assert_eq!(launch.version, None);
        }
    }

    #[test]
    fn js_tools_leave_a_runfiles_tree_beside_the_program_alone() {
        let dir = tempfile::Builder::new()
            .prefix("dx-launch-tree-")
            .tempdir_in(std::env::temp_dir())
            .expect("scratch");
        std::fs::create_dir_all(dir.path().join("eslint.sh.runfiles/_main/pkg"))
            .expect("staged tree");
        let launch = ResolvedToolLaunch::resolve("eslint", &dir.path().join("eslint.sh"))
            .expect("js launch resolves")
            .expect("eslint is migrated");
        assert!(
            launch.env.is_empty(),
            "tree execution keeps its own discovery"
        );
    }

    #[test]
    fn js_tools_take_the_manifest_on_disk_over_the_suffixed_name() {
        let dir = tempfile::Builder::new()
            .prefix("dx-launch-manifest-")
            .tempdir_in(std::env::temp_dir())
            .expect("scratch");
        let bare = dir.path().join("eslint");
        let manifest = format!("{}.runfiles_manifest", bare.display());
        std::fs::write(&manifest, "_main/pkg/tool.txt out/tool\n").expect("staged manifest");
        let launch = ResolvedToolLaunch::resolve("eslint", &dir.path().join("eslint.sh"))
            .expect("js launch resolves")
            .expect("eslint is migrated");
        assert_eq!(
            launch.env,
            vec![(os("RUNFILES_MANIFEST_FILE"), os(&manifest))],
            "the manifest beside the program wins over the suffixed name"
        );
    }

    #[test]
    fn missing_program_fails_before_any_spawn() {
        assert_eq!(
            ResolvedToolLaunch::resolve("eslint", Path::new("")),
            Err(LaunchError::MissingProgram {
                tool: "eslint".to_owned()
            })
        );
        assert_eq!(
            LaunchError::MissingProgram {
                tool: "prettier".to_owned()
            }
            .to_string(),
            "prettier: missing program: the aspect declared no binary"
        );
    }

    #[test]
    fn argv_keeps_program_entry_and_tool_args_byte_identical() {
        let launch = ResolvedToolLaunch {
            tool_id: "eslint".to_owned(),
            program: os("/out/my dir/eslint.sh"),
            entry: Some(os("entry with spaces.js")),
            runtime_args: vec![os("--runtime-flag"), os("a=b;c,d")],
            env: Vec::new(),
            cwd_rel: String::new(),
            version: None,
        };
        let tool_args = vec![os("-f"), os("json"), "caf\u{e9}.js".into(), os("--fix")];
        assert_eq!(
            launch.argv(&tool_args),
            vec![
                os("/out/my dir/eslint.sh"),
                os("--runtime-flag"),
                os("a=b;c,d"),
                os("entry with spaces.js"),
                os("-f"),
                os("json"),
                "caf\u{e9}.js".into(),
                os("--fix"),
            ]
        );
    }

    #[test]
    fn argv_without_entry_matches_the_legacy_binary_first_layout() {
        let launch = ResolvedToolLaunch::resolve("prettier", Path::new("/fake/bin/tool"))
            .expect("resolves")
            .expect("migrated");
        let tool_args = vec![os("--check"), os("a.js")];
        let argv = launch.argv(&tool_args);
        assert_eq!(argv[0], os("/fake/bin/tool"));
        assert_eq!(&argv[1..], &tool_args);
    }
}
