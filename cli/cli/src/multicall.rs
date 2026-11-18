//! `dx` runs a bundled tool when started under that tool's name, so the prebuilt hub can ship one
//! binary and expose it as `quality_runner`, `quality_markdown`, `quality_notebook`, `lsp_check`,
//! `collect_native`, `env_shard_writer` and `codegen_shard_writer`. Rule helpers that need no alias
//! run as `dx <tool> [ARG]...`, so a host without symlinks keeps one copy of dx.

use std::ffi::OsStr;
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tool {
    Runner,
    Markdown,
    Notebook,
    LspCheck,
    CollectNative,
    EnvShardWriter,
    CodegenShardWriter,
    Capture,
    ReleaseTree,
    ApiSpecPreflight,
    OapiCodegen,
}

/// The tool `dx` was started as, judged by the file stem of argv[0] so `.exe` does not matter.
#[must_use]
pub fn tool_for(argv0: &OsStr) -> Option<Tool> {
    match Path::new(argv0).file_stem()?.to_str()? {
        "quality_runner" => Some(Tool::Runner),
        "quality_markdown" => Some(Tool::Markdown),
        "quality_notebook" => Some(Tool::Notebook),
        "lsp_check" => Some(Tool::LspCheck),
        "collect_native" => Some(Tool::CollectNative),
        "env_shard_writer" => Some(Tool::EnvShardWriter),
        "codegen_shard_writer" => Some(Tool::CodegenShardWriter),
        _ => None,
    }
}

/// The helper named by dx's first argument.
#[must_use]
pub fn subcommand_for(arg1: &OsStr) -> Option<Tool> {
    match arg1.to_str()? {
        "capture" => Some(Tool::Capture),
        "release_tree" => Some(Tool::ReleaseTree),
        "api_spec_preflight" => Some(Tool::ApiSpecPreflight),
        "oapi_codegen" => Some(Tool::OapiCodegen),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_names_pick_their_tool() {
        assert_eq!(tool_for(OsStr::new("quality_runner")), Some(Tool::Runner));
        assert_eq!(
            tool_for(OsStr::new("quality_markdown")),
            Some(Tool::Markdown)
        );
        assert_eq!(
            tool_for(OsStr::new("quality_notebook")),
            Some(Tool::Notebook)
        );
    }

    #[test]
    fn rule_helpers_pick_their_tool() {
        for (name, tool) in [
            ("lsp_check", Tool::LspCheck),
            ("collect_native", Tool::CollectNative),
            ("env_shard_writer.exe", Tool::EnvShardWriter),
            ("codegen_shard_writer", Tool::CodegenShardWriter),
        ] {
            assert_eq!(tool_for(OsStr::new(name)), Some(tool), "{name}");
        }
    }

    #[test]
    fn exe_suffix_and_directories_are_ignored() {
        assert_eq!(
            tool_for(OsStr::new("quality_runner.exe")),
            Some(Tool::Runner)
        );
        assert_eq!(
            tool_for(OsStr::new(
                "bazel-out/k8-opt-exec/bin/external/dx_prebuilt/linux_x86_64/quality_notebook"
            )),
            Some(Tool::Notebook)
        );
    }

    #[test]
    fn other_names_run_dx() {
        for name in [
            "dx",
            "dx.exe",
            "/usr/local/bin/dx",
            "",
            "quality_runner_lib",
            "quality",
        ] {
            assert_eq!(tool_for(OsStr::new(name)), None, "{name}");
        }
    }

    #[test]
    fn subcommands_pick_their_tool() {
        for (name, tool) in [
            ("capture", Tool::Capture),
            ("release_tree", Tool::ReleaseTree),
            ("api_spec_preflight", Tool::ApiSpecPreflight),
            ("oapi_codegen", Tool::OapiCodegen),
        ] {
            assert_eq!(subcommand_for(OsStr::new(name)), Some(tool), "{name}");
        }
    }

    #[test]
    fn subcommands_have_no_alias_and_aliases_are_not_subcommands() {
        for name in [
            "capture",
            "release_tree",
            "capture.exe",
            "api_spec_preflight",
            "oapi_codegen",
        ] {
            assert_eq!(tool_for(OsStr::new(name)), None, "{name}");
        }
        for name in ["quality_runner", "capture.exe", "test", "check", "", "--help"] {
            assert_eq!(subcommand_for(OsStr::new(name)), None, "{name}");
        }
    }
}
