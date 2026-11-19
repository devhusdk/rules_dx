//! `dx` runs a bundled tool when started under that tool's name, so the prebuilt hub can ship one
//! binary and expose it as `quality_runner`, `quality_markdown`, `quality_notebook`, `lsp_check`,
//! `collect_native`, `env_shard_writer`, `codegen_shard_writer` and `deploy_tool`. Rule helpers
//! that need no alias run as `dx <tool> [ARG]...`, so a host without symlinks keeps one copy of dx.

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
    DeployTool,
    Capture,
    ReleaseTree,
    ApiSpecPreflight,
    OapiCodegen,
    Archiver,
    Hasher,
    SizeChecker,
    CloudflareVerify,
    NpmPacker,
    ReleaseTool,
    CapnpcRustDx,
    AvroExtract,
    DExtract,
    FortranExtract,
    LuaExtract,
    PerlExtract,
    ZigExtract,
    AndroidOps,
    QtRustBridge,
    BeamExtract,
    CcExtract,
    ComponentExtract,
    DartExtract,
    DotnetExtract,
    GoExtract,
    HaskellExtract,
    JuliaExtract,
    OcamlExtract,
    PhpExtract,
    PowershellExtract,
    ProtobufExtract,
    PythonExtract,
    RubyExtract,
    RustExtract,
    SwiftExtract,
    TypescriptExtract,
    JvmConvert,
    SiteAggregate,
    SiteBuild,
    SiteCheck,
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
        "deploy_tool" => Some(Tool::DeployTool),
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
        "archiver" => Some(Tool::Archiver),
        "hasher" => Some(Tool::Hasher),
        "size_checker" => Some(Tool::SizeChecker),
        "cloudflare_verify" => Some(Tool::CloudflareVerify),
        "npm_packer" => Some(Tool::NpmPacker),
        "release_tool" => Some(Tool::ReleaseTool),
        "capnpc_rust_dx" => Some(Tool::CapnpcRustDx),
        "avro_extract" => Some(Tool::AvroExtract),
        "d_extract" => Some(Tool::DExtract),
        "fortran_extract" => Some(Tool::FortranExtract),
        "lua_extract" => Some(Tool::LuaExtract),
        "perl_extract" => Some(Tool::PerlExtract),
        "zig_extract" => Some(Tool::ZigExtract),
        "android_ops" => Some(Tool::AndroidOps),
        "qt_rust_bridge" => Some(Tool::QtRustBridge),
        "beam_extract" => Some(Tool::BeamExtract),
        "cc_extract" => Some(Tool::CcExtract),
        "component_extract" => Some(Tool::ComponentExtract),
        "dart_extract" => Some(Tool::DartExtract),
        "dotnet_extract" => Some(Tool::DotnetExtract),
        "go_extract" => Some(Tool::GoExtract),
        "haskell_extract" => Some(Tool::HaskellExtract),
        "julia_extract" => Some(Tool::JuliaExtract),
        "ocaml_extract" => Some(Tool::OcamlExtract),
        "php_extract" => Some(Tool::PhpExtract),
        "powershell_extract" => Some(Tool::PowershellExtract),
        "protobuf_extract" => Some(Tool::ProtobufExtract),
        "python_extract" => Some(Tool::PythonExtract),
        "ruby_extract" => Some(Tool::RubyExtract),
        "rust_extract" => Some(Tool::RustExtract),
        "swift_extract" => Some(Tool::SwiftExtract),
        "typescript_extract" => Some(Tool::TypescriptExtract),
        "jvm_convert" => Some(Tool::JvmConvert),
        "site_aggregate" => Some(Tool::SiteAggregate),
        "site_build" => Some(Tool::SiteBuild),
        "site_check" => Some(Tool::SiteCheck),
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
            ("deploy_tool.exe", Tool::DeployTool),
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
            ("archiver", Tool::Archiver),
            ("hasher", Tool::Hasher),
            ("size_checker", Tool::SizeChecker),
            ("cloudflare_verify", Tool::CloudflareVerify),
            ("npm_packer", Tool::NpmPacker),
            ("release_tool", Tool::ReleaseTool),
            ("capnpc_rust_dx", Tool::CapnpcRustDx),
            ("avro_extract", Tool::AvroExtract),
            ("d_extract", Tool::DExtract),
            ("fortran_extract", Tool::FortranExtract),
            ("lua_extract", Tool::LuaExtract),
            ("perl_extract", Tool::PerlExtract),
            ("zig_extract", Tool::ZigExtract),
            ("android_ops", Tool::AndroidOps),
            ("qt_rust_bridge", Tool::QtRustBridge),
            ("beam_extract", Tool::BeamExtract),
            ("cc_extract", Tool::CcExtract),
            ("component_extract", Tool::ComponentExtract),
            ("dart_extract", Tool::DartExtract),
            ("dotnet_extract", Tool::DotnetExtract),
            ("go_extract", Tool::GoExtract),
            ("haskell_extract", Tool::HaskellExtract),
            ("julia_extract", Tool::JuliaExtract),
            ("ocaml_extract", Tool::OcamlExtract),
            ("php_extract", Tool::PhpExtract),
            ("powershell_extract", Tool::PowershellExtract),
            ("protobuf_extract", Tool::ProtobufExtract),
            ("python_extract", Tool::PythonExtract),
            ("ruby_extract", Tool::RubyExtract),
            ("rust_extract", Tool::RustExtract),
            ("swift_extract", Tool::SwiftExtract),
            ("typescript_extract", Tool::TypescriptExtract),
            ("jvm_convert", Tool::JvmConvert),
            ("site_aggregate", Tool::SiteAggregate),
            ("site_build", Tool::SiteBuild),
            ("site_check", Tool::SiteCheck),
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
            "archiver",
            "hasher",
            "release_tool",
            "capnpc_rust_dx",
            "avro_extract",
            "d_extract",
            "fortran_extract",
            "lua_extract",
            "perl_extract",
            "zig_extract",
            "android_ops",
            "qt_rust_bridge",
            "beam_extract",
            "cc_extract",
            "component_extract",
            "dart_extract",
            "dotnet_extract",
            "go_extract",
            "haskell_extract",
            "julia_extract",
            "ocaml_extract",
            "php_extract",
            "powershell_extract",
            "protobuf_extract",
            "python_extract",
            "ruby_extract",
            "rust_extract",
            "swift_extract",
            "typescript_extract",
            "jvm_convert",
            "site_aggregate",
            "site_build",
            "site_check",
        ] {
            assert_eq!(tool_for(OsStr::new(name)), None, "{name}");
        }
        for name in [
            "quality_runner",
            "deploy_tool",
            "capture.exe",
            "test",
            "check",
            "",
            "--help",
        ] {
            assert_eq!(subcommand_for(OsStr::new(name)), None, "{name}");
        }
    }
}
