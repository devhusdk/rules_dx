"""Target-scoped real capability aspects over real adapters."""

load("@rules_py//py:defs.bzl", _PyInfo = "PyInfo")
load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_dotnet//dotnet/private:providers.bzl", "DotnetAssemblyRuntimeInfo")
load("@rules_go//go:def.bzl", "GoArchive")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_julia//julia:julia_info.bzl", "JuliaInfo")
load("@rules_powershell//powershell:defs.bzl", _PwshInfo = "PwshInfo")
load("@rules_proto//proto:defs.bzl", "ProtoInfo")
load("@rules_ruby//ruby/private:providers.bzl", "RubyFilesInfo")
load("@rules_rust//rust:defs.bzl", "rust_clippy_aspect", _rust_common = "rust_common")
load("@rules_scala//scala:semanticdb_provider.bzl", "SemanticdbInfo")
load("//avro/rules:defs.bzl", "AvroInfo")
load("//capnproto/rules:defs.bzl", "CapnpInfo")
load("//dart/rules:providers.bzl", "DartAnalyzableInfo", "DartInfo")
load("//erlang/rules:providers.bzl", "BeamInfo", "DialyzerToolInfo")
load("//flatbuffers/rules:defs.bzl", "FlatBuffersInfo")
load("//fsharp/rules:symbols.bzl", "fsharp_compile_defines")
load("//ocaml/rules:providers.bzl", "OCamlCmtInfo")
load("//php/rules:php.bzl", "staged_path")
load("//quality:clang_syntax.bzl", "ClangSyntaxInfo", "clang_syntax_aspect")
load("//quality:compile_check.bzl", "COMPILE_CHECK_TOOLS", "CompileCheckInfo", "compile_check_aspect")
load("//quality:dart_context.bzl", "DART_ANALYZE", "DART_FORMAT", "dart_external_root", "dart_merge_packages", "dart_package_entries", "dart_pubspec_stubs")
load("//quality:fsc.bzl", "FscInfo", "FscUnsupportedInfo", "fsc_aspect")
load("//quality:go_analysis.bzl", "GO_TOOLCHAIN_TYPE", "GoAnalysisInfo", "go_analysis_aspect", "go_lang_version")
load("//quality:jvm_check.bzl", "JvmCheckInfo", "jvm_check_aspect")
load("//quality:families/core.bzl", _real_format_aspect = "real_format_aspect", _real_lint_aspect = "real_lint_aspect", _real_typecheck_aspect = "real_typecheck_aspect", _CORE_HOOKS = "HOOKS", _CORE_SHARD_SPECS = "SHARD_SPECS", _CORE_TOOLS = "TOOLS")
load("//quality:families/ansible.bzl", _real_ansible_lint_aspect = "real_ansible_lint_aspect", _ANSIBLE_SHARD_SPECS = "SHARD_SPECS", _ANSIBLE_TOOLS = "TOOLS")
load("//quality:families/cloudformation.bzl", _real_cloudformation_lint_aspect = "real_cloudformation_lint_aspect", _CLOUDFORMATION_SHARD_SPECS = "SHARD_SPECS", _CLOUDFORMATION_TOOLS = "TOOLS")
load("//quality:families/cmake.bzl", _real_cmake_format_aspect = "real_cmake_format_aspect", _real_cmake_lint_aspect = "real_cmake_lint_aspect", _CMAKE_SHARD_SPECS = "SHARD_SPECS", _CMAKE_TOOLS = "TOOLS")
load("//quality:families/dockerfile.bzl", _real_dockerfile_lint_aspect = "real_dockerfile_lint_aspect", _DOCKERFILE_SHARD_SPECS = "SHARD_SPECS", _DOCKERFILE_TOOLS = "TOOLS")
load("//quality:families/html_template.bzl", _real_html_template_format_aspect = "real_html_template_format_aspect", _real_html_template_lint_aspect = "real_html_template_lint_aspect", _HTML_TEMPLATE_SHARD_SPECS = "SHARD_SPECS", _HTML_TEMPLATE_TOOLS = "TOOLS")
load("//quality:families/latex.bzl", _real_latex_format_aspect = "real_latex_format_aspect", _real_latex_lint_aspect = "real_latex_lint_aspect", _LATEX_SHARD_SPECS = "SHARD_SPECS", _LATEX_TOOLS = "TOOLS")
load("//quality:families/make.bzl", _real_make_lint_aspect = "real_make_lint_aspect", _real_make_format_aspect = "real_make_format_aspect", _MAKE_SHARD_SPECS = "SHARD_SPECS", _MAKE_TOOLS = "TOOLS")
load("//quality:families/nix.bzl", _real_nix_format_aspect = "real_nix_format_aspect", _real_nix_lint_aspect = "real_nix_lint_aspect", _NIX_SHARD_SPECS = "SHARD_SPECS", _NIX_TOOLS = "TOOLS")
load("//quality:families/perl.bzl", _real_perl_format_aspect = "real_perl_format_aspect", _real_perl_lint_aspect = "real_perl_lint_aspect", _PERL_SHARD_SPECS = "SHARD_SPECS", _PERL_TOOLS = "TOOLS")
load("//quality:families/shell.bzl", _real_shell_lint_aspect = "real_shell_lint_aspect", _real_shell_format_aspect = "real_shell_format_aspect", _SHELL_SHARD_SPECS = "SHARD_SPECS", _SHELL_TOOLS = "TOOLS")
load("//quality:families/swift.bzl", _real_swift_format_aspect = "real_swift_format_aspect", _real_swift_lint_aspect = "real_swift_lint_aspect", _SWIFT_SHARD_SPECS = "SHARD_SPECS", _SWIFT_TOOLS = "TOOLS")
load("//quality:families/text.bzl", _real_text_lint_aspect = "real_text_lint_aspect", _TEXT_SHARD_SPECS = "SHARD_SPECS", _TEXT_TOOLS = "TOOLS")
load("//quality:families/typst.bzl", _real_typst_format_aspect = "real_typst_format_aspect", _real_typst_lint_aspect = "real_typst_lint_aspect", _TYPST_SHARD_SPECS = "SHARD_SPECS", _TYPST_TOOLS = "TOOLS")
load("//quality:families/xml.bzl", _real_xml_lint_aspect = "real_xml_lint_aspect", _XML_SHARD_SPECS = "SHARD_SPECS", _XML_TOOLS = "TOOLS")
load("//quality:families/yaml.bzl", _real_yaml_format_aspect = "real_yaml_format_aspect", _real_yaml_lint_aspect = "real_yaml_lint_aspect", _YAML_SHARD_SPECS = "SHARD_SPECS", _YAML_TOOLS = "TOOLS")
load("//quality:families/cue.bzl", _real_cue_format_aspect = "real_cue_format_aspect", _real_cue_typecheck_aspect = "real_cue_typecheck_aspect", _CUE_HOOKS = "HOOKS", _CUE_SHARD_SPECS = "SHARD_SPECS", _CUE_TOOLS = "TOOLS")
load("//quality:families/pkl.bzl", _real_pkl_format_aspect = "real_pkl_format_aspect", _real_pkl_typecheck_aspect = "real_pkl_typecheck_aspect", _PKL_HOOKS = "HOOKS", _PKL_SHARD_SPECS = "SHARD_SPECS", _PKL_TOOLS = "TOOLS")
load("//quality:families/jsonnet.bzl", _real_jsonnet_format_aspect = "real_jsonnet_format_aspect", _real_jsonnet_lint_aspect = "real_jsonnet_lint_aspect", _JSONNET_HOOKS = "HOOKS", _JSONNET_SHARD_SPECS = "SHARD_SPECS", _JSONNET_TOOLS = "TOOLS")
load("//quality:families/rego.bzl", _real_rego_format_aspect = "real_rego_format_aspect", _real_rego_lint_aspect = "real_rego_lint_aspect", _real_rego_typecheck_aspect = "real_rego_typecheck_aspect", _REGO_HOOKS = "HOOKS", _REGO_SHARD_SPECS = "SHARD_SPECS", _REGO_TOOLS = "TOOLS")
load("//quality:families/bicep.bzl", _real_bicep_format_aspect = "real_bicep_format_aspect", _real_bicep_lint_aspect = "real_bicep_lint_aspect", _BICEP_HOOKS = "HOOKS", _BICEP_SHARD_SPECS = "SHARD_SPECS", _BICEP_TOOLS = "TOOLS")
load("//quality:families/terraform.bzl", _real_terraform_format_aspect = "real_terraform_format_aspect", _real_terraform_lint_aspect = "real_terraform_lint_aspect", _real_terraform_typecheck_aspect = "real_terraform_typecheck_aspect", _TERRAFORM_HOOKS = "HOOKS", _TERRAFORM_SHARD_SPECS = "SHARD_SPECS", _TERRAFORM_TOOLS = "TOOLS")
load("//quality:families/lua.bzl", _real_lua_format_aspect = "real_lua_format_aspect", _real_lua_lint_aspect = "real_lua_lint_aspect", _real_lua_typecheck_aspect = "real_lua_typecheck_aspect", _LUA_HOOKS = "HOOKS", _LUA_SHARD_SPECS = "SHARD_SPECS", _LUA_TOOLS = "TOOLS")
load("//quality:families/php.bzl", _real_php_lint_aspect = "real_php_lint_aspect", _real_php_typecheck_aspect = "real_php_typecheck_aspect", _PHP_HOOKS = "HOOKS", _PHP_SHARD_SPECS = "SHARD_SPECS", _PHP_TOOLS = "TOOLS")
load("//quality:families/kubernetes.bzl", _real_kubernetes_lint_aspect = "real_kubernetes_lint_aspect", _KUBERNETES_HOOKS = "HOOKS", _KUBERNETES_SHARD_SPECS = "SHARD_SPECS", _KUBERNETES_TOOLS = "TOOLS")
load("//quality:families/sql.bzl", _real_sql_format_aspect = "real_sql_format_aspect", _real_sql_lint_aspect = "real_sql_lint_aspect", _SQL_HOOKS = "HOOKS", _SQL_SHARD_SPECS = "SHARD_SPECS", _SQL_TOOLS = "TOOLS")
load("//quality:family_hooks.bzl", _default_files_hook = "default_files_hook", _stage = "stage", _without = "without")
load("//quality:native_config.bzl", "DxDartConfigInfo")
load("//quality:nvcc_syntax.bzl", "NvccSyntaxInfo", "nvcc_syntax_aspect")
load("//quality:pipeline.bzl", "drop_pipeline_tool", "generated_source_paths", "prune_tool_generated_sources", "real_request_tool_env")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect", "real_upstream_direct_sources", _RealShardProbeInfo = "RealShardProbeInfo", _real_policy_error = "real_policy_error", _real_quality_resource_set = "real_quality_resource_set", _real_shard_result_stems = "real_shard_result_stems", _real_tool_table_error = "real_tool_table_error", _real_tool_table_shape_error = "real_tool_table_shape_error", _real_wired_capabilities = "real_wired_capabilities", _real_wired_tools = "real_wired_tools")
load("//quality:roslyn.bzl", "RoslynInfo", "roslyn_aspect")
load("//quality:sources.bzl", "QualityToolsInfo")
load("//quality:tsc.bzl", "TscInfo", "tsc_aspect")
load("//rust/rules:edition.bzl", "RUST_EDITION")
load("//rust/toolchains:bindings.bzl", "RUST_TOOLCHAIN_TYPE", "rust_toolchain_rustc", "rust_toolchain_toolchains", "rust_toolchain_tools")
load("//sql/rules:providers.bzl", "SqlDialectInfo")
load("//thrift/rules:defs.bzl", "ThriftInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("//tools/docs/rust:defs.bzl", "rust_doc_inputs_aspect", "rust_shard_action")
load("//tools/docs/site:render.bzl", "DocsApiInfo", "DocsSiteInfo")

_REAL_TOOL_TABLE = {
    "buf": {
        "attr": attr.label(default = "@dx_tools//:buf", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format", "lint"],
        "env": [["PATH", "/usr/bin:/bin"]],
        "kind": "file",
        "resources": "light",
        "shard": "protobuf",
    },
    "capnp": {
        "attr": attr.label(default = "//config:tool_capnp", allow_files = True, cfg = "exec", executable = True),
        "capabilities": ["lint", "typecheck"],
        "fallback": " to a capnp binary",
        "kind": "setting",
        "resources": "light",
        "shard": "capnp",
    },
    "checkstyle": {
        "attr": attr.label(default = "//quality/tools/jvm:checkstyle", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "jvm",
    },
    "clang_format": {
        "attr": attr.label(default = "@llvm//tools:clang-format", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "file",
        "resources": "light",
        "shard": "cc",
    },
    "clang_syntax": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "cc",
    },
    "clang_tidy": {
        "capabilities": ["lint"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "cc",
    },
    "cppcheck": {
        "attr": attr.label(default = "//quality/tools/cc:cppcheck", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "extra": "runfiles",
        "kind": "executable",
        "resources": "heavy",
        "run_tool": False,
        "shard": "cc",
    },
    "clippy": {
        "capabilities": ["lint"],
        "kind": "toolchain",
        "resources": "heavy",
        "shard": "rust",
        "toolchain_file": "clippy_driver",
    },
    "credo": {
        "attr": attr.label(default = "//config:tool_credo", allow_files = True, cfg = "exec"),
        "capabilities": ["lint"],
        "fallback": "=@rules_dx//elixir/tools:credo",
        "kind": "setting",
        "resources": "medium",
        "shard": "elixir",
    },
    "csharpier": {
        "attr": attr.label(default = "//quality/tools/dotnet:csharpier", cfg = "exec", executable = True),
        "capabilities": ["format"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "dotnet",
    },
    "dialyzer": {
        "attr": attr.label(default = "//config:tool_dialyzer", allow_files = True, cfg = "exec"),
        "capabilities": ["typecheck"],
        "fallback": "=@rules_dx//erlang/tools:dialyzer",
        "kind": "setting",
        "resources": "heavy",
        "shard": "erlang",
    },
    "jet": {
        "attr": attr.label(default = "//config:tool_jet", allow_files = True, cfg = "exec"),
        "capabilities": ["typecheck"],
        "fallback": "=@rules_dx//julia/tools/jet:jet",
        "kind": "setting",
        "resources": "heavy",
        "shard": "julia",
    },
    "sorbet": {
        "attr": attr.label(default = "//config:tool_sorbet", allow_files = True, cfg = "exec"),
        "capabilities": ["typecheck"],
        "fallback": "=@rules_dx//ruby/tools:sorbet",
        "kind": "setting",
        "resources": "heavy",
        "shard": "ruby",
    },
    "elvis": {
        "attr": attr.label(default = "//config:tool_elvis", allow_files = True, cfg = "exec"),
        "capabilities": ["lint"],
        "fallback": "=@rules_dx//erlang/tools:elvis",
        "kind": "setting",
        "resources": "medium",
        "shard": "erlang",
    },
    "erlfmt": {
        "attr": attr.label(default = "//config:tool_erlfmt", allow_files = True, cfg = "exec"),
        "capabilities": ["format"],
        "fallback": "=@rules_dx//erlang/tools:erlfmt",
        "kind": "setting",
        "resources": "medium",
        "shard": "erlang",
    },
    "dart": {
        "attr": attr.label(default = "//config:tool_dart", allow_files = True, cfg = "exec"),
        "capabilities": ["format", "lint", "typecheck"],
        "kind": "setting",
        "resources": "medium",
        "shard": "dart",
        "target_tool": True,
    },
    "dfmt": {
        "attr": attr.label(default = "//config:tool_dfmt", allow_files = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "setting",
        "resources": "light",
        "shard": "d",
        "target_tool": True,
    },
    "dscanner": {
        "attr": attr.label(default = "//config:tool_dscanner", allow_files = True, cfg = "exec"),
        "capabilities": ["lint"],
        "kind": "setting",
        "resources": "medium",
        "shard": "d",
        "target_tool": True,
    },
    "eslint": {
        "attr": attr.label(default = "//quality/tools/javascript/bin:eslint", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "js",
    },
    "fantomas": {
        "attr": attr.label(default = "//quality/tools/dotnet:fantomas", cfg = "exec", executable = True),
        "capabilities": ["format"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "dotnet",
    },
    "flake8": {
        "attr": attr.label(default = "//quality/tools/python:flake8", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "medium",
        "runfiles_env": True,
        "shard": "py",
    },
    "flang": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "fortran",
    },
    "ghc": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "haskell",
    },
    "ldc": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "d",
    },
    "flatc": {
        "attr": attr.label(default = "//config:tool_flatc", allow_files = True, cfg = "exec", executable = True),
        "capabilities": ["lint", "typecheck"],
        "fallback": " to a flatc binary",
        "kind": "setting",
        "resources": "light",
        "shard": "flatbuffers",
    },
    "avro_tools": {
        "attr": attr.label(default = "//config:tool_avro", allow_files = True, cfg = "exec", executable = True),
        "capabilities": ["lint", "typecheck"],
        "fallback": " to an avro-tools binary",
        "kind": "setting",
        "resources": "light",
        "shard": "avro",
    },
    "thrift": {
        "attr": attr.label(default = "//config:tool_thrift", allow_files = True, cfg = "exec", executable = True),
        "capabilities": ["lint", "typecheck"],
        "fallback": " to a thrift binary",
        "kind": "setting",
        "resources": "light",
        "shard": "thrift",
    },
    "fsharplint": {
        "attr": attr.label(default = "//quality/tools/dotnet/fsharplint", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "dotnet",
    },
    "gofumpt": {
        "attr": attr.label(default = "//config:tool_gofumpt", allow_files = True, cfg = "exec", executable = True),
        "capabilities": ["format"],
        "fallback": " to a gofumpt binary",
        "kind": "setting",
        "resources": "light",
        "shard": "go",
    },
    "modfmt": {
        "attr": attr.label(default = "//config:tool_modfmt", allow_files = True, cfg = "exec", executable = True),
        "capabilities": ["format"],
        "fallback": " to a modfmt binary",
        "kind": "setting",
        "resources": "light",
        "shard": "go",
    },
    "google_java_format": {
        "attr": attr.label(default = "//quality/tools/jvm:google_java_format", cfg = "exec", executable = True),
        "capabilities": ["format"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "jvm",
    },
    "gplint": {
        "attr": attr.label(default = "//quality/tools/javascript/bin:gplint", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "js",
    },
    "graphql_eslint": {
        "attr": attr.label(default = "//quality/tools/javascript/bin:graphql_eslint", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "js",
    },
    "ffmt": {
        "attr": attr.label(default = "@dx_tools//:ffmt", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "file",
        "resources": "light",
        "shard": "fortran",
    },
    "fortitude": {
        "attr": attr.label(default = "@dx_tools//:fortitude", allow_single_file = True, cfg = "exec"),
        "capabilities": ["lint"],
        "kind": "file",
        "resources": "light",
        "shard": "fortran",
    },
    "ktfmt": {
        "attr": attr.label(default = "//quality/tools/jvm:ktfmt", cfg = "exec", executable = True),
        "capabilities": ["format"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "jvm",
    },
    "ktlint": {
        "attr": attr.label(default = "//quality/tools/jvm:ktlint", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "jvm",
    },
    "mix_format": {
        "attr": attr.label(default = "//config:tool_mix_format", allow_files = True, cfg = "exec"),
        "capabilities": ["format"],
        "fallback": "=@rules_dx//elixir/tools:mix_format",
        "kind": "setting",
        "resources": "medium",
        "shard": "elixir",
    },
    "hlint": {
        "attr": attr.label(default = "//config:tool_hlint", allow_files = True, cfg = "exec"),
        "capabilities": ["lint"],
        "fallback": "=<label of an hlint executable>",
        "kind": "file",
        "resources": "medium",
        "shard": "haskell",
    },
    "nvcc_syntax": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "cc",
    },
    "ormolu": {
        "attr": attr.label(default = "//config:tool_ormolu", allow_files = True, cfg = "exec"),
        "capabilities": ["format"],
        "fallback": "=<label of an ormolu executable>",
        "kind": "file",
        "resources": "medium",
        "shard": "haskell",
    },
    "pmd": {
        "attr": attr.label(default = "//quality/tools/jvm:pmd", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "jvm",
    },
    "psscriptanalyzer": {
        "attr": attr.label(default = "//quality/tools/powershell:psscriptanalyzer", cfg = "exec", executable = True),
        "capabilities": ["format", "lint"],
        "env": [["DOTNET_CLI_TELEMETRY_OPTOUT", "1"], ["POWERSHELL_TELEMETRY_OPTOUT", "1"], ["POWERSHELL_UPDATECHECK", "Off"]],
        "kind": "executable",
        "resources": "heavy",
        "shard": "powershell",
    },
    "prettier": {
        "attr": attr.label(default = "//quality/tools/javascript/bin:prettier", cfg = "exec", executable = True),
        "capabilities": ["format"],
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "js",
    },
    "pydoclint": {
        "attr": attr.label(default = "//quality/tools/python:pydoclint", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "medium",
        "runfiles_env": True,
        "shard": "py",
    },
    "pylint": {
        "attr": attr.label(default = "//quality/tools/python:pylint", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "medium",
        "runfiles_env": True,
        "shard": "py",
    },
    "qmlformat": {
        "attr": attr.label(default = "//config:tool_qmlformat", allow_files = True, cfg = "exec"),
        "capabilities": ["format"],
        "fallback": "=@dx_qt//:qmlformat",
        "kind": "setting",
        "resources": "light",
        "shard": "cc",
    },
    "qmllint": {
        "attr": attr.label(default = "//config:tool_qmllint", allow_files = True, cfg = "exec"),
        "capabilities": ["lint", "typecheck"],
        "fallback": "=@dx_qt//:qmllint",
        "kind": "setting",
        "resources": "medium",
        "shard": "cc",
    },
    "redocly": {
        "attr": attr.label(default = "//quality/tools/javascript/bin:redocly", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "medium",
        "shard": "js",
    },
    "remark": {
        "attr": attr.label(default = "//quality/tools/javascript/bin:remark", cfg = "exec", executable = True),
        "capabilities": ["format"],
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "js",
    },
    "rubocop": {
        "attr": attr.label(default = "//quality/tools/ruby:rubocop", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "medium",
        "shard": "ruby",
    },
    "csc": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "dotnet",
    },
    "fsc": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "dotnet",
    },
    "roslyn": {
        "capabilities": ["lint"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "dotnet",
    },
    "errcheck": {
        "capabilities": ["lint"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "go",
    },
    "gotype": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "go",
    },
    "govet": {
        "capabilities": ["lint"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "go",
    },
    "javac": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "jvm",
    },
    "kotlinc": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "jvm",
    },
    "tsc": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "js",
    },
    "ruff": {
        "attr": attr.label(default = "//config:tool_ruff", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format", "lint"],
        "kind": "file",
        "resources": "light",
        "shard": "py",
    },
    "rustc": {
        "capabilities": ["typecheck"],
        "kind": "toolchain",
        "resources": "heavy",
        "shard": "rust",
        "toolchain_file": "rustc",
    },
    "rustfmt": {
        "capabilities": ["format"],
        "kind": "toolchain",
        "resources": "light",
        "shard": "rust",
        "toolchain_file": "rustfmt",
    },
    "scalac": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "jvm",
    },
    "scalafix": {
        "attr": attr.label(default = "//quality/tools/jvm:scalafix", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "scala",
    },
    "scalafmt": {
        "attr": attr.label(default = "@dx_tools//:scalafmt", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "file",
        "resources": "heavy",
        "shard": "scala",
    },
    "spotbugs": {
        "attr": attr.label(default = "//quality/tools/jvm:spotbugs", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "jvm",
    },
    "staticcheck": {
        "capabilities": ["lint"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "go",
    },
    "stylelint": {
        "attr": attr.label(default = "//quality/tools/javascript/bin:stylelint", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "env": [["JS_BINARY__NO_CD_BINDIR", "1"]],
        "extra": "files",
        "kind": "executable",
        "resources": "heavy",
        "shard": "js",
    },
    "standardrb": {
        "attr": attr.label(default = "//quality/tools/ruby:standardrb", cfg = "exec", executable = True),
        "capabilities": ["format"],
        "kind": "executable",
        "resources": "medium",
        "shard": "ruby",
    },
    "topiary": {
        "attr": attr.label(default = "//quality/tools/rust:topiary", cfg = "exec", executable = True),
        "capabilities": ["format"],
        "kind": "executable",
        "resources": "light",
        "shard": "ocaml",
    },
    "ty": {
        "attr": attr.label(default = "@dx_tools//:ty", allow_single_file = True, cfg = "exec"),
        "capabilities": ["typecheck"],
        "kind": "file",
        "resources": "medium",
        "shard": "py",
    },
    "ocaml": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "ocaml",
    },
    "zanuda": {
        "attr": attr.label(default = "//quality/tools/ocaml:zanuda_tool", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "light",
        "shard": "ocaml",
    },
    "julialint": {
        "attr": attr.label(default = "//config:tool_julialint", allow_files = True, cfg = "exec"),
        "capabilities": ["lint"],
        "fallback": "=@rules_dx//julia/tools/lint:julialint",
        "kind": "setting",
        "resources": "medium",
        "shard": "julia",
    },
    "julia_format": {
        "attr": attr.label(default = "//config:tool_julia_format", allow_files = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "setting",
        "resources": "medium",
        "shard": "julia",
        "target_tool": True,
    },
    "zig": {
        "capabilities": ["typecheck"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "zig",
    },
    "zig_ast_check": {
        "attr": attr.label(default = "//config:tool_zig_ast_check", allow_files = True, cfg = "exec"),
        "capabilities": ["lint"],
        "kind": "setting",
        "resources": "light",
        "shard": "zig",
        "target_tool": True,
    },
    "zig_fmt": {
        "attr": attr.label(default = "//config:tool_zig_fmt", allow_files = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "setting",
        "resources": "light",
        "shard": "zig",
        "target_tool": True,
    },
}

_JVM_CHECK_TOOLS = ["javac", "kotlinc", "scalac"]

_GO_ANALYSIS_TOOLS = ["errcheck", "gotype", "govet", "staticcheck"]

def rustc_diagnostics_group(groups):
    """Returns the rustc diagnostics output group to read, preferring the metadata build."""
    for name in ["rustc_rmeta_output", "rustc_output"]:
        if name in groups:
            return name
    return None

def inherits_test_crate_sources(capability):
    """Whether a Rust test crate's inherited sources feed this capability's diagnostics."""
    return capability == "typecheck"

def merge_inherited_sources(direct_sources, inherited, class_id):
    """Adds inherited in-workspace .rs sources to a copy of direct_sources."""
    merged = {k: v.to_list() for k, v in direct_sources.items()}
    seen = {}
    for files in merged.values():
        for f in files:
            seen[f.path] = True
    added = []
    for f in inherited:
        if not f.is_source or f.extension != "rs" or f.owner.repo_name != "":
            continue
        if f.path in seen:
            continue
        seen[f.path] = True
        added.append(f)
    if added:
        merged[class_id] = merged.get(class_id, []) + added
    return {k: depset(v) for k, v in merged.items()}

_QML_CONTEXT_EXTENSIONS = ["qml", "qmltypes", "js", "mjs"]

def _capnp_closure(target):
    return [(i.path, i.file) for i in target[CapnpInfo].transitive_imports.to_list()]

def _flatc_closure(target):
    staged = []
    for f in target[FlatBuffersInfo].transitive_srcs.to_list():
        ws_path = f.short_path
        if ws_path.startswith("../"):
            ws_path = "_external/" + ws_path[3:]
        if ".." in ws_path.split("/"):
            continue
        staged.append((ws_path, f))
    return staged

def _avro_closure(target):
    return [(f.short_path, f) for f in target[AvroInfo].transitive_srcs.to_list() if ".." not in f.short_path.split("/")]

def _thrift_closure(target):
    return [(i.path, i.file) for i in target[ThriftInfo].transitive_includes.to_list()]

# Tools whose declared import closure is staged next to the checked files:
# tool -> (provider, closure(target) -> [(staged path, File)], strict). A strict
# tool fails when two different Files claim one staged path, because the
# compiler would see a different file than the one declared.
_CLOSURE_STAGING = {
    "capnp": (CapnpInfo, _capnp_closure, True),
    "flatc": (FlatBuffersInfo, _flatc_closure, False),
    "thrift": (ThriftInfo, _thrift_closure, True),
    "avro_tools": (AvroInfo, _avro_closure, True),
}

QmlContextInfo = provider(
    doc = "QML import context files reachable through deps.",
    fields = {"files": "depset[File]: qmldir, qml, qmltypes and js files."},
)

def _qml_context_impl(target, ctx):
    if target.label.repo_name != "":
        return [QmlContextInfo(files = depset())]
    direct = [
        f
        for f in target[DefaultInfo].files.to_list()
        if f.basename == "qmldir" or f.extension in _QML_CONTEXT_EXTENSIONS
    ]
    deps = getattr(ctx.rule.attr, "deps", [])
    if type(deps) != "list":
        deps = [deps]
    transitive = [
        dep[QmlContextInfo].files
        for dep in deps
        if type(dep) == "Target" and QmlContextInfo in dep
    ]
    return [QmlContextInfo(files = depset(direct, transitive = transitive))]

_qml_context_aspect = aspect(
    implementation = _qml_context_impl,
    doc = "Collects QML import context files through deps.",
    attr_aspects = ["deps"],
    provides = [QmlContextInfo],
)

def _jvm_jar_key(jar):
    """Returns the jar path without its header or ijar marker."""
    name = jar.basename.removeprefix("header_")
    for suffix in ["-hjar.jar", "-ijar.jar"]:
        if name.endswith(suffix):
            name = name.removesuffix(suffix) + ".jar"
            break
    return "jar:" + jar.short_path.rpartition("/")[0] + "/" + name

def _jacoco_runtime_jars(ctx):
    """Returns the jacoco agent runtime jars (a separate target, not a tool classpath)."""
    return ctx.attr._jacoco_runtime[JavaInfo].transitive_runtime_jars.to_list()

def _jvm_aux_jars(java_info, owned):
    """Returns the runtime classpath jars plus compile-only jars not in owned."""
    runtime = [java_info.transitive_runtime_jars]
    compile = [java_info.transitive_compile_time_jars]
    if java_info.compilation_info != None:
        runtime.append(java_info.compilation_info.runtime_classpath)
        compile.append(java_info.compilation_info.compilation_classpath)
    auxiliary = {}
    seen = {}
    for jar in owned.values():
        seen[_jvm_jar_key(jar)] = True
        if jar.owner != None:
            seen[str(jar.owner)] = True
    for jar in depset(transitive = runtime).to_list():
        if jar.path not in owned:
            auxiliary[jar.path] = jar
            seen[_jvm_jar_key(jar)] = True
            if jar.owner != None:
                seen[str(jar.owner)] = True
    for jar in depset(transitive = compile).to_list():
        if jar.path in owned or jar.path in auxiliary or _jvm_jar_key(jar) in seen:
            continue
        if jar.owner != None and str(jar.owner) in seen:
            continue
        auxiliary[jar.path] = jar
    return auxiliary

def _scalafix_context(target, what):
    """Returns the semanticdb jars and the dependency jars scalafix reads for one target."""
    if SemanticdbInfo not in target or not target[SemanticdbInfo].semanticdb_enabled or not target[SemanticdbInfo].is_bundled_in_jar:
        fail(what + ": scalafix needs semanticdb bundled in the jar; add register_toolchains(\"@rules_dx//scala/toolchain:semanticdb\") to MODULE.bazel, or set enable_semanticdb = True and semanticdb_bundle_in_jar = True on your own scala_toolchain")
    if JavaInfo not in target:
        fail(what + ": scalafix needs a JavaInfo target")
    owned = {}
    for output in target[JavaInfo].java_outputs:
        if output.class_jar != None:
            owned[output.class_jar.path] = output.class_jar
    if len(owned) == 0:
        fail(what + ": scalafix found no compiled jar for this target")
    auxiliary = _jvm_aux_jars(target[JavaInfo], owned)
    return (
        sorted(owned.values(), key = lambda f: f.path),
        sorted(auxiliary.values(), key = lambda f: f.path),
    )

def _fsharplint_context(target, ctx, what):
    """Returns the dependency assemblies and the compile context file for one F# target."""
    if DotnetAssemblyRuntimeInfo not in target:
        fail(what + ": fsharplint needs the compiled dependency assemblies of this target")
    if not (hasattr(ctx.rule.attr, "defines") and hasattr(ctx.rule.attr, "target_frameworks") and hasattr(ctx.rule.attr, "_target_framework")):
        fail(what + ": fsharplint needs an fsharp_* target with compile symbols")
    runtime = target[DotnetAssemblyRuntimeInfo]
    seen = {lib.basename: True for lib in runtime.libs}
    refs = []
    for info in runtime.deps.to_list():
        for lib in info.libs:
            if lib.basename.endswith(".dll") and lib.basename not in seen:
                seen[lib.basename] = True
                refs.append(lib)
    context = ctx.actions.declare_file(target.label.name + "-fsharplint-context.json")
    ctx.actions.write(
        output = context,
        content = json.encode({
            "defines": fsharp_compile_defines(ctx.rule.attr._target_framework[BuildSettingInfo].value, ctx.rule.attr.target_frameworks, ctx.rule.attr.defines),
            "langversion": ctx.rule.attr.langversion,
            "refs": ["dx-fsharplint/ref/%d-%s" % (index, lib.basename) for index, lib in enumerate(refs)],
            "sources": [f.short_path for f in ctx.rule.files.srcs],
        }),
    )
    return (refs, context)

def _runfiles_path(ctx, file):
    """Returns the runfiles-relative path of one file."""
    if file.short_path.startswith("../"):
        return file.short_path[3:]
    return ctx.workspace_name + "/" + file.short_path

def _jet_staging(target, ctx, path_to_file):
    """Returns the staged dependency sources and the analysis config of one Julia target."""
    info = target[JuliaInfo]
    owned = {f.path: ws for (ws, f) in path_to_file.items()}
    main = getattr(ctx.rule.attr, "main", None)
    main_files = main.files.to_list() if main != None else []
    if info.entry != None and info.entry.path in owned:
        roots = [owned[info.entry.path]]
    elif len(main_files) == 1 and main_files[0].path in owned:
        roots = [owned[main_files[0].path]]
    else:
        roots = sorted(owned.values())
    staged = []
    seen = {}
    candidates = info.transitive_srcs.to_list() + [f for f in info.runfiles.files.to_list() if f.basename in ("Project.toml", "JuliaProject.toml") and "/share/julia/" not in f.short_path]
    for f in sorted(candidates, key = lambda f: f.path):
        if f.is_directory or f.path in seen:
            continue
        seen[f.path] = True
        staged.append(("dx-jet/rf/" + _runfiles_path(ctx, f), f))
    config = ctx.actions.declare_file(target.label.name + "-jet-config.toml")
    ctx.actions.write(
        output = config,
        content = "includes = %s\nroots = %s\n" % (
            json.encode(sorted(info.includes.to_list())),
            json.encode(roots),
        ),
    )
    return (staged, config)

_DART_PACKAGE_CONFIG_REL = ".dart_tool/package_config.json"
_DART_OPTIONS_REL = "analysis_options.yaml"

def _dart_info(target, what):
    if DartAnalyzableInfo in target:
        return target[DartAnalyzableInfo].dart_info
    if DartInfo in target:
        return target[DartInfo]
    fail(what + ": dart needs a dart_library, dart_binary or dart_test target")

def _dart_context(target, ctx, what, capability, config_packages = []):
    """Writes the package config that gives each Dart file the language version Dart itself would use.

    The version is the one of the in-repo package whose root holds the file (`test/` and `bin/`
    included); a package that states none is listed without one, so it formats at the newest
    version like a file in no package. The analyzer view (lint and typecheck) also resolves the
    external packages under `_external/` and gets one stub `pubspec.yaml` per in-repo package
    root. Returns (package_config, empty_options, stubs) where stubs are (scratch path, File).
    """
    info = _dart_info(target, what)
    mode = DART_FORMAT if capability == "format" else DART_ANALYZE
    packages = info.transitive_packages.to_list()
    if mode == DART_ANALYZE:
        packages = dart_merge_packages(packages, config_packages)
    config = ctx.actions.declare_file(target.label.name + "-dart-package-config-" + capability + ".json")
    ctx.actions.write(output = config, content = json.encode({"configVersion": 2, "packages": dart_package_entries(packages, mode)}))
    options = ctx.actions.declare_file(target.label.name + "-dart-analysis-options-" + capability + ".yaml")
    ctx.actions.write(output = options, content = "")
    stubs = []
    if mode == DART_ANALYZE:
        for index, (path, content) in enumerate(dart_pubspec_stubs(packages)):
            stub = ctx.actions.declare_file("%s-dart-pubspec-%s-%d.yaml" % (target.label.name, capability, index))
            ctx.actions.write(output = stub, content = content)
            stubs.append((path, stub))
    return (config, options, stubs)

# Converters Vale shells out to, by source class: (tool name, owner rule, defs.bzl).
# The owner rule carries the converter in QualityToolsInfo, so Markdown-only targets
# never load Python or Node.
def _gofumpt_lang(ctx, what):
    """Derives gofumpt -lang from the resolved rules_go SDK."""
    toolchain = ctx.toolchains[GO_TOOLCHAIN_TYPE]
    if toolchain == None:
        fail(what + ": gofumpt needs a registered rules_go Go toolchain to derive -lang")
    return go_lang_version(toolchain.sdk.version)

_REAL_SHARD_SPECS = {
    "real_cc_format": {
        "capability": "format",
        "doc": "Additive C/C++ format family aspect (clang-format).",
        "shard": "cc",
        "suffix": "-cc",
    },
    "real_cc_lint": {
        "capability": "lint",
        "doc": "Additive C/C++ lint family aspect (clang-tidy, cppcheck opt-in).",
        "requires": [clang_syntax_aspect, _qml_context_aspect],
        "shard": "cc",
        "suffix": "-cc",
    },
    "real_cc_typecheck": {
        "capability": "typecheck",
        "doc": "Additive C/C++/Objective-C/CUDA typecheck family aspect (compiler -fsyntax-only replay, nvcc -cuda replay).",
        "requires": [clang_syntax_aspect, nvcc_syntax_aspect, _qml_context_aspect],
        "shard": "cc",
        "suffix": "-cc",
    },
    "real_d_typecheck": {
        "capability": "typecheck",
        "doc": "Additive D typecheck family aspect (ldc replay of the compile action).",
        "requires": [compile_check_aspect],
        "shard": "d",
        "suffix": "-d",
    },
    "real_dotnet_format": {
        "capability": "format",
        "doc": "Additive C#/F# format family aspect (CSharpier/Fantomas).",
        "shard": "dotnet",
        "suffix": "-dotnet",
    },
    "real_dotnet_lint": {
        "capability": "lint",
        "doc": "Additive C#/F# lint family aspect (Roslyn/FSharpLint).",
        "requires": [roslyn_aspect],
        "shard": "dotnet",
        "suffix": "-dotnet",
    },
    "real_dotnet_typecheck": {
        "capability": "typecheck",
        "doc": "Additive C#/F# typecheck family aspect (csc and fsc compiler diagnostics).",
        "requires": [fsc_aspect, roslyn_aspect],
        "shard": "dotnet",
        "suffix": "-dotnet",
    },
    "real_erlang_format": {
        "capability": "format",
        "doc": "Additive Erlang format family aspect (erlfmt).",
        "shard": "erlang",
        "suffix": "-erlang",
    },
    "real_erlang_lint": {
        "capability": "lint",
        "doc": "Additive Erlang lint family aspect (elvis).",
        "shard": "erlang",
        "suffix": "-erlang",
    },
    "real_erlang_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Erlang typecheck family aspect (dialyzer).",
        "shard": "erlang",
        "suffix": "-erlang",
    },
    "real_elixir_format": {
        "capability": "format",
        "doc": "Additive Elixir format family aspect (mix format).",
        "shard": "elixir",
        "suffix": "-elixir",
    },
    "real_elixir_lint": {
        "capability": "lint",
        "doc": "Additive Elixir lint family aspect (credo).",
        "shard": "elixir",
        "suffix": "-elixir",
    },
    "real_julia_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Julia typecheck family aspect (JET).",
        "shard": "julia",
        "suffix": "-julia",
    },
    "real_ruby_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Ruby typecheck family aspect (Sorbet).",
        "shard": "ruby",
        "suffix": "-ruby",
    },
    "real_ruby_format": {
        "capability": "format",
        "doc": "Additive Ruby format family aspect (Standard).",
        "shard": "ruby",
        "suffix": "-ruby",
    },
    "real_ruby_lint": {
        "capability": "lint",
        "doc": "Additive Ruby lint family aspect (RuboCop).",
        "shard": "ruby",
        "suffix": "-ruby",
    },
    "real_go_lint": {
        "capability": "lint",
        "doc": "Additive Go lint family aspect (delegated dx_govet, dx_errcheck and dx_staticcheck over rules_go packages).",
        "requires": [go_analysis_aspect],
        "shard": "go",
        "suffix": "-go",
        "toolchains": [config_common.toolchain_type(GO_TOOLCHAIN_TYPE, mandatory = False)],
    },
    "real_go_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Go typecheck family aspect (delegated dx_gotype over rules_go packages).",
        "requires": [go_analysis_aspect],
        "shard": "go",
        "suffix": "-go",
        "toolchains": [config_common.toolchain_type(GO_TOOLCHAIN_TYPE, mandatory = False)],
    },
    "real_go_format": {
        "capability": "format",
        "doc": "Additive Go format family aspect (gofumpt).",
        "shard": "go",
        "suffix": "-go",
        "toolchains": [config_common.toolchain_type(GO_TOOLCHAIN_TYPE, mandatory = False)],
    },
    "real_dart_format": {
        "capability": "format",
        "doc": "Additive Dart format family aspect (dart format).",
        "shard": "dart",
        "suffix": "-dart",
    },
    "real_dart_lint": {
        "capability": "lint",
        "doc": "Additive Dart lint family aspect (dart analyze).",
        "shard": "dart",
        "suffix": "-dart",
    },
    "real_dart_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Dart typecheck family aspect (dart analyze).",
        "shard": "dart",
        "suffix": "-dart",
    },
    "real_d_format": {
        "capability": "format",
        "doc": "Additive D format family aspect (dfmt).",
        "shard": "d",
        "suffix": "-d",
    },
    "real_d_lint": {
        "capability": "lint",
        "doc": "Additive D lint family aspect (D-Scanner).",
        "shard": "d",
        "suffix": "-d",
    },
    "real_python_format": {
        "capability": "format",
        "doc": "Additive Python format family aspect (ruff format).",
        "shard": "py",
        "suffix": "-py",
    },
    "real_python_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Python typecheck family aspect (ty).",
        "shard": "py",
        "suffix": "-py",
    },
    "real_julia_format": {
        "capability": "format",
        "doc": "Additive Julia format family aspect (JuliaFormatter).",
        "shard": "julia",
        "suffix": "-julia",
    },
    "real_zig_format": {
        "capability": "format",
        "doc": "Additive Zig format family aspect (zig fmt).",
        "shard": "zig",
        "suffix": "-zig",
    },
    "real_julia_lint": {
        "capability": "lint",
        "doc": "Additive Julia lint family aspect (JuliaWorkspaces).",
        "shard": "julia",
        "suffix": "-julia",
    },
    "real_zig_lint": {
        "capability": "lint",
        "doc": "Additive Zig lint family aspect (zig ast-check).",
        "shard": "zig",
        "suffix": "-zig",
    },
    "real_js_format": {
        "capability": "format",
        "doc": "Additive JavaScript/JSON/MDX format family aspect (Prettier, remark).",
        "shard": "js",
        "suffix": "-js",
    },
    "real_js_lint": {
        "capability": "lint",
        "doc": "Additive JavaScript lint family aspect (ESLint/Stylelint/graphql-eslint/Redocly opt-in).",
        "shard": "js",
        "suffix": "-js",
    },
    "real_js_typecheck": {
        "capability": "typecheck",
        "doc": "Additive TypeScript typecheck family aspect (delegated tsc over typescript_project and js_library).",
        "requires": [tsc_aspect],
        "shard": "js",
        "suffix": "-js",
    },
    "real_jvm_typecheck": {
        "capability": "typecheck",
        "doc": "Additive JVM typecheck family aspect (delegated javac over rules_java targets, kotlinc over rules_kotlin targets, scalac over Scala 2.13 rules_scala targets).",
        "requires": [jvm_check_aspect],
        "shard": "jvm",
        "suffix": "-jvm",
    },
    "real_jvm_format": {
        "capability": "format",
        "doc": "Additive JVM format family aspect (google-java-format/ktfmt).",
        "shard": "jvm",
        "suffix": "-jvm",
    },
    "real_jvm_lint": {
        "capability": "lint",
        "doc": "Additive JVM lint family aspect (Checkstyle/Pmd/SpotBugs/ktlint; SpotBugs target-coupled via JavaInfo).",
        "extra_attrs": {
            "_jacoco_runtime": attr.label(
                default = "@bazel_tools//tools/jdk:JacocoCoverage",
                providers = [JavaInfo],
                doc = "JaCoCo runtime SpotBugs needs to resolve coverage-instrumented classes.",
            ),
        },
        "shard": "jvm",
        "suffix": "-jvm",
    },
    "real_powershell_format": {
        "capability": "format",
        "doc": "Additive PowerShell format family aspect (PSScriptAnalyzer Invoke-Formatter).",
        "shard": "powershell",
        "suffix": "-powershell",
    },
    "real_powershell_lint": {
        "capability": "lint",
        "doc": "Additive PowerShell lint family aspect (PSScriptAnalyzer).",
        "shard": "powershell",
        "suffix": "-powershell",
    },
    "real_capnp_lint": {
        "capability": "lint",
        "doc": "Additive Cap'n Proto lint family aspect (capnp).",
        "shard": "capnp",
        "suffix": "-capnp",
    },
    "real_capnp_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Cap'n Proto typecheck family aspect (capnp errors only).",
        "shard": "capnp",
        "suffix": "-capnp",
    },
    "real_flatbuffers_lint": {
        "capability": "lint",
        "doc": "Additive FlatBuffers lint family aspect (flatc).",
        "shard": "flatbuffers",
        "suffix": "-flatbuffers",
    },
    "real_flatbuffers_typecheck": {
        "capability": "typecheck",
        "doc": "Additive FlatBuffers typecheck family aspect (flatc errors only).",
        "shard": "flatbuffers",
        "suffix": "-flatbuffers",
    },
    "real_avro_lint": {
        "capability": "lint",
        "doc": "Additive Avro lint family aspect (avro-tools).",
        "shard": "avro",
        "suffix": "-avro",
    },
    "real_avro_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Avro typecheck family aspect (avro-tools errors only).",
        "shard": "avro",
        "suffix": "-avro",
    },
    "real_thrift_lint": {
        "capability": "lint",
        "doc": "Additive Thrift lint family aspect (thrift).",
        "shard": "thrift",
        "suffix": "-thrift",
    },
    "real_thrift_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Thrift typecheck family aspect (thrift errors only).",
        "shard": "thrift",
        "suffix": "-thrift",
    },
    "real_protobuf_format": {
        "capability": "format",
        "doc": "Additive Protocol Buffers format family aspect (buf format).",
        "shard": "protobuf",
        "suffix": "-protobuf",
    },
    "real_protobuf_lint": {
        "capability": "lint",
        "doc": "Additive Protocol Buffers lint family aspect (buf lint).",
        "shard": "protobuf",
        "suffix": "-protobuf",
    },
    "real_python_lint": {
        "capability": "lint",
        "doc": "Additive Python lint family aspect (flake8/pylint/pydoclint).",
        "shard": "py",
        "suffix": "-py",
    },
    "real_rust_format": {
        "capability": "format",
        "doc": "Additive Rust format family aspect (toolchain rustfmt).",
        "shard": "rust",
        "suffix": "-rust",
        "toolchains": rust_toolchain_toolchains(),
    },
    "real_rust_lint": {
        "capability": "lint",
        "doc": "Additive Rust lint family aspect (delegated Clippy).",
        "requires": [rust_clippy_aspect],
        "shard": "rust",
        "suffix": "-rust",
        "toolchains": rust_toolchain_toolchains(),
    },
    "real_rust_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Rust typecheck family aspect (delegated rustc).",
        "shard": "rust",
        "suffix": "-rust",
        "toolchains": rust_toolchain_toolchains(),
    },
    "real_scala_format": {
        "capability": "format",
        "doc": "Additive Scala format family aspect (scalafmt).",
        "shard": "scala",
        "suffix": "-scala",
    },
    "real_scala_lint": {
        "capability": "lint",
        "doc": "Additive Scala lint family aspect (Scalafix; target-coupled via SemanticdbInfo and JavaInfo).",
        "shard": "scala",
        "suffix": "-scala",
    },
    "real_fortran_format": {
        "capability": "format",
        "doc": "Additive Fortran format family aspect (ffmt).",
        "shard": "fortran",
        "suffix": "-fortran",
    },
    "real_fortran_lint": {
        "capability": "lint",
        "doc": "Additive Fortran lint family aspect (fortitude).",
        "shard": "fortran",
        "suffix": "-fortran",
    },
    "real_fortran_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Fortran typecheck family aspect (flang replay of the compile action).",
        "requires": [compile_check_aspect],
        "shard": "fortran",
        "suffix": "-fortran",
    },
    "real_haskell_format": {
        "capability": "format",
        "doc": "Additive Haskell format family aspect (ormolu).",
        "shard": "haskell",
        "suffix": "-haskell",
    },
    "real_haskell_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Haskell typecheck family aspect (ghc -fno-code replay of the compile action).",
        "requires": [compile_check_aspect],
        "shard": "haskell",
        "suffix": "-haskell",
    },
    "real_haskell_lint": {
        "capability": "lint",
        "doc": "Additive Haskell lint family aspect (hlint).",
        "shard": "haskell",
        "suffix": "-haskell",
    },
    "real_ocaml_lint": {
        "capability": "lint",
        "doc": "Additive OCaml lint family aspect (zanuda; target-coupled via OCamlCmtInfo).",
        "extra_attrs": {
            "_ocaml_stdlib": attr.label(
                default = "@dx_ocaml_toolchains//:stdlib_cmis",
                cfg = "exec",
                doc = "The compiler's standard library interfaces, which zanuda type checks against.",
            ),
        },
        "shard": "ocaml",
        "suffix": "-ocaml",
    },
    "real_ocaml_format": {
        "capability": "format",
        "doc": "Additive OCaml format family aspect (topiary).",
        "shard": "ocaml",
        "suffix": "-ocaml",
    },
    "real_ocaml_typecheck": {
        "capability": "typecheck",
        "doc": "Additive OCaml typecheck family aspect (ocamlc -stop-after typing replay of the compile action).",
        "requires": [compile_check_aspect],
        "shard": "ocaml",
        "suffix": "-ocaml",
    },
    "real_zig_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Zig typecheck family aspect (zig -fno-emit-bin replay of the compile action).",
        "requires": [compile_check_aspect],
        "shard": "zig",
        "suffix": "-zig",
    },
}

def _spotbugs_hook(target, ctx, resolved, _capability, _what, _shared):
    if JavaInfo not in target:
        return _without(resolved, "spotbugs")
    owned = {}
    for output in target[JavaInfo].java_outputs:
        if output.class_jar != None and ".." not in output.class_jar.short_path.split("/"):
            owned[output.class_jar.path] = output.class_jar
    auxiliary = _jvm_aux_jars(target[JavaInfo], owned)
    if ctx.configuration.coverage_enabled and hasattr(ctx.attr, "_jacoco_runtime"):
        for jar in _jacoco_runtime_jars(ctx):
            if jar.path not in owned:
                auxiliary[jar.path] = jar
    if len(owned) == 0:
        return _without(resolved, "spotbugs")
    tool_files = []
    inputs = []
    for jar in sorted(owned.values(), key = lambda f: f.short_path):
        _stage(tool_files, inputs, "dx-spotbugs/owned/" + jar.short_path, jar)
    for index, jar in enumerate(sorted(auxiliary.values(), key = lambda f: f.short_path)):
        _stage(tool_files, inputs, "dx-spotbugs/aux/%d-%s" % (index, jar.basename), jar)
    return real_context(tool_files = tool_files, inputs = inputs)

def _dialyzer_hook(target, ctx, resolved, _capability, what, _shared):
    if BeamInfo not in target or len(target[BeamInfo].owned.to_list()) == 0:
        return _without(resolved, "dialyzer")
    tool_target = getattr(ctx.attr, "_dialyzer")
    if DialyzerToolInfo not in tool_target:
        fail(what + ": dialyzer needs PLTs; set --@rules_dx//config:tool_dialyzer=@rules_dx//erlang/tools:dialyzer")

    # The tool's PLTs come first; a language's own follow, without repeating one.
    plts = []
    seen_plts = {}
    for plt in tool_target[DialyzerToolInfo].plts + target[BeamInfo].plts:
        if plt.path not in seen_plts:
            seen_plts[plt.path] = True
            plts.append(plt)
    code = sorted(target[BeamInfo].code, key = lambda f: f.basename)
    owned = sorted(target[BeamInfo].owned.to_list(), key = lambda f: f.short_path)
    owned_paths = {f.path: True for f in owned}
    aux = [f for f in sorted(target[BeamInfo].deps.to_list(), key = lambda f: f.short_path) if f.path not in owned_paths]
    tool_files = []
    inputs = []
    for index, plt in enumerate(plts):
        _stage(tool_files, inputs, "dx-dialyzer/plt/%s-%s" % (("000" + str(index))[-3:], plt.basename), plt)
    for index, beam in enumerate(owned):
        _stage(tool_files, inputs, "dx-dialyzer/owned/%d/%s" % (index, beam.basename), beam)
    for beam in code:
        _stage(tool_files, inputs, "dx-dialyzer/pa/" + beam.basename, beam)
    seen_aux = {}
    for index, beam in enumerate(aux):
        if beam.path in seen_aux:
            continue
        seen_aux[beam.path] = True
        _stage(tool_files, inputs, "dx-dialyzer/aux/%d/%s" % (index, beam.basename), beam)
    return real_context(tool_files = tool_files, inputs = inputs)

def _zanuda_hook(target, ctx, resolved, _capability, what, _shared):
    # zanuda reads the typed trees the compile wrote. A target that owns OCaml sources but
    # carries no OCamlCmtInfo, or one that owns a source with no tree, would otherwise lint
    # nothing and look clean, so both fail the analysis.
    if OCamlCmtInfo not in target:
        fail(what + ": zanuda needs OCamlCmtInfo, which only the dx ocaml_module and ocaml_signature rules provide")
    listed = {}
    for entry in target[OCamlCmtInfo].owned:
        if entry.cmt and entry.source:
            listed[entry.source.short_path] = ("impl", entry.cmt)
        if entry.cmti and entry.signature:
            listed[entry.signature.short_path] = ("intf", entry.cmti)
    tool_files = []
    inputs = []
    for stage in resolved:
        if stage["tool"] != "zanuda":
            continue
        for ws_path in stage["sources"]:
            if ws_path not in listed:
                fail(what + ": zanuda has no .cmt or .cmti for " + ws_path + "; was the target compiled with -no-bin-annot?")
            (kind, tree) = listed[ws_path]
            _stage(tool_files, inputs, "dx-zanuda/%s/%s.%s" % (kind, ws_path, tree.extension), tree)
    stdlib = ctx.files._ocaml_stdlib
    inputs.extend(stdlib)

    # Relative to the execution root, like the binary; the runner makes it absolute.
    env = [real_request_tool_env("OCAMLLIB", stdlib[0].dirname)] if len(stdlib) > 0 else []
    return real_context(tool_files = tool_files, inputs = inputs, env = env)

def _sorbet_hook(target, _ctx, _resolved, _capability, _what, shared):
    # Dependency sources are only read for the constants they define; the owned sources
    # are the CLI paths, so they stay out of the dependency set. A target without
    # RubyFilesInfo has no dependencies to stage.
    owned_paths = {f.path: True for f in shared.path_to_file.values()}
    tool_files = []
    inputs = []
    if RubyFilesInfo in target:
        seen_deps = {}
        for dep_file in sorted(target[RubyFilesInfo].transitive_srcs.to_list(), key = lambda f: f.path):
            if dep_file.path.endswith((".rb", ".rbi")) and dep_file.path not in owned_paths and dep_file.path not in seen_deps:
                seen_deps[dep_file.path] = True
                _stage(tool_files, inputs, "dx-sorbet/dep/" + staged_path(dep_file), dep_file)
    return real_context(tool_files = tool_files, inputs = inputs)

def _scalafix_hook(target, _ctx, _resolved, _capability, what, _shared):
    (owned, aux) = _scalafix_context(target, what)
    tool_files = []
    inputs = []
    for index, jar in enumerate(owned):
        _stage(tool_files, inputs, "dx-scalafix/owned/%d-%s" % (index, jar.basename), jar)
    for index, jar in enumerate(aux):
        _stage(tool_files, inputs, "dx-scalafix/aux/%d-%s" % (index, jar.basename), jar)
    return real_context(tool_files = tool_files, inputs = inputs)

def _fsharplint_hook(target, ctx, _resolved, _capability, what, _shared):
    (refs, context) = _fsharplint_context(target, ctx, what)
    tool_files = []
    inputs = []
    for index, lib in enumerate(refs):
        _stage(tool_files, inputs, "dx-fsharplint/ref/%d-%s" % (index, lib.basename), lib)
    _stage(tool_files, inputs, "dx-fsharplint/context.json", context)
    return real_context(tool_files = tool_files, inputs = inputs)

def _jet_hook(target, ctx, resolved, _capability, _what, shared):
    if JuliaInfo not in target:
        return _without(resolved, "jet")
    (staged, config) = _jet_staging(target, ctx, shared.path_to_file)
    tool_files = []
    inputs = []
    if config != None:
        for (rel, staged_file) in staged:
            _stage(tool_files, inputs, rel, staged_file)
        _stage(tool_files, inputs, "dx-jet/config.toml", config)
    return real_context(tool_files = tool_files, inputs = inputs)

def _dart_hook(target, ctx, _resolved, capability, what, _shared):
    config_packages = []
    config_files = []
    if capability != "format" and hasattr(ctx.rule.attr, "aspect_hints"):
        for hint_target in ctx.rule.attr.aspect_hints:
            if DxDartConfigInfo in hint_target:
                config_packages.extend(hint_target[DxDartConfigInfo].packages.to_list())
                config_files.extend(hint_target[DxDartConfigInfo].srcs.to_list())
                config_files.extend(hint_target[DxDartConfigInfo].resources.to_list())
    (package_config, dart_options, stubs) = _dart_context(target, ctx, what, capability, config_packages)

    pairs = []
    reserved = []
    if capability != "format":
        dart_info = _dart_info(target, what)
        dart_files = [f for f in dart_info.transitive_srcs.to_list() if f.extension == "dart"]
        dart_files += [f for f in dart_info.transitive_resources.to_list() if f.basename == "_embedder.yaml"]
        for f in dart_files + config_files:
            ws_path = f.short_path
            if ws_path.startswith("../"):
                ws_path = dart_external_root(ws_path)
            if ".." in ws_path.split("/"):
                continue
            pairs.append((ws_path, f, False))
        for (stub_path, _) in stubs:
            reserved.append((stub_path, "generated pubspec.yaml stub collides with the staged source " + stub_path))

    def _with_config(hint, config_rel):
        # dart format reads `formatter:` from the nearest analysis_options.yaml, so the
        # hinted config (or an empty one) sits at the scratch root and nowhere else.
        options = dart_options
        tool_files = []
        inputs = []
        if hint != None:
            options = hint.config
        else:
            inputs.append(options)
        if config_rel != _DART_OPTIONS_REL:
            tool_files.append((_DART_OPTIONS_REL, options))
        _stage(tool_files, inputs, _DART_PACKAGE_CONFIG_REL, package_config)
        for (stub_path, stub) in stubs:
            if stub_path == config_rel:
                fail(what + ": generated pubspec.yaml stub collides with the config " + stub_path)
            _stage(tool_files, inputs, stub_path, stub)
        return struct(tool_files = tool_files, inputs = inputs)

    return real_context(resolve_pairs = pairs, reserved = reserved, with_config = _with_config)

def _jvm_check_hook(tool):
    def _hook(target, _ctx, resolved, _capability, _what, _shared):
        if JvmCheckInfo not in target or target[JvmCheckInfo].tool != tool:
            return _without(resolved, tool)
        log = target[JvmCheckInfo].log
        return real_context(upstream = [log], late_inputs = [log])

    return _hook

def _go_analysis_hook(tool):
    def _hook(target, _ctx, resolved, _capability, what, _shared):
        if GoAnalysisInfo not in target:
            if GoArchive not in target:
                return _without(resolved, tool)
            fail(what + ": " + tool + " needs the Go analysis aspect report for a rules_go target")
        report = target[GoAnalysisInfo].reports[tool]
        return real_context(upstream = [report], late_inputs = [report])

    return _hook

def _compile_check_hook(tool):
    def _hook(target, _ctx, resolved, _capability, _what, _shared):
        if CompileCheckInfo not in target or target[CompileCheckInfo].tool != tool:
            return _without(resolved, tool)
        info = target[CompileCheckInfo]
        tool_files = []
        inputs = []
        for i in range(len(info.diagnostics)):
            inputs.append(info.diagnostics[i])
            stem = info.diagnostics[i].basename.rpartition(".")[0]
            _stage(tool_files, inputs, "dx-compile-check/" + stem + "/exit", info.statuses[i])
        return real_context(upstream = info.diagnostics, late_tool_files = tool_files, late_inputs = inputs)

    return _hook

def _tsc_hook(target, _ctx, resolved, _capability, _what, _shared):
    if TscInfo not in target:
        return _without(resolved, "tsc")
    info = target[TscInfo]
    return real_context(upstream = [info.stdout], late_tool_files = [("dx-tsc/exit", info.status)], late_inputs = [info.stdout, info.status])

def _roslyn_hook(target, _ctx, _resolved, _capability, what, _shared):
    if RoslynInfo not in target:
        fail(what + ": roslyn and csc need a csharp_* target compiled with the SDK analyzers")
    info = target[RoslynInfo]
    tool_files = []
    inputs = []
    for i in range(len(info.tfms)):
        inputs.append(info.sarifs[i])
        _stage(tool_files, inputs, "dx-roslyn/" + info.tfms[i] + "/exit", info.statuses[i])
    return real_context(upstream = info.sarifs, late_tool_files = tool_files, late_inputs = inputs)

def _fsc_hook(target, _ctx, _resolved, _capability, what, _shared):
    if FscUnsupportedInfo in target:
        fail(what + ": fsc is unavailable: " + target[FscUnsupportedInfo].reason)
    if FscInfo not in target:
        fail(what + ": fsc needs an fsharp_* target")
    info = target[FscInfo]
    tool_files = []
    inputs = []
    for i in range(len(info.tfms)):
        inputs.append(info.logs[i])
        _stage(tool_files, inputs, "dx-fsc/" + info.tfms[i] + "/exit", info.statuses[i])
    return real_context(upstream = info.logs, late_tool_files = tool_files, late_inputs = inputs)

def _clang_info(target, what):
    if ClangSyntaxInfo not in target:
        fail(what + ": clang_syntax and clang_tidy need a cc_*, objc_* or cuda_* target with compile actions (a header-only library needs --features=parse_headers)")
    return target[ClangSyntaxInfo]

def _clang_tidy_hook(target, _ctx, _resolved, _capability, what, _shared):
    info = _clang_info(target, what)
    return real_context(upstream = info.tidy_diagnostics, late_inputs = info.tidy_diagnostics)

def _clang_syntax_hook(target, _ctx, _resolved, _capability, what, _shared):
    info = _clang_info(target, what)
    tool_files = []
    inputs = []
    for i in range(len(info.diagnostics)):
        inputs.append(info.diagnostics[i])
        stem = info.diagnostics[i].basename.rpartition(".")[0]
        _stage(tool_files, inputs, "dx-clang-syntax/" + stem + "/exit", info.statuses[i])
        _stage(tool_files, inputs, "dx-clang-syntax/" + stem + "/driver", info.drivers[i])
    return real_context(upstream = info.diagnostics, late_tool_files = tool_files, late_inputs = inputs)

def _nvcc_syntax_hook(target, _ctx, _resolved, _capability, what, _shared):
    if NvccSyntaxInfo not in target:
        fail(what + ": nvcc_syntax needs a cuda_library or cuda_objects target with a CudaCompile action (a header-only library has none)")
    info = target[NvccSyntaxInfo]
    tool_files = []
    inputs = []
    for i in range(len(info.diagnostics)):
        inputs.append(info.diagnostics[i])
        stem = info.diagnostics[i].basename.rpartition(".")[0]
        _stage(tool_files, inputs, "dx-nvcc-syntax/" + stem + "/exit", info.statuses[i])
    return real_context(upstream = info.diagnostics, late_tool_files = tool_files, late_inputs = inputs)

def _prettier_hook(target, _ctx, _resolved, _capability, _what, _shared):
    if SqlDialectInfo not in target:
        return real_context()
    return real_context(options = {"dialect": target[SqlDialectInfo].dialect})

def _rustfmt_hook(target, _ctx, resolved, _capability, what, shared):
    if not shared.has_toolchain:
        fail(what + ": rustfmt needs the Rust family aspect")
    if _rust_common.crate_info in target:
        edition = target[_rust_common.crate_info].edition
    elif _rust_common.test_crate_info in target:
        edition = target[_rust_common.test_crate_info].crate.edition
    else:
        edition = RUST_EDITION
    generated = generated_source_paths(shared.direct_files)
    pruned = None
    if len(generated) > 0:
        pruned = prune_tool_generated_sources(resolved, generated, "rustfmt")
    return real_context(resolved = pruned, options = {"edition": edition})

def _clippy_hook(target, _ctx, _resolved, _capability, what, shared):
    if not shared.has_toolchain:
        fail(what + ": clippy needs the Rust family aspect")
    diagnostics = []
    if OutputGroupInfo in target and "clippy_output" in target[OutputGroupInfo]:
        diagnostics = target[OutputGroupInfo]["clippy_output"].to_list()
    return real_context(upstream = diagnostics, late_inputs = diagnostics, delegated = len(diagnostics) > 0)

def _rustc_hook(target, _ctx, _resolved, _capability, what, shared):
    if not shared.has_toolchain:
        fail(what + ": rustc needs the Rust family aspect")
    diagnostics = []
    if OutputGroupInfo in target:
        group = rustc_diagnostics_group(target[OutputGroupInfo])
        if group:
            diagnostics = target[OutputGroupInfo][group].to_list()
    return real_context(upstream = diagnostics, late_inputs = diagnostics, delegated = len(diagnostics) > 0)

def _ty_hook(target, _ctx, _resolved, _capability, _what, _shared):
    if _PyInfo not in target:
        return real_context()
    transitive = getattr(target[_PyInfo], "transitive_sources", None)
    if transitive == None:
        return real_context()
    pairs = []
    search_dirs = []
    for f in transitive.to_list():
        ws_path = f.short_path
        if ".." not in ws_path.split("/") and (ws_path.endswith(".py") or ws_path.endswith(".pyi")):
            pairs.append((ws_path, f, False))
    for f in transitive.to_list():
        if f.is_directory and f.short_path.endswith(".install"):
            search_dirs.append(f)
    return real_context(early_inputs = search_dirs, resolve_pairs = pairs, options = {"search_paths": search_dirs})

def _qmllint_hook(target, _ctx, _resolved, _capability, _what, _shared):
    context_files = list(target[DefaultInfo].files.to_list())
    if QmlContextInfo in target:
        context_files.extend(target[QmlContextInfo].files.to_list())
    pairs = []
    for f in context_files:
        ws_path = f.short_path
        if ws_path.startswith("../"):
            ws_path = "_external/" + ws_path[3:]
        if ".." in ws_path.split("/"):
            continue
        if f.basename != "qmldir" and f.extension not in _QML_CONTEXT_EXTENSIONS:
            continue
        pairs.append((ws_path, f, False))
    return real_context(resolve_pairs = pairs)

def _closure_hook(tool):
    (provider, closure_files, strict) = _CLOSURE_STAGING[tool]

    def _hook(target, _ctx, _resolved, _capability, _what, _shared):
        if provider not in target:
            return real_context()
        return real_context(resolve_pairs = [(ws_path, f, strict) for (ws_path, f) in closure_files(target)])

    return _hook

def _buf_hook(target, _ctx, _resolved, _capability, _what, _shared):
    if ProtoInfo not in target:
        return real_context()
    pairs = []
    proto_roots = sorted([r for r in target[ProtoInfo].transitive_proto_path.to_list() if r != "."], key = lambda r: -len(r))
    for f in target[ProtoInfo].transitive_sources.to_list():
        ws_path = f.path
        for root in proto_roots:
            if ws_path.startswith(root + "/"):
                ws_path = ws_path[len(root) + 1:]
                break
        if ".." in ws_path.split("/"):
            continue
        pairs.append((ws_path, f, False))
    return real_context(resolve_pairs = pairs)

def _psscriptanalyzer_hook(target, ctx, _resolved, _capability, _what, _shared):
    context_files = list(target[DefaultInfo].files.to_list())
    upstream = getattr(ctx.rule.attr, "upstream", None)
    if type(upstream) == "Target" and _PwshInfo in upstream:
        context_files.extend(upstream[DefaultInfo].default_runfiles.files.to_list())
    pairs = []
    module_roots = {}
    for f in context_files:
        ws_path = f.short_path
        if f.extension not in ["ps1", "psd1", "psm1"]:
            continue
        if ws_path.startswith("../"):
            if "_toolchains__" in ws_path.split("/")[1]:
                continue
            ws_path = "external/" + ws_path[3:]
        if ".." in ws_path.split("/"):
            continue
        if f.extension in ["psd1", "psm1"]:
            module_dir = ws_path.rpartition("/")[0]
            module_roots[module_dir.rpartition("/")[0] or "."] = True
        pairs.append((ws_path, f, False))
    env = [real_request_tool_env("DX_PWSH_MODULE_ROOTS", "|".join(sorted(module_roots.keys())))]
    return real_context(resolve_pairs = pairs, env = env)

def _gofumpt_hook(_target, ctx, _resolved, _capability, what, _shared):
    return real_context(options = {"edition": _gofumpt_lang(ctx, what)})

def _upstream_files(srcs, kind):
    if srcs == None:
        return struct(files = [], error = "upstream " + kind + " target has no readable srcs; unsupported ownership is unchecked, not clean")
    return struct(files = list(srcs), error = "")

def _proto_sources(target, ctx):
    if ProtoInfo not in target:
        return None
    return _upstream_files(getattr(ctx.rule.files, "srcs", None), "proto")

def _rust_sources(target, ctx):
    if _rust_common.crate_info not in target and _rust_common.test_crate_info not in target:
        return None
    return _upstream_files(getattr(ctx.rule.files, "srcs", None), "rust")

def _cc_sources(target, ctx):
    if CcInfo not in target:
        return None
    srcs = getattr(ctx.rule.files, "srcs", None)
    hdrs = getattr(ctx.rule.files, "hdrs", None)
    non_arc_srcs = getattr(ctx.rule.files, "non_arc_srcs", None)
    if srcs == None and hdrs == None and non_arc_srcs == None:
        return struct(files = [], error = "upstream cc target has no readable srcs, hdrs or non_arc_srcs; unsupported ownership is unchecked, not clean")
    files = []
    for group in [srcs, hdrs, non_arc_srcs]:
        if group != None:
            files.extend(group)
    return struct(files = files, error = "")

def _rust_harness_sources(target, capability, direct_sources):
    """Adds the sources of a Rust test crate's harness to a typecheck's direct sources."""
    if not inherits_test_crate_sources(capability):
        return direct_sources
    harness = None
    if _rust_common.test_crate_info in target:
        harness = target[_rust_common.test_crate_info].crate
    elif _rust_common.crate_info in target and target[_rust_common.crate_info].is_test:
        harness = target[_rust_common.crate_info]
    if harness == None:
        return direct_sources
    return merge_inherited_sources(direct_sources, harness.srcs.to_list(), "rust")

def _rust_toolchain_file(ctx, name):
    """Returns one tool file from the resolved Rust toolchains."""
    if name == "rustc":
        return rust_toolchain_rustc(ctx)
    clippy_driver, rustfmt = rust_toolchain_tools(ctx)
    return clippy_driver if name == "clippy_driver" else rustfmt

# The core runs the hooks in this order and no other.
_HOOK_ORDER = [
    "spotbugs",
    "dialyzer",
    "zanuda",
    "sorbet",
    "scalafix",
    "fsharplint",
    "jet",
    "dart",
] + _JVM_CHECK_TOOLS + _GO_ANALYSIS_TOOLS + COMPILE_CHECK_TOOLS + [
    "tsc",
    "roslyn",
    "csc",
    "fsc",
    "clang_syntax",
    "clang_tidy",
    "nvcc_syntax",
    "prettier",
    "rustfmt",
    "clippy",
    "rustc",
    "ty",
    "qmllint",
    "cue",
    "pkl",
    "terraform",
    "opa",
    "bicep",
    "jsonnetfmt",
    "jsonnet_lint",
    "redocly",
    "capnp",
    "flatc",
    "thrift",
    "avro_tools",
    "buf",
    "psscriptanalyzer",
    "vale",
    "gofumpt",
]

_CONTEXT_HOOKS = {
    "spotbugs": _spotbugs_hook,
    "dialyzer": _dialyzer_hook,
    "zanuda": _zanuda_hook,
    "sorbet": _sorbet_hook,
    "scalafix": _scalafix_hook,
    "fsharplint": _fsharplint_hook,
    "jet": _jet_hook,
    "dart": _dart_hook,
    "tsc": _tsc_hook,
    "roslyn": _roslyn_hook,
    "csc": _roslyn_hook,
    "fsc": _fsc_hook,
    "clang_syntax": _clang_syntax_hook,
    "clang_tidy": _clang_tidy_hook,
    "nvcc_syntax": _nvcc_syntax_hook,
    "prettier": _prettier_hook,
    "rustfmt": _rustfmt_hook,
    "clippy": _clippy_hook,
    "rustc": _rustc_hook,
    "ty": _ty_hook,
    "qmllint": _qmllint_hook,
    "redocly": _default_files_hook(["yaml", "yml", "json"]),
    "buf": _buf_hook,
    "psscriptanalyzer": _psscriptanalyzer_hook,
    "gofumpt": _gofumpt_hook,
} | _CORE_HOOKS | _CUE_HOOKS | _PKL_HOOKS | _JSONNET_HOOKS | _REGO_HOOKS | _BICEP_HOOKS | _TERRAFORM_HOOKS | _LUA_HOOKS | _PHP_HOOKS | _KUBERNETES_HOOKS | _SQL_HOOKS | {tool: _jvm_check_hook(tool) for tool in _JVM_CHECK_TOOLS} | {
    tool: _go_analysis_hook(tool)
    for tool in _GO_ANALYSIS_TOOLS
} | {
    tool: _compile_check_hook(tool)
    for tool in COMPILE_CHECK_TOOLS
} | {
    tool: _closure_hook(tool)
    for tool in _CLOSURE_STAGING.keys()
}

_CFG = real_config(
    tools = _REAL_TOOL_TABLE | _CORE_TOOLS | _ANSIBLE_TOOLS | _CLOUDFORMATION_TOOLS | _MAKE_TOOLS | _DOCKERFILE_TOOLS | _HTML_TEMPLATE_TOOLS | _XML_TOOLS | _YAML_TOOLS | _NIX_TOOLS | _SHELL_TOOLS | _LATEX_TOOLS | _CMAKE_TOOLS | _TYPST_TOOLS | _TEXT_TOOLS | _SWIFT_TOOLS | _PERL_TOOLS | _CUE_TOOLS | _PKL_TOOLS | _JSONNET_TOOLS | _REGO_TOOLS | _BICEP_TOOLS | _TERRAFORM_TOOLS | _LUA_TOOLS | _PHP_TOOLS | _KUBERNETES_TOOLS | _SQL_TOOLS,
    shard_specs = _REAL_SHARD_SPECS | _CORE_SHARD_SPECS | _ANSIBLE_SHARD_SPECS | _CLOUDFORMATION_SHARD_SPECS | _MAKE_SHARD_SPECS | _DOCKERFILE_SHARD_SPECS | _HTML_TEMPLATE_SHARD_SPECS | _XML_SHARD_SPECS | _YAML_SHARD_SPECS | _NIX_SHARD_SPECS | _SHELL_SHARD_SPECS | _LATEX_SHARD_SPECS | _CMAKE_SHARD_SPECS | _TYPST_SHARD_SPECS | _TEXT_SHARD_SPECS | _SWIFT_SHARD_SPECS | _PERL_SHARD_SPECS | _CUE_SHARD_SPECS | _PKL_SHARD_SPECS | _JSONNET_SHARD_SPECS | _REGO_SHARD_SPECS | _BICEP_SHARD_SPECS | _TERRAFORM_SHARD_SPECS | _LUA_SHARD_SPECS | _PHP_SHARD_SPECS | _KUBERNETES_SHARD_SPECS | _SQL_SHARD_SPECS,
    hook_order = _HOOK_ORDER,
    context_hooks = _CONTEXT_HOOKS,
    source_recognizers = [_proto_sources, _rust_sources, _cc_sources],
    harness_sources = _rust_harness_sources,
    toolchain = struct(type = RUST_TOOLCHAIN_TYPE, file = _rust_toolchain_file),
)

def real_wired_tools():
    """Maps each capability to the sorted tool names the table wires for it."""
    return _real_wired_tools(_CFG)

def real_policy_error(policy, capability, what):
    """Validates the selected workspace policy for one real capability."""
    return _real_policy_error(_CFG, policy, capability, what)

def upstream_direct_sources(target, ctx):
    """Derives directly owned sources for an ordinary upstream native target."""
    return real_upstream_direct_sources(_CFG.source_recognizers, target, ctx)

def real_quality_resource_set(tools):
    """Returns the resource_set callback for the costliest tier among the launched tools."""
    return _real_quality_resource_set(_CFG, tools)

def real_tool_table_error():
    """Validates the real tool table and that a built shard runs every tool capability."""
    return _real_tool_table_error(_CFG)

def real_shard_result_stems():
    """Maps each shard's dx_results stem ("real-" + capability + suffix) to its shard name."""
    return _real_shard_result_stems(_CFG)

def real_wired_capabilities():
    """Maps each tool to the sorted capabilities a built shard aspect runs for it."""
    return _real_wired_capabilities(_CFG)

real_tool_table_shape_error = _real_tool_table_shape_error
RealShardProbeInfo = _RealShardProbeInfo

def _make_real_aspect(shard_name):
    return real_make_aspect(_CFG, shard_name)

def _make_real_probe_aspect(capability):
    return real_make_probe_aspect(_CFG, capability)

real_lint_aspect = _real_lint_aspect

real_format_aspect = _real_format_aspect

real_typecheck_aspect = _real_typecheck_aspect

real_zig_typecheck_aspect = _make_real_aspect("real_zig_typecheck")

real_cc_format_aspect = _make_real_aspect("real_cc_format")

real_cc_lint_aspect = _make_real_aspect("real_cc_lint")

real_cc_typecheck_aspect = _make_real_aspect("real_cc_typecheck")

real_d_typecheck_aspect = _make_real_aspect("real_d_typecheck")

real_dotnet_format_aspect = _make_real_aspect("real_dotnet_format")

real_dotnet_lint_aspect = _make_real_aspect("real_dotnet_lint")

real_dotnet_typecheck_aspect = _make_real_aspect("real_dotnet_typecheck")

real_erlang_format_aspect = _make_real_aspect("real_erlang_format")

real_erlang_lint_aspect = _make_real_aspect("real_erlang_lint")

real_erlang_typecheck_aspect = _make_real_aspect("real_erlang_typecheck")

real_elixir_format_aspect = _make_real_aspect("real_elixir_format")

real_elixir_lint_aspect = _make_real_aspect("real_elixir_lint")

real_dart_format_aspect = _make_real_aspect("real_dart_format")

real_dart_lint_aspect = _make_real_aspect("real_dart_lint")

real_dart_typecheck_aspect = _make_real_aspect("real_dart_typecheck")

real_d_format_aspect = _make_real_aspect("real_d_format")

real_d_lint_aspect = _make_real_aspect("real_d_lint")

real_perl_format_aspect = _real_perl_format_aspect

real_perl_lint_aspect = _real_perl_lint_aspect

real_python_format_aspect = _make_real_aspect("real_python_format")

real_python_typecheck_aspect = _make_real_aspect("real_python_typecheck")

real_julia_format_aspect = _make_real_aspect("real_julia_format")

real_swift_format_aspect = _real_swift_format_aspect

real_swift_lint_aspect = _real_swift_lint_aspect

real_zig_format_aspect = _make_real_aspect("real_zig_format")

real_zig_lint_aspect = _make_real_aspect("real_zig_lint")

real_julia_lint_aspect = _make_real_aspect("real_julia_lint")

real_julia_typecheck_aspect = _make_real_aspect("real_julia_typecheck")

real_ruby_typecheck_aspect = _make_real_aspect("real_ruby_typecheck")

real_ruby_format_aspect = _make_real_aspect("real_ruby_format")

real_ruby_lint_aspect = _make_real_aspect("real_ruby_lint")

real_html_template_format_aspect = _real_html_template_format_aspect

real_html_template_lint_aspect = _real_html_template_lint_aspect

real_xml_lint_aspect = _real_xml_lint_aspect

real_yaml_format_aspect = _real_yaml_format_aspect

real_yaml_lint_aspect = _real_yaml_lint_aspect

real_js_lint_aspect = _make_real_aspect("real_js_lint")

real_js_format_aspect = _make_real_aspect("real_js_format")

real_python_lint_aspect = _make_real_aspect("real_python_lint")

real_jvm_lint_aspect = _make_real_aspect("real_jvm_lint")

real_jvm_format_aspect = _make_real_aspect("real_jvm_format")

real_jvm_typecheck_aspect = _make_real_aspect("real_jvm_typecheck")

real_rust_lint_aspect = _make_real_aspect("real_rust_lint")

real_rust_format_aspect = _make_real_aspect("real_rust_format")

real_js_typecheck_aspect = _make_real_aspect("real_js_typecheck")
real_rust_typecheck_aspect = _make_real_aspect("real_rust_typecheck")

real_scala_format_aspect = _make_real_aspect("real_scala_format")

real_scala_lint_aspect = _make_real_aspect("real_scala_lint")

real_shell_lint_aspect = _real_shell_lint_aspect

real_shell_format_aspect = _real_shell_format_aspect

real_dockerfile_lint_aspect = _real_dockerfile_lint_aspect
real_make_format_aspect = _real_make_format_aspect
real_make_lint_aspect = _real_make_lint_aspect

real_nix_format_aspect = _real_nix_format_aspect

real_nix_lint_aspect = _real_nix_lint_aspect

real_ocaml_format_aspect = _make_real_aspect("real_ocaml_format")

real_ocaml_typecheck_aspect = _make_real_aspect("real_ocaml_typecheck")

real_ocaml_lint_aspect = _make_real_aspect("real_ocaml_lint")

real_ansible_lint_aspect = _real_ansible_lint_aspect

real_cloudformation_lint_aspect = _real_cloudformation_lint_aspect

real_powershell_format_aspect = _make_real_aspect("real_powershell_format")

real_powershell_lint_aspect = _make_real_aspect("real_powershell_lint")

real_protobuf_format_aspect = _make_real_aspect("real_protobuf_format")

real_capnp_lint_aspect = _make_real_aspect("real_capnp_lint")
real_capnp_typecheck_aspect = _make_real_aspect("real_capnp_typecheck")
real_flatbuffers_lint_aspect = _make_real_aspect("real_flatbuffers_lint")
real_flatbuffers_typecheck_aspect = _make_real_aspect("real_flatbuffers_typecheck")
real_avro_lint_aspect = _make_real_aspect("real_avro_lint")
real_avro_typecheck_aspect = _make_real_aspect("real_avro_typecheck")
real_thrift_lint_aspect = _make_real_aspect("real_thrift_lint")
real_thrift_typecheck_aspect = _make_real_aspect("real_thrift_typecheck")
real_protobuf_lint_aspect = _make_real_aspect("real_protobuf_lint")

real_go_format_aspect = _make_real_aspect("real_go_format")

real_go_lint_aspect = _make_real_aspect("real_go_lint")

real_go_typecheck_aspect = _make_real_aspect("real_go_typecheck")

real_fortran_format_aspect = _make_real_aspect("real_fortran_format")
real_fortran_lint_aspect = _make_real_aspect("real_fortran_lint")

real_fortran_typecheck_aspect = _make_real_aspect("real_fortran_typecheck")

real_typst_format_aspect = _real_typst_format_aspect

real_typst_lint_aspect = _real_typst_lint_aspect

real_latex_format_aspect = _real_latex_format_aspect

real_cmake_format_aspect = _real_cmake_format_aspect

real_cmake_lint_aspect = _real_cmake_lint_aspect
real_haskell_format_aspect = _make_real_aspect("real_haskell_format")
real_haskell_lint_aspect = _make_real_aspect("real_haskell_lint")
real_latex_lint_aspect = _real_latex_lint_aspect
real_haskell_typecheck_aspect = _make_real_aspect("real_haskell_typecheck")

real_text_lint_aspect = _real_text_lint_aspect

real_lint_probe_aspect = _make_real_probe_aspect("lint")

real_format_probe_aspect = _make_real_probe_aspect("format")

real_typecheck_probe_aspect = _make_real_probe_aspect("typecheck")

_RUST_DOCS_TOOLCHAIN = str(Label("@rules_rust//rust:toolchain_type"))
_CC_DOCS_TOOLCHAIN = "@bazel_tools//tools/cpp:toolchain_type"

def _rust_docs_shard(target, ctx):
    if _rust_common.crate_info not in target or DocsShardInfo in target:
        return None
    crate = target[_rust_common.crate_info]
    if crate.type not in ("rlib", "lib") or crate.is_test:
        return None
    if not ctx.toolchains[_RUST_DOCS_TOOLCHAIN] or not ctx.toolchains[_CC_DOCS_TOOLCHAIN]:
        return None
    return rust_shard_action(ctx, target, "", stem = "_dx_docs/" + ctx.label.name).shard

def _docs_site_groups(target, ctx):
    tree = target[DocsSiteInfo].tree
    manifest = ctx.actions.declare_file(ctx.label.name + ".dx_docs_site")
    ctx.actions.write(manifest, tree.path)
    return {"dx_docs_sites": [manifest], "dx_results": [tree]}

def _docs_api_groups(target):
    api = target[DocsApiInfo]
    return {"dx_results": [api.api, api.records, api.summary, api.tree]}

def _docs_shard_groups(target):
    return {"dx_results": [target[DocsShardInfo].shard]}

def _real_docs_aspect_impl(target, ctx):
    parts = []
    if DocsSiteInfo in target:
        parts.append(_docs_site_groups(target, ctx))
    if DocsApiInfo in target:
        parts.append(_docs_api_groups(target))
    if DocsShardInfo in target:
        parts.append(_docs_shard_groups(target))
    rust_shard = _rust_docs_shard(target, ctx)
    if rust_shard:
        parts.append({"dx_results": [rust_shard], "dx_docs_shards": [rust_shard]})
    if not parts:
        return []
    groups = {}
    for part in parts:
        for name, files in part.items():
            groups[name] = groups.get(name, []) + files
    return [OutputGroupInfo(**{name: depset(files) for name, files in groups.items()})]

real_docs_aspect = aspect(
    implementation = _real_docs_aspect_impl,
    doc = "Puts the docs sites, shards and API trees of a target in dx_results and a manifest naming each site tree in dx_docs_sites. Rust library shards go in dx_docs_shards.",
    attrs = {
        "_error_format": attr.label(default = Label("@rules_rust//rust/settings:error_format")),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/rust/extract:rust_extract"),
            executable = True,
        ),
        "_process_wrapper": attr.label(
            default = Label("@rules_rust//util/process_wrapper"),
            cfg = "exec",
            executable = True,
        ),
    },
    fragments = ["cpp"],
    requires = [rust_doc_inputs_aspect],
    toolchains = [
        config_common.toolchain_type(_RUST_DOCS_TOOLCHAIN, mandatory = False),
        config_common.toolchain_type(_CC_DOCS_TOOLCHAIN, mandatory = False),
    ],
)

real_cue_format_aspect = _real_cue_format_aspect

real_cue_typecheck_aspect = _real_cue_typecheck_aspect

real_pkl_format_aspect = _real_pkl_format_aspect

real_pkl_typecheck_aspect = _real_pkl_typecheck_aspect

real_jsonnet_format_aspect = _real_jsonnet_format_aspect

real_jsonnet_lint_aspect = _real_jsonnet_lint_aspect

real_rego_format_aspect = _real_rego_format_aspect

real_rego_lint_aspect = _real_rego_lint_aspect

real_rego_typecheck_aspect = _real_rego_typecheck_aspect

real_bicep_format_aspect = _real_bicep_format_aspect

real_bicep_lint_aspect = _real_bicep_lint_aspect

real_terraform_format_aspect = _real_terraform_format_aspect

real_terraform_lint_aspect = _real_terraform_lint_aspect

real_terraform_typecheck_aspect = _real_terraform_typecheck_aspect

real_lua_format_aspect = _real_lua_format_aspect

real_lua_lint_aspect = _real_lua_lint_aspect

real_lua_typecheck_aspect = _real_lua_typecheck_aspect

real_php_lint_aspect = _real_php_lint_aspect

real_php_typecheck_aspect = _real_php_typecheck_aspect

real_kubernetes_lint_aspect = _real_kubernetes_lint_aspect

real_sql_format_aspect = _real_sql_format_aspect

real_sql_lint_aspect = _real_sql_lint_aspect
