"""Target-scoped real capability aspects over real adapters."""

load("@aspect_rules_py//py:defs.bzl", _PyInfo = "PyInfo")
load("@bazel_skylib//rules:common_settings.bzl", "BuildSettingInfo")
load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_dotnet//dotnet/private:providers.bzl", "DotnetAssemblyRuntimeInfo")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_powershell//powershell:defs.bzl", _PwshInfo = "PwshInfo")
load("@rules_proto//proto:defs.bzl", "ProtoInfo")
load("@rules_rust//rust:defs.bzl", "rust_clippy_aspect", _rust_common = "rust_common")
load("@rules_scala//scala:semanticdb_provider.bzl", "SemanticdbInfo")
load("//fsharp/rules:symbols.bzl", "fsharp_compile_defines")
load("//helm/rules:providers.bzl", "HelmChartInfo")
load("//libs/starlark:resources.bzl", "DX_QUALITY_TIERS", "dx_quality_resource_set")
load(
    "//quality:adapters.bzl",
    "REAL_ADAPTERS",
    "REAL_CLASS_TO_FAMILY",
)
load("//quality:dart_context.bzl", "DART_ANALYZE", "DART_FORMAT", "dart_external_root", "dart_package_entries", "dart_pubspec_stubs")
load("//quality:execution_requirements.bzl", "dx_stage_execution_requirements")
load("//quality:native_config.bzl", "DxNativeConfigInfo", "collect_native_configs", "native_config_scratch_rel", "missing_required_config_error")
load("//quality:parity_tests.bzl", "deferred_pipeline_error")
load("//quality:pipeline.bzl", "aspect_capability_blocked", "aspect_direct_maps", "aspect_family_selections", "drop_pipeline_tool", "filter_pipeline_by_tools", "generated_source_paths", "ordered_pipeline_paths", "pipeline_inputs_for_paths", "prune_tool_generated_sources", "real_request_doc", "real_request_mapping", "real_request_stage", "real_request_tool", "real_request_tool_env", "real_request_tool_file", "resolve_pipeline")
load("//quality:policy.bzl", "QualityPolicyInfo", "family_section_error")
load("//quality:clang_syntax.bzl", "ClangSyntaxInfo", "clang_syntax_aspect")
load("//quality:roslyn.bzl", "RoslynInfo", "roslyn_aspect")
load("//quality:tsc.bzl", "TscInfo", "tsc_aspect")
load("//dart/rules:providers.bzl", "DartAnalyzableInfo", "DartInfo")
load("//erlang/rules:providers.bzl", "BeamInfo", "DialyzerToolInfo")
load("//flatbuffers/rules:defs.bzl", "FlatBuffersInfo")
load("//php/rules:php.bzl", "staged_path")
load("//php/rules:providers.bzl", "PhpInfo")
load("//quality:sources.bzl", "QualitySourcesInfo", "QualityToolsInfo", "check_direct_sources", "upstream_source_class")
load("//quality:support.bzl", "support_unpinned_tools")
load("//rego/rules:providers.bzl", "RegoInfo")
load("//rust/rules:edition.bzl", "RUST_EDITION")
load("//rust/toolchains:bindings.bzl", "rust_toolchain_rustc", "rust_toolchain_toolchains", "rust_toolchain_tools")
load("//sql/rules:providers.bzl", "SqlDialectInfo")

_REAL_TOOL_TABLE = {
    "biome": {
        "attr": attr.label(default = "@dx_tools//:biome", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format", "lint"],
        "kind": "file",
        "resources": "light",
        "shard": "core",
    },
    "buf": {
        "attr": attr.label(default = "@dx_tools//:buf", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format", "lint"],
        "env": [["PATH", "/usr/bin:/bin"]],
        "kind": "file",
        "resources": "light",
        "shard": "protobuf",
    },
    "buildifier": {
        "attr": attr.label(default = "@dx_tools//:buildifier", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format", "lint"],
        "kind": "file",
        "resources": "light",
        "shard": "core",
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
        "shard": "core",
    },
    "csharpier": {
        "attr": attr.label(default = "//quality/tools/dotnet:csharpier", cfg = "exec", executable = True),
        "capabilities": ["format"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "dotnet",
    },
    "cue": {
        "attr": attr.label(default = "@dx_tools//:cue", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format", "typecheck"],
        "env": [["CUE_REGISTRY", "none"]],
        "kind": "file",
        "resources": "light",
        "shard": "cue",
    },
    "opa": {
        "attr": attr.label(default = "//config:tool_opa", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format", "typecheck"],
        "kind": "file",
        "resources": "light",
        "shard": "rego",
    },
    "regal": {
        "attr": attr.label(default = "//config:tool_regal", allow_single_file = True, cfg = "exec"),
        "capabilities": ["lint"],
        "env": [["REGAL_DISABLE_VERSION_CHECK", "1"]],
        "kind": "file",
        "resources": "light",
        "shard": "rego",
    },
    "dialyzer": {
        "attr": attr.label(default = "//config:tool_dialyzer", allow_files = True, cfg = "exec"),
        "capabilities": ["typecheck"],
        "fallback": "=@rules_dx//erlang/tools:dialyzer",
        "kind": "setting",
        "resources": "heavy",
        "shard": "core",
    },
    "phpcs": {
        "attr": attr.label(default = "//config:tool_phpcs", allow_files = True, cfg = "exec"),
        "capabilities": ["lint"],
        "fallback": "=@rules_dx//php/tools:phpcs",
        "kind": "setting",
        "resources": "medium",
        "shard": "core",
    },
    "phpstan": {
        "attr": attr.label(default = "//config:tool_phpstan", allow_files = True, cfg = "exec"),
        "capabilities": ["typecheck"],
        "fallback": "=@rules_dx//php/tools:phpstan",
        "kind": "setting",
        "resources": "heavy",
        "shard": "core",
    },
    "elvis": {
        "attr": attr.label(default = "//config:tool_elvis", allow_files = True, cfg = "exec"),
        "capabilities": ["lint"],
        "fallback": "=@rules_dx//erlang/tools:elvis",
        "kind": "setting",
        "resources": "medium",
        "shard": "core",
    },
    "erlfmt": {
        "attr": attr.label(default = "//config:tool_erlfmt", allow_files = True, cfg = "exec"),
        "capabilities": ["format"],
        "fallback": "=@rules_dx//erlang/tools:erlfmt",
        "kind": "setting",
        "resources": "medium",
        "shard": "core",
    },
    "deadnix": {
        "attr": attr.label(default = "//quality/tools/rust:deadnix", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "light",
        "shard": "nix",
    },
    "dart": {
        "attr": attr.label(default = "//config:tool_dart", allow_files = True, cfg = "exec"),
        "capabilities": ["format", "lint", "typecheck"],
        "kind": "setting",
        "resources": "medium",
        "shard": "core",
        "target_tool": True,
    },
    "dfmt": {
        "attr": attr.label(default = "//config:tool_dfmt", allow_files = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "setting",
        "resources": "light",
        "shard": "core",
        "target_tool": True,
    },
    "dscanner": {
        "attr": attr.label(default = "//config:tool_dscanner", allow_files = True, cfg = "exec"),
        "capabilities": ["lint"],
        "kind": "setting",
        "resources": "medium",
        "shard": "core",
        "target_tool": True,
    },
    "djlint": {
        "attr": attr.label(default = "//quality/tools/python:djlint", cfg = "exec", executable = True),
        "capabilities": ["format", "lint"],
        "kind": "executable",
        "resources": "medium",
        "runfiles_env": True,
        "shard": "html_template",
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
    "flatc": {
        "attr": attr.label(default = "//config:tool_flatc", allow_files = True, cfg = "exec", executable = True),
        "capabilities": ["lint", "typecheck"],
        "fallback": " to a flatc binary",
        "kind": "setting",
        "resources": "light",
        "shard": "flatbuffers",
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
    "hadolint": {
        "attr": attr.label(default = "//config:tool_hadolint", allow_single_file = True, cfg = "exec"),
        "capabilities": ["lint"],
        "kind": "file",
        "resources": "light",
        "shard": "dockerfile",
    },
    "ansible_lint": {
        "attr": attr.label(default = "//quality/tools/python:ansible_lint", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "runfiles_env": True,
        "shard": "ansible",
    },
    "cfn_lint": {
        "attr": attr.label(default = "//quality/tools/python:cfn_lint", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "medium",
        "runfiles_env": True,
        "shard": "cloudformation",
    },
    "helm": {
        "attr": attr.label(default = "//config:tool_helm", allow_single_file = True, cfg = "exec"),
        "capabilities": ["lint"],
        "kind": "file",
        "resources": "light",
        "shard": "kubernetes",
    },
    "kubeconform": {
        "attr": attr.label(default = "//config:tool_kubeconform", allow_single_file = True, cfg = "exec"),
        "capabilities": ["lint"],
        "kind": "file",
        "resources": "light",
        "shard": "kubernetes",
    },
    "checkmake": {
        "attr": attr.label(default = "@dx_tools//:checkmake", allow_single_file = True, cfg = "exec"),
        "capabilities": ["lint"],
        "kind": "file",
        "resources": "light",
        "shard": "make",
    },
    "fortitude": {
        "attr": attr.label(default = "@dx_tools//:fortitude", allow_single_file = True, cfg = "exec"),
        "capabilities": ["lint"],
        "kind": "file",
        "resources": "light",
        "shard": "fortran",
    },
    "jsonnetfmt": {
        "attr": attr.label(default = "@dx_tools//:jsonnetfmt", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "file",
        "resources": "light",
        "shard": "jsonnet",
    },
    "jsonnet_lint": {
        "attr": attr.label(default = "@dx_tools//:jsonnet_lint", allow_single_file = True, cfg = "exec"),
        "capabilities": ["lint"],
        "kind": "file",
        "resources": "light",
        "shard": "jsonnet",
    },
    "keep_sorted": {
        "attr": attr.label(default = "@dx_tools//:keep_sorted", allow_single_file = True, cfg = "exec"),
        "capabilities": ["lint"],
        "kind": "file",
        "resources": "light",
        "shard": "text",
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
    "markdown_check": {
        "attr": attr.label(default = "//quality/markdown:quality_markdown", allow_single_file = True, cfg = "exec"),
        "capabilities": ["lint"],
        "kind": "file",
        "resources": "light",
        "shard": "core",
    },
    "mix_format": {
        "attr": attr.label(default = "//config:tool_mix_format", allow_files = True, cfg = "exec"),
        "capabilities": ["format"],
        "fallback": "=@rules_dx//elixir/tools:mix_format",
        "kind": "setting",
        "resources": "medium",
        "shard": "core",
    },
    "nixfmt": {
        "attr": attr.label(default = "//config:tool_nixfmt", allow_files = True, cfg = "exec"),
        "capabilities": ["format"],
        "fallback": "=<label of a nixfmt executable>",
        "kind": "file",
        "resources": "light",
        "shard": "nix",
    },
    "notebook_outputs": {
        "attr": attr.label(default = "//quality/notebook:quality_notebook", allow_single_file = True, cfg = "exec"),
        "capabilities": ["lint"],
        "kind": "file",
        "resources": "light",
        "shard": "core",
    },
    "hlint": {
        "attr": attr.label(default = "//config:tool_hlint", allow_files = True, cfg = "exec"),
        "capabilities": ["lint"],
        "fallback": "=<label of an hlint executable>",
        "kind": "file",
        "resources": "medium",
        "shard": "haskell",
    },
    "ormolu": {
        "attr": attr.label(default = "//config:tool_ormolu", allow_files = True, cfg = "exec"),
        "capabilities": ["format"],
        "fallback": "=<label of an ormolu executable>",
        "kind": "file",
        "resources": "medium",
        "shard": "haskell",
    },
    "perlcritic": {
        "attr": attr.label(default = "//config:tool_perlcritic", allow_files = True, cfg = "exec"),
        "capabilities": ["lint"],
        "kind": "setting",
        "resources": "light",
        "shard": "core",
        "target_tool": True,
    },
    "perltidy": {
        "attr": attr.label(default = "//config:tool_perltidy", allow_files = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "setting",
        "resources": "light",
        "shard": "core",
        "target_tool": True,
    },
    "pkl": {
        "attr": attr.label(default = "//config:tool_pkl", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "file",
        "resources": "medium",
        "shard": "pkl",
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
        "shard": "core",
    },
    "qmllint": {
        "attr": attr.label(default = "//config:tool_qmllint", allow_files = True, cfg = "exec"),
        "capabilities": ["lint", "typecheck"],
        "fallback": "=@dx_qt//:qmllint",
        "kind": "setting",
        "resources": "medium",
        "shard": "core",
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
    "roslyn": {
        "capabilities": ["lint"],
        "kind": "upstream",
        "resources": "heavy",
        "shard": "dotnet",
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
        "shard": "core",
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
    "shellcheck": {
        "attr": attr.label(default = "@dx_tools//:shellcheck", allow_single_file = True, cfg = "exec"),
        "capabilities": ["lint"],
        "kind": "file",
        "resources": "light",
        "shard": "shell",
    },
    "shfmt": {
        "attr": attr.label(default = "@dx_tools//:shfmt", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "file",
        "resources": "light",
        "shard": "shell",
    },
    "spotbugs": {
        "attr": attr.label(default = "//quality/tools/jvm:spotbugs", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "heavy",
        "shard": "jvm",
    },
    "statix": {
        "attr": attr.label(default = "//quality/tools/rust:statix", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "light",
        "shard": "nix",
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
    "gersemi": {
        "attr": attr.label(default = "//config:tool_gersemi", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "file",
        "resources": "light",
        "shard": "cmake",
    },
    "stylua": {
        "attr": attr.label(default = "//config:tool_stylua", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "file",
        "resources": "light",
        "shard": "lua",
    },
    "taplo": {
        "attr": attr.label(default = "@dx_tools//:taplo", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format", "lint"],
        "kind": "file",
        "resources": "light",
        "shard": "core",
    },
    "terraform": {
        "attr": attr.label(default = "//config:tool_terraform", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format"],
        "env": [["CHECKPOINT_DISABLE", "1"], ["TF_IN_AUTOMATION", "1"], ["TF_INPUT", "0"]],
        "kind": "file",
        "resources": "light",
        "shard": "terraform",
    },
    "tflint": {
        "attr": attr.label(default = "@dx_tools//:tflint", allow_single_file = True, cfg = "exec"),
        "capabilities": ["lint"],
        "env": [["CHECKPOINT_DISABLE", "1"]],
        "kind": "file",
        "resources": "light",
        "shard": "terraform",
    },
    "ty": {
        "attr": attr.label(default = "@dx_tools//:ty", allow_single_file = True, cfg = "exec"),
        "capabilities": ["typecheck"],
        "kind": "file",
        "resources": "medium",
        "shard": "core",
    },
    "typstyle": {
        "attr": attr.label(default = "@dx_tools//:typstyle", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "file",
        "resources": "light",
        "shard": "typst",
    },
    "latexindent": {
        "attr": attr.label(default = "@dx_tools//:latexindent", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "file",
        "resources": "light",
        "shard": "latex",
    },
    "vale": {
        "attr": attr.label(default = "@dx_tools//:vale", allow_single_file = True, cfg = "exec"),
        "capabilities": ["lint"],
        "kind": "file",
        "resources": "light",
        "shard": "core",
    },
    "yamlfmt": {
        "attr": attr.label(default = "@dx_tools//:yamlfmt", allow_single_file = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "file",
        "resources": "light",
        "shard": "yaml",
    },
    "yamllint": {
        "attr": attr.label(default = "//quality/tools/python:yamllint", cfg = "exec", executable = True),
        "capabilities": ["lint"],
        "kind": "executable",
        "resources": "medium",
        "runfiles_env": True,
        "shard": "yaml",
    },
    "julia_format": {
        "attr": attr.label(default = "//config:tool_julia_format", allow_files = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "setting",
        "resources": "medium",
        "shard": "core",
        "target_tool": True,
    },
    "zig_fmt": {
        "attr": attr.label(default = "//config:tool_zig_fmt", allow_files = True, cfg = "exec"),
        "capabilities": ["format"],
        "kind": "setting",
        "resources": "light",
        "shard": "core",
        "target_tool": True,
    },
}

_REAL_TOOL_KINDS = ["executable", "file", "setting", "toolchain", "upstream"]

_REAL_TOOLCHAIN_FILES = ["clippy_driver", "rustc", "rustfmt"]

def _shard_tools(shard, capability):
    """Lists wired tools for one shard and capability, or for every capability when capability is None."""
    return sorted([tool for tool in _REAL_TOOL_TABLE if _REAL_TOOL_TABLE[tool]["shard"] == shard and (capability == None or capability in _REAL_TOOL_TABLE[tool]["capabilities"])])

_WIRED_TOOLS = {
    capability: sorted([tool for tool in _REAL_TOOL_TABLE if capability in _REAL_TOOL_TABLE[tool]["capabilities"]])
    for capability in ["format", "lint", "typecheck"]
}

def real_wired_tools():
    """Maps each capability to the sorted tool names the table wires for it."""
    return {capability: list(tools) for capability, tools in _WIRED_TOOLS.items()}

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
        if not f.is_source or f.extension != "rs" or f.owner.workspace_name != "":
            continue
        if f.path in seen:
            continue
        seen[f.path] = True
        added.append(f)
    if added:
        merged[class_id] = merged.get(class_id, []) + added
    return {k: depset(v) for k, v in merged.items()}

def real_policy_error(policy, capability, what):
    """Validates the selected workspace policy for one real capability."""
    unpinned = support_unpinned_tools(capability)
    for family_id in sorted(policy.families.keys()):
        section = policy.families[family_id]
        error = family_section_error(
            family_id,
            capability,
            getattr(section, capability),
            section.disabled,
            _WIRED_TOOLS[capability],
            unpinned,
        )
        if error != "":
            return what + ": " + error
    return ""

def upstream_direct_sources(target, ctx):
    """Derives directly owned sources for an ordinary upstream native target."""
    is_rust = _rust_common.crate_info in target or _rust_common.test_crate_info in target
    is_cc = CcInfo in target
    is_proto = ProtoInfo in target
    if not is_rust and not is_cc and not is_proto:
        return (False, {}, "")
    files = []
    if is_proto:
        proto_srcs = getattr(ctx.rule.files, "srcs", None)
        if proto_srcs == None:
            return (True, {}, "upstream proto target has no readable srcs; unsupported ownership is unchecked, not clean")
        files.extend(proto_srcs)
    if is_rust:
        rust_srcs = getattr(ctx.rule.files, "srcs", None)
        if rust_srcs == None:
            return (True, {}, "upstream rust target has no readable srcs; unsupported ownership is unchecked, not clean")
        files.extend(rust_srcs)
    if is_cc:
        cc_srcs = getattr(ctx.rule.files, "srcs", None)
        cc_hdrs = getattr(ctx.rule.files, "hdrs", None)
        cc_non_arc_srcs = getattr(ctx.rule.files, "non_arc_srcs", None)
        if cc_srcs == None and cc_hdrs == None and cc_non_arc_srcs == None:
            return (True, {}, "upstream cc target has no readable srcs, hdrs or non_arc_srcs; unsupported ownership is unchecked, not clean")
        if cc_srcs != None:
            files.extend(cc_srcs)
        if cc_hdrs != None:
            files.extend(cc_hdrs)
        if cc_non_arc_srcs != None:
            files.extend(cc_non_arc_srcs)
    buckets = {}
    for f in files:
        if not f.is_source:
            continue
        class_id = upstream_source_class(f.extension)
        if class_id == "":
            continue
        buckets.setdefault(class_id, []).append(f)
    direct_sources = {}
    for class_id in buckets.keys():
        if len(buckets[class_id]) > 0:
            direct_sources[class_id] = depset(buckets[class_id])
    check_direct_sources(direct_sources, str(target.label))
    return (True, direct_sources, "")

_QML_CONTEXT_EXTENSIONS = ["qml", "qmltypes", "js", "mjs"]

QmlContextInfo = provider(
    doc = "QML import context files reachable through deps.",
    fields = {"files": "depset[File]: qmldir, qml, qmltypes and js files."},
)

def _qml_context_impl(target, ctx):
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

def _helm_context(target, ctx, what):
    """Returns the chart context file for one helm_chart target."""
    if HelmChartInfo not in target:
        fail(what + ": helm needs a helm_chart target")
    chart = target[HelmChartInfo]
    context = ctx.actions.declare_file(target.label.name + "-helm-context.json")
    ctx.actions.write(
        output = context,
        content = json.encode({
            "chart_root": chart.chart_root,
            "namespace": chart.namespace,
            "release": chart.release_name,
            "schema_version": 1,
            "values": [f.short_path for f in chart.values],
        }),
    )
    return context
_DART_PACKAGE_CONFIG_REL = ".dart_tool/package_config.json"
_DART_OPTIONS_REL = "analysis_options.yaml"

def _dart_info(target, what):
    if DartInfo in target:
        return target[DartInfo]
    if DartAnalyzableInfo in target:
        return target[DartAnalyzableInfo].dart_info
    fail(what + ": dart needs a dart_library, dart_binary or dart_test target")

def _dart_context(target, ctx, what, capability):
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

# Where each wrapper-carried tool comes from: (wrappers, defs.bzl, tool label).
_TARGET_TOOL_OWNERS = {
    "dart": ("dart_library, dart_binary or dart_test", "@rules_dx//dart/rules:defs.bzl", "@rules_dx//dart/tools:dart"),
    "dfmt": ("d_library, d_binary or d_test", "@rules_dx//d/rules:defs.bzl", "@rules_dx//d/tools:dfmt"),
    "dscanner": ("d_library, d_binary or d_test", "@rules_dx//d/rules:defs.bzl", "@rules_dx//d/tools:dscanner"),
    "zig_fmt": ("zig_library, zig_binary or zig_test", "@rules_dx//zig/rules:defs.bzl", "@rules_dx//zig/tools:zig"),
    "perlcritic": ("perl_library, perl_binary or perl_test", "@rules_dx//perl/rules:defs.bzl", "@rules_dx//perl/tools:perlcritic"),
    "perltidy": ("perl_library, perl_binary or perl_test", "@rules_dx//perl/rules:defs.bzl", "@rules_dx//perl/tools:perltidy"),
    "julia_format": ("julia_library, julia_binary or julia_test", "@rules_dx//julia/rules:defs.bzl", "@rules_dx//julia/tools:julia_format"),
}

# Converters Vale shells out to, by source class: (tool name, owner rule, defs.bzl).
# The owner rule carries the converter in QualityToolsInfo, so Markdown-only targets
# never load Python or Node.
_VALE_CONVERTERS = {
    "rst": ("rst2html", "rst_library", "@rules_dx//rst/rules:defs.bzl"),
}

def _vale_converters(target, stage_classes, what):
    """Returns (name, FilesToRunProvider) for each converter the stage's classes need."""
    found = []
    for cls in sorted(_VALE_CONVERTERS.keys()):
        if cls not in stage_classes:
            continue
        name, owner, defs = _VALE_CONVERTERS[cls]
        if QualityToolsInfo not in target or name not in target[QualityToolsInfo].tools:
            fail(what + ": vale: " + cls + " files need " + name + "; own them with " + owner + " from " + defs)
        found.append((name, target[QualityToolsInfo].tools[name]))
    return found

def _tool_files_to_run(target, flag, tool, what):
    """Returns the flag's tool, else the one the target carries."""
    ftr = flag[DefaultInfo].files_to_run
    if ftr != None and ftr.executable != None:
        return ftr
    if QualityToolsInfo in target and tool in target[QualityToolsInfo].tools:
        return target[QualityToolsInfo].tools[tool]
    wrappers, defs, tool_label = _TARGET_TOOL_OWNERS[tool]
    fail(what + ": " + tool + " has no executable; use " + wrappers + " from " + defs + ", or set --@rules_dx//config:tool_" + tool + "=" + tool_label)

_GO_TOOLCHAIN_TYPE = "@rules_go//go:toolchain"

def go_lang_version(sdk_version):
    """Returns the gofumpt -lang value (go<major>.<minor>) for one Go SDK version."""
    parts = sdk_version.removeprefix("go").split(".")
    minor = ""
    if len(parts) >= 2:
        for c in parts[1].elems():
            if not c.isdigit():
                break
            minor += c
    if len(parts) < 2 or not parts[0].isdigit() or minor == "":
        fail("gofumpt: cannot derive -lang from Go SDK version '" + sdk_version + "'")
    return "go" + parts[0] + "." + minor

def _gofumpt_lang(ctx, what):
    """Derives gofumpt -lang from the resolved rules_go SDK."""
    toolchain = ctx.toolchains[_GO_TOOLCHAIN_TYPE]
    if toolchain == None:
        fail(what + ": gofumpt needs a registered rules_go Go toolchain to derive -lang")
    return go_lang_version(toolchain.sdk.version)

def _unique(items):
    """Returns items without repeats, in first-seen order."""
    return list({item: True for item in items}.keys())

def _real_tool_launch(target, ctx, stage_tools, delegated, has_rust_toolchain, what):
    """Resolves each launched tool's binary, extra inputs, run tools and env from the tool table."""
    binaries = {}
    extra = {}
    run_tools = []
    env = {}
    for tool in _unique(stage_tools):
        spec = _REAL_TOOL_TABLE[tool]
        kind = spec["kind"]
        if kind == "upstream" or tool in delegated:
            continue
        if kind == "toolchain":
            if has_rust_toolchain:
                binaries[tool] = _rust_toolchain_file(ctx, spec["toolchain_file"])
            continue
        if kind == "file" and "fallback" in spec:
            # An optional pin: the hub resolves an empty file set on platforms without an artifact.
            files = getattr(ctx.files, "_" + tool)
            if len(files) != 1:
                fail(what + ": " + tool + " has no pinned artifact for this execution platform; set --@rules_dx//config:tool_" + tool + spec["fallback"])
            binaries[tool] = files[0]
        elif kind == "file":
            binaries[tool] = getattr(ctx.file, "_" + tool)
        elif kind == "executable":
            tool_target = getattr(ctx.attr, "_" + tool)
            binaries[tool] = getattr(ctx.executable, "_" + tool)
            if spec.get("extra") == "files":
                extra[tool] = tool_target[DefaultInfo].files.to_list()
            elif spec.get("extra") == "runfiles":
                extra[tool] = tool_target[DefaultInfo].default_runfiles.files.to_list()
            if spec.get("run_tool", True):
                run_tools.append(tool_target[DefaultInfo].files_to_run)
        elif spec.get("target_tool", False):
            ftr = _tool_files_to_run(target, getattr(ctx.attr, "_" + tool), tool, what)
            binaries[tool] = ftr.executable
            run_tools.append(ftr)
        else:
            ftr = getattr(ctx.attr, "_" + tool)[DefaultInfo].files_to_run
            if ftr == None or ftr.executable == None:
                fail(what + ": " + tool + " has no executable; set --@rules_dx//config:tool_" + tool + spec["fallback"])
            binaries[tool] = ftr.executable
            run_tools.append(ftr)
        pairs = [real_request_tool_env(pair[0], pair[1]) for pair in spec.get("env", [])]
        if spec.get("runfiles_env", False):
            pairs.append(real_request_tool_env("RUNFILES_DIR", ctx.executable._runner.path + ".runfiles"))
        if len(pairs) > 0:
            env[tool] = pairs
    return struct(binaries = binaries, extra = extra, run_tools = run_tools, env = env)

def _rust_toolchain_file(ctx, name):
    """Returns one tool file from the resolved Rust toolchains."""
    if name == "rustc":
        return rust_toolchain_rustc(ctx)
    clippy_driver, rustfmt = rust_toolchain_tools(ctx)
    return clippy_driver if name == "clippy_driver" else rustfmt

def _real_resolved_stages(target, ctx, capability, what):
    """Resolves one target's pipeline stages for both the real shards and the probe."""
    policy = ctx.attr._policy[QualityPolicyInfo]
    policy_error = real_policy_error(policy, capability, what)
    if policy_error != "":
        fail(policy_error)
    if aspect_capability_blocked(ctx.rule.attr, capability):
        return ([], None, None)
    if target.label.workspace_name != "":
        return ([], None, None)
    if QualitySourcesInfo in target:
        direct_sources = target[QualitySourcesInfo].direct_sources
    else:
        # The dx wrapper's native "<name>_upstream" target shares its sources with the
        # forwarder, which is the sole origin; skip it to avoid duplicate findings.
        if target.label.name.endswith("_upstream"):
            return ([], None, None)
        (recognized, derived, ownership_error) = upstream_direct_sources(target, ctx)
        if ownership_error != "":
            fail(what + ": " + ownership_error)
        if not recognized:
            return ([], None, None)
        direct_sources = derived
    if inherits_test_crate_sources(capability):
        harness = None
        if _rust_common.test_crate_info in target:
            harness = target[_rust_common.test_crate_info].crate
        elif _rust_common.crate_info in target and target[_rust_common.crate_info].is_test:
            harness = target[_rust_common.crate_info]
        if harness != None:
            direct_sources = merge_inherited_sources(direct_sources, harness.srcs.to_list(), "rust")
    if len(direct_sources) == 0:
        return ([], None, None)

    (target_classes, direct_files, direct_paths, path_to_file) = aspect_direct_maps(direct_sources, what)
    selections = aspect_family_selections(policy, capability)
    resolved = resolve_pipeline(
        target_classes,
        direct_paths,
        capability,
        selections,
        REAL_CLASS_TO_FAMILY,
        REAL_ADAPTERS,
    )
    if len(resolved) == 0:
        err = deferred_pipeline_error(target_classes, capability)
        if err != "":
            fail(what + ": " + err)
    return (resolved, direct_files, path_to_file)

def _real_pipeline_action(target, ctx, capability, allowed_tools, output_suffix, has_rust_toolchain):
    what = "real_aspect (" + str(target.label) + ")"
    (resolved, direct_files, path_to_file) = _real_resolved_stages(target, ctx, capability, what)
    if len(resolved) == 0:
        return []
    resolved = filter_pipeline_by_tools(resolved, allowed_tools)
    if len(resolved) == 0:
        return []

    spotbugs_owned = []
    spotbugs_aux = []
    if "spotbugs" in [stage["tool"] for stage in resolved]:
        if JavaInfo not in target:
            resolved = drop_pipeline_tool(resolved, "spotbugs")
            if len(resolved) == 0:
                return []
        else:
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
                resolved = drop_pipeline_tool(resolved, "spotbugs")
                if len(resolved) == 0:
                    return []
            else:
                spotbugs_owned = sorted(owned.values(), key = lambda f: f.short_path)
                spotbugs_aux = sorted(auxiliary.values(), key = lambda f: f.short_path)

    dialyzer_owned = []
    dialyzer_aux = []
    dialyzer_plts = []
    dialyzer_code = []
    if "dialyzer" in [stage["tool"] for stage in resolved]:
        if BeamInfo not in target or len(target[BeamInfo].owned.to_list()) == 0:
            resolved = drop_pipeline_tool(resolved, "dialyzer")
            if len(resolved) == 0:
                return []
        else:
            tool_target = getattr(ctx.attr, "_dialyzer")
            if DialyzerToolInfo not in tool_target:
                fail(what + ": dialyzer needs PLTs; set --@rules_dx//config:tool_dialyzer=@rules_dx//erlang/tools:dialyzer")
            # The tool's PLTs come first; a language's own follow, without repeating one.
            seen_plts = {}
            for plt in tool_target[DialyzerToolInfo].plts + target[BeamInfo].plts:
                if plt.path not in seen_plts:
                    seen_plts[plt.path] = True
                    dialyzer_plts.append(plt)
            dialyzer_code = sorted(target[BeamInfo].code, key = lambda f: f.basename)
            dialyzer_owned = sorted(target[BeamInfo].owned.to_list(), key = lambda f: f.short_path)
            owned_paths = {f.path: True for f in dialyzer_owned}
            dialyzer_aux = [f for f in sorted(target[BeamInfo].deps.to_list(), key = lambda f: f.short_path) if f.path not in owned_paths]

    phpstan_scan = []
    if "phpstan" in [stage["tool"] for stage in resolved]:
        if PhpInfo not in target:
            resolved = drop_pipeline_tool(resolved, "phpstan")
            if len(resolved) == 0:
                return []
        else:
            # Dependencies are scanned for their symbols, never analysed or run; the owned
            # sources are the CLI paths, so they stay out of the scan set.
            owned_paths = {f.path: True for f in path_to_file.values()}
            info = target[PhpInfo]
            seen_scan = {}
            for scan_file in sorted(info.transitive_srcs.to_list() + info.autoload_files.to_list(), key = lambda f: f.path):
                if scan_file.path not in owned_paths and scan_file.path not in seen_scan:
                    seen_scan[scan_file.path] = True
                    phpstan_scan.append(("dx-phpstan/scan/" + staged_path(scan_file), scan_file))
            for pkg_file in sorted(info.package_files.to_list(), key = lambda f: f.path):
                if pkg_file.path.endswith(".php") and pkg_file.path not in owned_paths and pkg_file.path not in seen_scan:
                    seen_scan[pkg_file.path] = True
                    phpstan_scan.append(("dx-phpstan/pkg/" + staged_path(pkg_file), pkg_file))

    scalafix_owned = []
    scalafix_aux = []
    if "scalafix" in [stage["tool"] for stage in resolved]:
        (scalafix_owned, scalafix_aux) = _scalafix_context(target, what)

    fsharplint_refs = []
    fsharplint_context = None
    if "fsharplint" in [stage["tool"] for stage in resolved]:
        (fsharplint_refs, fsharplint_context) = _fsharplint_context(target, ctx, what)

    helm_context = None
    if "helm" in [stage["tool"] for stage in resolved]:
        helm_context = _helm_context(target, ctx, what)
    dart_package_config = None
    dart_options = None
    dart_stubs = []
    if "dart" in [stage["tool"] for stage in resolved]:
        (dart_package_config, dart_options, dart_stubs) = _dart_context(target, ctx, what, capability)

    tsc = None
    if "tsc" in [stage["tool"] for stage in resolved]:
        if TscInfo not in target:
            resolved = drop_pipeline_tool(resolved, "tsc")
            if len(resolved) == 0:
                return []
        else:
            tsc = target[TscInfo]

    roslyn = None
    if "roslyn" in [stage["tool"] for stage in resolved] or "csc" in [stage["tool"] for stage in resolved]:
        if RoslynInfo not in target:
            fail(what + ": roslyn and csc need a csharp_* target compiled with the SDK analyzers")
        roslyn = target[RoslynInfo]

    clang_syntax = None
    if "clang_syntax" in [stage["tool"] for stage in resolved] or "clang_tidy" in [stage["tool"] for stage in resolved]:
        if ClangSyntaxInfo not in target:
            fail(what + ": clang_syntax and clang_tidy need a cc_* or objc_* target with compile actions (a header-only library needs --features=parse_headers)")
        clang_syntax = target[ClangSyntaxInfo]

    sql_dialect = None
    if "prettier" in [stage["tool"] for stage in resolved] and SqlDialectInfo in target:
        sql_dialect = target[SqlDialectInfo].dialect

    rustfmt_edition = None
    if "rustfmt" in [stage["tool"] for stage in resolved]:
        if not has_rust_toolchain:
            fail(what + ": rustfmt needs the Rust family aspect")
        if _rust_common.crate_info in target:
            rustfmt_edition = target[_rust_common.crate_info].edition
        elif _rust_common.test_crate_info in target:
            rustfmt_edition = target[_rust_common.test_crate_info].crate.edition
        else:
            rustfmt_edition = RUST_EDITION
        generated = generated_source_paths(direct_files)
        if len(generated) > 0:
            resolved = prune_tool_generated_sources(resolved, generated, "rustfmt")
            if len(resolved) == 0:
                return []

    clippy_diagnostics = []
    if "clippy" in [stage["tool"] for stage in resolved]:
        if not has_rust_toolchain:
            fail(what + ": clippy needs the Rust family aspect")
        if OutputGroupInfo in target and "clippy_output" in target[OutputGroupInfo]:
            clippy_diagnostics = target[OutputGroupInfo]["clippy_output"].to_list()
    clippy_delegated = len(clippy_diagnostics) > 0

    rustc_diagnostics = []
    if "rustc" in [stage["tool"] for stage in resolved]:
        if not has_rust_toolchain:
            fail(what + ": rustc needs the Rust family aspect")
        if OutputGroupInfo in target:
            group = rustc_diagnostics_group(target[OutputGroupInfo])
            if group:
                rustc_diagnostics = target[OutputGroupInfo][group].to_list()
    rustc_delegated = len(rustc_diagnostics) > 0

    hints = []
    if hasattr(ctx.rule.attr, "aspect_hints"):
        for hint_target in ctx.rule.attr.aspect_hints:
            if DxNativeConfigInfo in hint_target:
                hints.append(hint_target[DxNativeConfigInfo])
    stage_tools = [stage["tool"] for stage in resolved]
    configs_by_tool = collect_native_configs(hints, stage_tools, str(target.label))

    for stage in resolved:
        config_error = missing_required_config_error(stage["tool"], capability, configs_by_tool, str(target.label))
        if config_error != "":
            fail(config_error)

    delegated = {}
    if clippy_delegated:
        delegated["clippy"] = True
    if rustc_delegated:
        delegated["rustc"] = True
    launch = _real_tool_launch(target, ctx, stage_tools, delegated, has_rust_toolchain, what)
    tool_binaries = launch.binaries
    tool_extra = launch.extra

    out = ctx.actions.declare_file(target.label.name + "-real-" + capability + output_suffix + ".pb")

    ordered_paths = ordered_pipeline_paths(resolved)
    inputs = pipeline_inputs_for_paths(ordered_paths, path_to_file)

    sibling_pairs = {}
    if "markdown_check" in stage_tools and hasattr(ctx.rule.attr, "markdown_siblings"):
        for sibling in ctx.rule.attr.markdown_siblings:
            if hasattr(sibling, "files"):
                sibling_files = sibling.files.to_list()
            else:
                sibling_files = [sibling]
            for f in sibling_files:
                if ".." in f.short_path.split("/"):
                    fail(what + ": sibling path escapes workspace: '" + f.short_path + "'")
                if f.short_path in path_to_file:
                    fail(what + ": sibling '" + f.short_path + "' shadows a checked source; siblings must not shadow sources")
                if f.short_path in sibling_pairs:
                    fail(what + ": duplicate sibling '" + f.short_path + "'; siblings must be unique")
                sibling_pairs[f.short_path] = f

    request_stages = [real_request_stage(stage["tool"], stage["classes"], stage["sources"]) for stage in resolved]
    request_sources = []
    for ws_path in ordered_paths:
        f = path_to_file.get(ws_path)
        if f != None:
            request_sources.append(real_request_mapping(ws_path, f.path))
    request_siblings = []
    for ws_path in sorted(sibling_pairs.keys()):
        f = sibling_pairs[ws_path]
        request_siblings.append(real_request_mapping(ws_path, f.path))
        inputs.append(f)

    resolve_pairs = {}
    if "ty" in stage_tools and _PyInfo in target:
        transitive = getattr(target[_PyInfo], "transitive_sources", None)
        if transitive != None:
            for f in transitive.to_list():
                ws_path = f.short_path

                if ".." in ws_path.split("/"):
                    continue
                if ws_path in path_to_file or ws_path in sibling_pairs or ws_path in resolve_pairs:
                    continue
                if not ws_path.endswith(".py") and not ws_path.endswith(".pyi"):
                    continue
                resolve_pairs[ws_path] = f
    ty_search_dirs = []
    if "ty" in stage_tools and _PyInfo in target:
        transitive_dirs = getattr(target[_PyInfo], "transitive_sources", None)
        if transitive_dirs != None:
            for f in transitive_dirs.to_list():
                if f.is_directory and f.short_path.endswith(".install"):
                    ty_search_dirs.append(f)
                    inputs.append(f)
    if "qmllint" in stage_tools:
        context_files = list(target[DefaultInfo].files.to_list())
        if QmlContextInfo in target:
            context_files.extend(target[QmlContextInfo].files.to_list())
        for f in context_files:
            ws_path = f.short_path
            if ws_path.startswith("../"):
                ws_path = "_external/" + ws_path[3:]
            if ".." in ws_path.split("/"):
                continue
            if ws_path in path_to_file or ws_path in sibling_pairs or ws_path in resolve_pairs:
                continue
            if f.basename != "qmldir" and f.extension not in _QML_CONTEXT_EXTENSIONS:
                continue
            resolve_pairs[ws_path] = f
    if "dart" in stage_tools and capability != "format":
        for f in _dart_info(target, what).transitive_srcs.to_list():
            ws_path = f.short_path
            if ws_path.startswith("../"):
                ws_path = dart_external_root(ws_path)
            if ".." in ws_path.split("/") or f.extension != "dart":
                continue
            if ws_path in path_to_file or ws_path in sibling_pairs or ws_path in resolve_pairs:
                continue
            resolve_pairs[ws_path] = f
        for (stub_path, _) in dart_stubs:
            if stub_path in path_to_file or stub_path in sibling_pairs or stub_path in resolve_pairs:
                fail(what + ": generated pubspec.yaml stub collides with the staged source " + stub_path)
    if "cue" in stage_tools:
        for f in target[DefaultInfo].files.to_list():
            ws_path = f.short_path
            if ".." in ws_path.split("/") or f.extension != "cue":
                continue
            if ws_path in path_to_file or ws_path in sibling_pairs or ws_path in resolve_pairs:
                continue
            resolve_pairs[ws_path] = f
    if "opa" in stage_tools and RegoInfo in target:
        for f in target[RegoInfo].transitive_srcs.to_list():
            ws_path = f.short_path
            if ".." in ws_path.split("/") or f.extension != "rego":
                continue
            if ws_path in path_to_file or ws_path in sibling_pairs or ws_path in resolve_pairs:
                continue
            resolve_pairs[ws_path] = f
    if "jsonnetfmt" in stage_tools or "jsonnet_lint" in stage_tools:
        for f in target[DefaultInfo].files.to_list():
            ws_path = f.short_path
            if ".." in ws_path.split("/") or f.extension not in ["jsonnet", "libsonnet", "json"]:
                continue
            if ws_path in path_to_file or ws_path in sibling_pairs or ws_path in resolve_pairs:
                continue
            resolve_pairs[ws_path] = f
    if "redocly" in stage_tools:
        for f in target[DefaultInfo].files.to_list():
            ws_path = f.short_path
            if ".." in ws_path.split("/") or f.extension not in ["yaml", "yml", "json"]:
                continue
            if ws_path in path_to_file or ws_path in sibling_pairs or ws_path in resolve_pairs:
                continue
            resolve_pairs[ws_path] = f
    if "flatc" in stage_tools and FlatBuffersInfo in target:
        for f in target[FlatBuffersInfo].transitive_srcs.to_list():
            ws_path = f.short_path
            if ws_path.startswith("../"):
                ws_path = "_external/" + ws_path[3:]
            if ".." in ws_path.split("/"):
                continue
            if ws_path in path_to_file or ws_path in sibling_pairs or ws_path in resolve_pairs:
                continue
            resolve_pairs[ws_path] = f
    if "buf" in stage_tools and ProtoInfo in target:
        proto_roots = sorted([r for r in target[ProtoInfo].transitive_proto_path.to_list() if r != "."], key = lambda r: -len(r))
        for f in target[ProtoInfo].transitive_sources.to_list():
            ws_path = f.path
            for root in proto_roots:
                if ws_path.startswith(root + "/"):
                    ws_path = ws_path[len(root) + 1:]
                    break
            if ".." in ws_path.split("/"):
                continue
            if ws_path in path_to_file or ws_path in sibling_pairs or ws_path in resolve_pairs:
                continue
            resolve_pairs[ws_path] = f
    if "helm" in stage_tools and HelmChartInfo in target:
        for f in target[HelmChartInfo].dependencies:
            ws_path = f.short_path
            if ".." in ws_path.split("/") or ws_path.startswith("../"):
                continue
            if ws_path in path_to_file or ws_path in sibling_pairs or ws_path in resolve_pairs:
                continue
            resolve_pairs[ws_path] = f
    pwsh_module_roots = {}
    if "psscriptanalyzer" in stage_tools:
        context_files = list(target[DefaultInfo].files.to_list())
        upstream = getattr(ctx.rule.attr, "upstream", None)
        if type(upstream) == "Target" and _PwshInfo in upstream:
            context_files.extend(upstream[DefaultInfo].default_runfiles.files.to_list())
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
                pwsh_module_roots[module_dir.rpartition("/")[0] or "."] = True
            if ws_path in path_to_file or ws_path in sibling_pairs or ws_path in resolve_pairs:
                continue
            resolve_pairs[ws_path] = f
    request_resolves = []
    for ws_path in sorted(resolve_pairs.keys()):
        f = resolve_pairs[ws_path]
        request_resolves.append(real_request_mapping(ws_path, f.path))
        inputs.append(f)

    tool_env_entries = launch.env
    run_tools = launch.run_tools
    if "psscriptanalyzer" in tool_env_entries:
        tool_env_entries["psscriptanalyzer"].append(real_request_tool_env("DX_PWSH_MODULE_ROOTS", "|".join(sorted(pwsh_module_roots.keys()))))

    vale_converters = []
    for stage in resolved:
        if stage["tool"] == "vale":
            vale_converters = _vale_converters(target, stage["classes"], what)
    for _name, converter in vale_converters:
        run_tools = run_tools + [converter]

    request_tools = {}
    for tool in stage_tools:
        binary_path = None
        if not (tool == "clippy" and clippy_delegated) and not (tool == "rustc" and rustc_delegated) and _REAL_TOOL_TABLE[tool]["kind"] != "upstream":
            binary = tool_binaries[tool]
            binary_path = binary.path
            inputs.append(binary)
            inputs.extend(tool_extra.get(tool, []))
        tool_files = []
        if tool == "spotbugs":
            for jar in spotbugs_owned:
                tool_files.append(real_request_tool_file("dx-spotbugs/owned/" + jar.short_path, jar.path))
                inputs.append(jar)
            for index, jar in enumerate(spotbugs_aux):
                tool_files.append(real_request_tool_file("dx-spotbugs/aux/%d-%s" % (index, jar.basename), jar.path))
                inputs.append(jar)
        if tool == "dialyzer":
            for index, plt in enumerate(dialyzer_plts):
                tool_files.append(real_request_tool_file("dx-dialyzer/plt/%s-%s" % (("000" + str(index))[-3:], plt.basename), plt.path))
                inputs.append(plt)
            for index, beam in enumerate(dialyzer_owned):
                tool_files.append(real_request_tool_file("dx-dialyzer/owned/%d/%s" % (index, beam.basename), beam.path))
                inputs.append(beam)
            for beam in dialyzer_code:
                tool_files.append(real_request_tool_file("dx-dialyzer/pa/" + beam.basename, beam.path))
                inputs.append(beam)
            seen_aux = {}
            for index, beam in enumerate(dialyzer_aux):
                if beam.path in seen_aux:
                    continue
                seen_aux[beam.path] = True
                tool_files.append(real_request_tool_file("dx-dialyzer/aux/%d/%s" % (index, beam.basename), beam.path))
                inputs.append(beam)
        if tool == "phpstan":
            for (rel, scan_file) in phpstan_scan:
                tool_files.append(real_request_tool_file(rel, scan_file.path))
                inputs.append(scan_file)
        if tool == "scalafix":
            for index, jar in enumerate(scalafix_owned):
                tool_files.append(real_request_tool_file("dx-scalafix/owned/%d-%s" % (index, jar.basename), jar.path))
                inputs.append(jar)
            for index, jar in enumerate(scalafix_aux):
                tool_files.append(real_request_tool_file("dx-scalafix/aux/%d-%s" % (index, jar.basename), jar.path))
                inputs.append(jar)
        if tool == "fsharplint":
            for index, lib in enumerate(fsharplint_refs):
                tool_files.append(real_request_tool_file("dx-fsharplint/ref/%d-%s" % (index, lib.basename), lib.path))
                inputs.append(lib)
            tool_files.append(real_request_tool_file("dx-fsharplint/context.json", fsharplint_context.path))
            inputs.append(fsharplint_context)
        path_tools = []
        if tool == "helm":
            tool_files.append(real_request_tool_file("dx-helm/context.json", helm_context.path))
            inputs.append(helm_context)
            path_tools.append({"binary": ctx.file._kubeconform.path, "name": "kubeconform"})
            inputs.append(ctx.file._kubeconform)
        if tool == "vale":
            for name, converter in vale_converters:
                path_tools.append({"binary": converter.executable.path, "name": name})
                inputs.append(converter.executable)
        edition = None
        if tool == "rustfmt":
            edition = rustfmt_edition
        if tool == "gofumpt":
            edition = _gofumpt_lang(ctx, what)
        config_rel = None
        if tool in configs_by_tool:
            hint = configs_by_tool[tool]
            config_rel = native_config_scratch_rel(hint.config.short_path)
            if config_rel == None:
                fail("native_config (%s): config %s for %s escapes the scratch" % (tool, hint.config.short_path, what))
            for f in sorted(hint.closure.to_list(), key = lambda f: f.short_path):
                closure_rel = native_config_scratch_rel(f.short_path)
                if closure_rel == None:
                    fail("native_config (%s): file %s for %s escapes the scratch" % (tool, f.short_path, what))
                tool_files.append(real_request_tool_file(closure_rel, f.path))
                inputs.append(f)
        if tool == "dart":
            # dart format reads `formatter:` from the nearest analysis_options.yaml, so the
            # hinted config (or an empty one) sits at the scratch root and nowhere else.
            options = dart_options
            if tool in configs_by_tool:
                options = configs_by_tool[tool].config
            else:
                inputs.append(options)
            if config_rel != _DART_OPTIONS_REL:
                tool_files.append(real_request_tool_file(_DART_OPTIONS_REL, options.path))
            tool_files.append(real_request_tool_file(_DART_PACKAGE_CONFIG_REL, dart_package_config.path))
            inputs.append(dart_package_config)
            for (stub_path, stub) in dart_stubs:
                if stub_path == config_rel:
                    fail(what + ": generated pubspec.yaml stub collides with the config " + stub_path)
                tool_files.append(real_request_tool_file(stub_path, stub.path))
                inputs.append(stub)
        upstream = []
        if tool == "clippy" and clippy_delegated:
            for diagnostics in clippy_diagnostics:
                upstream.append(diagnostics.path)
                inputs.append(diagnostics)
        if tool == "rustc" and rustc_delegated:
            for diagnostics in rustc_diagnostics:
                upstream.append(diagnostics.path)
                inputs.append(diagnostics)
        if tool == "tsc":
            upstream.append(tsc.stdout.path)
            inputs.append(tsc.stdout)
            tool_files.append(real_request_tool_file("dx-tsc/exit", tsc.status.path))
            inputs.append(tsc.status)
        if tool == "roslyn" or tool == "csc":
            for i in range(len(roslyn.tfms)):
                upstream.append(roslyn.sarifs[i].path)
                inputs.append(roslyn.sarifs[i])
                tool_files.append(real_request_tool_file("dx-roslyn/" + roslyn.tfms[i] + "/exit", roslyn.statuses[i].path))
                inputs.append(roslyn.statuses[i])
        if tool == "clang_tidy":
            for diagnostics in clang_syntax.tidy_diagnostics:
                upstream.append(diagnostics.path)
                inputs.append(diagnostics)
        if tool == "clang_syntax":
            for i in range(len(clang_syntax.diagnostics)):
                upstream.append(clang_syntax.diagnostics[i].path)
                inputs.append(clang_syntax.diagnostics[i])
                stem = clang_syntax.diagnostics[i].basename.rpartition(".")[0]
                tool_files.append(real_request_tool_file("dx-clang-syntax/" + stem + "/exit", clang_syntax.statuses[i].path))
                inputs.append(clang_syntax.statuses[i])
        request_tools[tool] = real_request_tool(
            binary_path = binary_path,
            config_rel = config_rel,
            edition = edition,
            dialect = sql_dialect if tool == "prettier" else None,
            files = tool_files,
            env = tool_env_entries.get(tool, []),
            upstream = upstream,
            search_paths = [f.path for f in ty_search_dirs] if tool == "ty" else [],
            path_tools = path_tools,
        )

    doc = real_request_doc(
        str(target.label),
        capability,
        request_stages,
        request_sources,
        request_siblings,
        request_resolves,
        request_tools,
    )
    request_file = ctx.actions.declare_file(target.label.name + "-real-" + capability + output_suffix + "-request.json")
    ctx.actions.write(output = request_file, content = json.encode(doc))
    inputs.append(request_file)

    args = ctx.actions.args()
    args.add("--request", request_file.path)
    args.add("--output", out.path)

    ctx.actions.run(
        executable = ctx.executable._runner,
        inputs = depset(inputs),
        tools = run_tools,
        outputs = [out],
        arguments = [args],
        execution_requirements = dx_stage_execution_requirements(stage_tools, capability),
        mnemonic = "DxRealQuality" + capability.capitalize(),
        resource_set = real_quality_resource_set(sorted(tool_binaries.keys())),
        progress_message = "Dx real quality " + capability + " %{label}",
    )

    return [OutputGroupInfo(dx_results = depset([out]))]

def _make_real_impl(capability, allowed_tools, output_suffix, has_rust_toolchain):
    """Makes one shard impl over the shared real pipeline action."""

    def _impl(target, ctx):
        return _real_pipeline_action(target, ctx, capability, allowed_tools, output_suffix, has_rust_toolchain)

    return _impl

def real_quality_resource_set(tools):
    """Returns the resource_set callback for the costliest tier among the launched tools."""
    return dx_quality_resource_set([_REAL_TOOL_TABLE[tool]["resources"] for tool in tools])

def real_tool_table_shape_error(table):
    """Returns the first broken shape rule in a real tool table, else an empty string."""
    for tool in sorted(table.keys()):
        spec = table[tool]
        prefix = "real aspects: table tool '" + tool + "' "
        if tool not in REAL_ADAPTERS:
            return prefix + "is outside REAL_ADAPTERS"
        if spec.get("resources") not in DX_QUALITY_TIERS:
            return prefix + "names unknown resources tier '" + str(spec.get("resources")) + "'"
        for capability in spec["capabilities"]:
            if capability not in REAL_ADAPTERS[tool]:
                return prefix + "names unwired capability '" + capability + "'"
        kind = spec.get("kind")
        if kind not in _REAL_TOOL_KINDS:
            return prefix + "names unknown kind '" + str(kind) + "'"
        if kind in ["executable", "file", "setting"] and "attr" not in spec:
            return prefix + "needs an attr for kind '" + kind + "'"
        if kind not in ["executable", "file", "setting"] and "attr" in spec:
            return prefix + "must not set an attr for kind '" + kind + "'"
        if kind == "setting" and ("fallback" in spec) == ("target_tool" in spec):
            return prefix + "needs exactly one of fallback or target_tool"
        if kind not in ["setting", "file"] and "fallback" in spec:
            return prefix + "sets fallback outside the setting and file kinds"
        if kind != "setting" and "target_tool" in spec:
            return prefix + "sets target_tool outside the setting kind"
        if spec.get("target_tool", False) and tool not in _TARGET_TOOL_OWNERS:
            return prefix + "has no wrapper owner in _TARGET_TOOL_OWNERS"
        if kind != "executable" and ("extra" in spec or "run_tool" in spec or "runfiles_env" in spec):
            return prefix + "sets extra, run_tool or runfiles_env outside the executable kind"
        if spec.get("extra", "files") not in ["files", "runfiles"]:
            return prefix + "names unknown extra '" + str(spec.get("extra")) + "'"
        if (kind == "toolchain") != ("toolchain_file" in spec) or spec.get("toolchain_file", "rustc") not in _REAL_TOOLCHAIN_FILES:
            return prefix + "needs a known toolchain_file exactly when its kind is toolchain"
        for pair in spec.get("env", []):
            if type(pair) != "list" or len(pair) != 2 or type(pair[0]) != "string" or type(pair[1]) != "string":
                return prefix + "has an env entry that is not a pair of strings"
    return ""

def real_tool_table_error():
    """Validates the real tool table and that a built shard runs every tool capability."""
    error = real_tool_table_shape_error(_REAL_TOOL_TABLE)
    if error != "":
        return error
    wired = real_wired_capabilities()
    for tool in sorted(_REAL_TOOL_TABLE.keys()):
        if wired.get(tool, []) != sorted(_REAL_TOOL_TABLE[tool]["capabilities"]):
            return "real aspects: table tool '" + tool + "' has a capability no shard runs"
    return ""

_REAL_BASE_ATTRS = {
    "_policy": attr.label(
        default = "//config:workspace",
        providers = [QualityPolicyInfo],
    ),
    "_runner": attr.label(
        default = "//quality/runner:quality_runner",
        executable = True,
        cfg = "exec",
        allow_files = True,
    ),
}

def _real_attrs_for(tools):
    """Builds one shard attr set from the attrs in the tool table."""
    return _REAL_BASE_ATTRS | {"_" + tool: _REAL_TOOL_TABLE[tool]["attr"] for tool in tools if "attr" in _REAL_TOOL_TABLE[tool]}

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
        "requires": [clang_syntax_aspect],
        "shard": "cc",
        "suffix": "-cc",
    },
    "real_cc_typecheck": {
        "capability": "typecheck",
        "doc": "Additive C/C++/Objective-C typecheck family aspect (compiler -fsyntax-only replay).",
        "requires": [clang_syntax_aspect],
        "shard": "cc",
        "suffix": "-cc",
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
        "doc": "Additive C# typecheck family aspect (csc compiler diagnostics).",
        "requires": [roslyn_aspect],
        "shard": "dotnet",
        "suffix": "-dotnet",
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
    "real_go_format": {
        "capability": "format",
        "doc": "Additive Go format family aspect (gofumpt).",
        "shard": "go",
        "suffix": "-go",
        "toolchains": [config_common.toolchain_type(_GO_TOOLCHAIN_TYPE, mandatory = False)],
    },
    "real_html_template_format": {
        "capability": "format",
        "doc": "Additive HTML template format family aspect (djLint).",
        "shard": "html_template",
        "suffix": "-html-template",
    },
    "real_html_template_lint": {
        "capability": "lint",
        "doc": "Additive HTML template lint family aspect (djLint).",
        "shard": "html_template",
        "suffix": "-html-template",
    },
    "real_yaml_format": {
        "capability": "format",
        "doc": "Additive YAML format family aspect (yamlfmt).",
        "shard": "yaml",
        "suffix": "-yaml",
    },
    "real_yaml_lint": {
        "capability": "lint",
        "doc": "Additive YAML lint family aspect (yamllint).",
        "shard": "yaml",
        "suffix": "-yaml",
    },
    "real_format": {
        "capability": "format",
        "doc": "Registers the exact-input real format pipeline action in dx_results.",
        "shard": "core",
        "shared_attrs": True,
        "suffix": "",
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
        "doc": "Additive TypeScript typecheck family aspect (delegated tsc over typescript_project).",
        "requires": [tsc_aspect],
        "shard": "js",
        "suffix": "-js",
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
    "real_lint": {
        "capability": "lint",
        "doc": "Registers the exact-input real lint pipeline action in dx_results.",
        "required_aspect_providers": [[QmlContextInfo]],
        "requires": [_qml_context_aspect],
        "shard": "core",
        "shared_attrs": True,
        "suffix": "",
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
    "real_shell_lint": {
        "capability": "lint",
        "doc": "Additive shell lint family aspect (ShellCheck).",
        "shard": "shell",
        "suffix": "-shell",
    },
    "real_shell_format": {
        "capability": "format",
        "doc": "Additive shell format family aspect (shfmt).",
        "shard": "shell",
        "suffix": "-shell",
    },
    "real_dockerfile_lint": {
        "capability": "lint",
        "doc": "Additive Dockerfile lint family aspect (hadolint).",
        "shard": "dockerfile",
        "suffix": "-dockerfile",
    },
    "real_nix_format": {
        "capability": "format",
        "doc": "Additive Nix format family aspect (nixfmt).",
        "shard": "nix",
        "suffix": "-nix",
    },
    "real_nix_lint": {
        "capability": "lint",
        "doc": "Additive Nix lint family aspect (statix, deadnix).",
        "shard": "nix",
        "suffix": "-nix",
    },
    "real_ansible_lint": {
        "capability": "lint",
        "doc": "Additive Ansible lint family aspect (ansible-lint).",
        "shard": "ansible",
        "suffix": "-ansible",
    },
    "real_cloudformation_lint": {
        "capability": "lint",
        "doc": "Additive CloudFormation lint family aspect (cfn-lint).",
        "shard": "cloudformation",
        "suffix": "-cloudformation",
    },
    "real_kubernetes_lint": {
        "capability": "lint",
        "doc": "Additive Kubernetes lint family aspect (kubeconform, helm).",
        "shard": "kubernetes",
        "suffix": "-kubernetes",
    },
    "real_make_lint": {
        "capability": "lint",
        "doc": "Additive Makefile lint family aspect (checkmake).",
        "shard": "make",
        "suffix": "-make",
    },
    "real_cue_format": {
        "capability": "format",
        "doc": "Additive CUE format family aspect (cue fmt).",
        "shard": "cue",
        "suffix": "-cue",
    },
    "real_cue_typecheck": {
        "capability": "typecheck",
        "doc": "Additive CUE typecheck family aspect (cue vet -c=false).",
        "shard": "cue",
        "suffix": "-cue",
    },
    "real_rego_format": {
        "capability": "format",
        "doc": "Additive Rego format family aspect (opa fmt).",
        "shard": "rego",
        "suffix": "-rego",
    },
    "real_rego_lint": {
        "capability": "lint",
        "doc": "Additive Rego lint family aspect (regal lint).",
        "shard": "rego",
        "suffix": "-rego",
    },
    "real_rego_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Rego typecheck family aspect (opa check).",
        "shard": "rego",
        "suffix": "-rego",
    },
    "real_jsonnet_format": {
        "capability": "format",
        "doc": "Additive Jsonnet format family aspect (jsonnetfmt).",
        "shard": "jsonnet",
        "suffix": "-jsonnet",
    },
    "real_fortran_lint": {
        "capability": "lint",
        "doc": "Additive Fortran lint family aspect (fortitude).",
        "shard": "fortran",
        "suffix": "-fortran",
    },
    "real_jsonnet_lint": {
        "capability": "lint",
        "doc": "Additive Jsonnet lint family aspect (jsonnet-lint).",
        "shard": "jsonnet",
        "suffix": "-jsonnet",
    },
    "real_typst_format": {
        "capability": "format",
        "doc": "Additive Typst format family aspect (typstyle).",
        "shard": "typst",
        "suffix": "-typst",
    },
    "real_latex_format": {
        "capability": "format",
        "doc": "Additive LaTeX format family aspect (latexindent).",
        "shard": "latex",
        "suffix": "-latex",
    },
    "real_cmake_format": {
        "capability": "format",
        "doc": "Additive CMake format family aspect (gersemi).",
        "shard": "cmake",
        "suffix": "-cmake",
    },
    "real_lua_format": {
        "capability": "format",
        "doc": "Additive Lua format family aspect (stylua).",
        "shard": "lua",
        "suffix": "-lua",
    },
    "real_haskell_format": {
        "capability": "format",
        "doc": "Additive Haskell format family aspect (ormolu).",
        "shard": "haskell",
        "suffix": "-haskell",
    },
    "real_haskell_lint": {
        "capability": "lint",
        "doc": "Additive Haskell lint family aspect (hlint).",
        "shard": "haskell",
        "suffix": "-haskell",
    },
    "real_pkl_format": {
        "capability": "format",
        "doc": "Additive Pkl format family aspect (pkl format).",
        "shard": "pkl",
        "suffix": "-pkl",
    },
    "real_terraform_format": {
        "capability": "format",
        "doc": "Additive Terraform format family aspect (terraform fmt).",
        "shard": "terraform",
        "suffix": "-terraform",
    },
    "real_terraform_lint": {
        "capability": "lint",
        "doc": "Additive Terraform lint family aspect (tflint).",
        "shard": "terraform",
        "suffix": "-terraform",
    },
    "real_text_lint": {
        "capability": "lint",
        "doc": "Additive text lint family aspect (keep-sorted).",
        "shard": "text",
        "suffix": "-text",
    },
    "real_typecheck": {
        "capability": "typecheck",
        "doc": "Registers the exact-input real typecheck pipeline action in dx_results.",
        "shard": "core",
        "shared_attrs": True,
        "suffix": "",
    },
}

def _expand_shard(spec):
    """Expands one shard spec into its tools, attrs and Rust toolchain flag."""
    tools = _shard_tools(spec["shard"], spec["capability"])
    attr_tools = _shard_tools(spec["shard"], None) if spec.get("shared_attrs", False) else tools
    return spec | {
        "attrs": _real_attrs_for(attr_tools) | spec.get("extra_attrs", {}),
        "has_rust": len([tool for tool in tools if _REAL_TOOL_TABLE[tool]["kind"] == "toolchain"]) > 0,
        "tools": tools,
    }

_REAL_SHARDS = {name: _expand_shard(spec) for name, spec in _REAL_SHARD_SPECS.items()}

def _make_real_aspect(shard_name):
    """Builds one shard aspect from the shard table."""
    shard = _REAL_SHARDS[shard_name]
    return aspect(
        implementation = _make_real_impl(shard["capability"], shard["tools"], shard["suffix"], shard["has_rust"]),
        doc = shard["doc"],
        attr_aspects = ["aspect_hints"],
        attrs = shard["attrs"],
        toolchains = shard.get("toolchains", []),
        requires = shard.get("requires", []),
        required_aspect_providers = shard.get("required_aspect_providers", []),
    )

RealShardProbeInfo = provider(
    doc = "Shard names the shard probe found for one target and capability.",
    fields = {
        "capability": "lint, format or typecheck",
        "shards": "sorted shard names whose tool filter keeps a resolved stage",
    },
)

def real_shard_result_stems():
    """Maps each shard's dx_results stem ("real-" + capability + suffix) to its shard name."""
    return {
        "real-" + shard["capability"] + shard["suffix"]: shard_name
        for shard_name, shard in _REAL_SHARDS.items()
    }

def real_wired_capabilities():
    """Maps each tool to the sorted capabilities a built shard aspect runs for it."""
    wired = {}
    for shard in _REAL_SHARDS.values():
        for tool in shard["tools"]:
            if shard["capability"] not in wired.setdefault(tool, []):
                wired[tool].append(shard["capability"])
    return {tool: sorted(caps) for tool, caps in wired.items()}

def _probe_shards(resolved, capability):
    """Names every shard of the capability whose tool filter keeps a stage."""
    shards = []
    for shard_name in sorted(_REAL_SHARDS.keys()):
        shard = _REAL_SHARDS[shard_name]
        if shard["capability"] != capability:
            continue
        if len(filter_pipeline_by_tools(resolved, shard["tools"])) > 0:
            shards.append(shard_name)
    return shards

def _make_real_probe_impl(capability):
    """Makes the shard probe impl for one capability."""

    def _impl(target, ctx):
        what = "real_aspect (" + str(target.label) + ")"
        (resolved, _direct_files, _path_to_file) = _real_resolved_stages(target, ctx, capability, what)
        if len(resolved) == 0:
            return []
        shards = _probe_shards(resolved, capability)
        if len(shards) == 0:
            return []
        out = ctx.actions.declare_file(target.label.name + ".dx-shards-" + capability + ".json")
        ctx.actions.write(out, json.encode({"shards": shards}))
        return [
            OutputGroupInfo(dx_shards = depset([out])),
            RealShardProbeInfo(capability = capability, shards = shards),
        ]

    return _impl

def _make_real_probe_aspect(capability):
    """Builds the tool-free shard probe aspect for one capability."""
    return aspect(
        implementation = _make_real_probe_impl(capability),
        attr_aspects = ["aspect_hints"],
        attrs = {"_policy": _REAL_BASE_ATTRS["_policy"]},
        doc = "Writes the " + capability + " shard names each target needs to the dx_shards output group.",
    )

real_lint_aspect = _make_real_aspect("real_lint")

real_format_aspect = _make_real_aspect("real_format")

real_typecheck_aspect = _make_real_aspect("real_typecheck")

real_cc_format_aspect = _make_real_aspect("real_cc_format")

real_cc_lint_aspect = _make_real_aspect("real_cc_lint")

real_cc_typecheck_aspect = _make_real_aspect("real_cc_typecheck")

real_dotnet_format_aspect = _make_real_aspect("real_dotnet_format")

real_dotnet_lint_aspect = _make_real_aspect("real_dotnet_lint")

real_dotnet_typecheck_aspect = _make_real_aspect("real_dotnet_typecheck")

real_ruby_format_aspect = _make_real_aspect("real_ruby_format")

real_ruby_lint_aspect = _make_real_aspect("real_ruby_lint")

real_html_template_format_aspect = _make_real_aspect("real_html_template_format")

real_html_template_lint_aspect = _make_real_aspect("real_html_template_lint")

real_yaml_format_aspect = _make_real_aspect("real_yaml_format")

real_yaml_lint_aspect = _make_real_aspect("real_yaml_lint")

real_js_lint_aspect = _make_real_aspect("real_js_lint")

real_js_format_aspect = _make_real_aspect("real_js_format")

real_python_lint_aspect = _make_real_aspect("real_python_lint")

real_jvm_lint_aspect = _make_real_aspect("real_jvm_lint")

real_jvm_format_aspect = _make_real_aspect("real_jvm_format")

real_rust_lint_aspect = _make_real_aspect("real_rust_lint")

real_rust_format_aspect = _make_real_aspect("real_rust_format")

real_js_typecheck_aspect = _make_real_aspect("real_js_typecheck")
real_rust_typecheck_aspect = _make_real_aspect("real_rust_typecheck")

real_scala_format_aspect = _make_real_aspect("real_scala_format")

real_scala_lint_aspect = _make_real_aspect("real_scala_lint")

real_shell_lint_aspect = _make_real_aspect("real_shell_lint")

real_shell_format_aspect = _make_real_aspect("real_shell_format")

real_dockerfile_lint_aspect = _make_real_aspect("real_dockerfile_lint")
real_make_lint_aspect = _make_real_aspect("real_make_lint")

real_nix_format_aspect = _make_real_aspect("real_nix_format")

real_nix_lint_aspect = _make_real_aspect("real_nix_lint")

real_kubernetes_lint_aspect = _make_real_aspect("real_kubernetes_lint")

real_ansible_lint_aspect = _make_real_aspect("real_ansible_lint")

real_cloudformation_lint_aspect = _make_real_aspect("real_cloudformation_lint")

real_powershell_format_aspect = _make_real_aspect("real_powershell_format")

real_powershell_lint_aspect = _make_real_aspect("real_powershell_lint")

real_protobuf_format_aspect = _make_real_aspect("real_protobuf_format")

real_flatbuffers_lint_aspect = _make_real_aspect("real_flatbuffers_lint")
real_flatbuffers_typecheck_aspect = _make_real_aspect("real_flatbuffers_typecheck")
real_protobuf_lint_aspect = _make_real_aspect("real_protobuf_lint")

real_terraform_format_aspect = _make_real_aspect("real_terraform_format")
real_terraform_lint_aspect = _make_real_aspect("real_terraform_lint")

real_go_format_aspect = _make_real_aspect("real_go_format")

real_cue_format_aspect = _make_real_aspect("real_cue_format")

real_jsonnet_format_aspect = _make_real_aspect("real_jsonnet_format")

real_jsonnet_lint_aspect = _make_real_aspect("real_jsonnet_lint")

real_fortran_lint_aspect = _make_real_aspect("real_fortran_lint")

real_typst_format_aspect = _make_real_aspect("real_typst_format")

real_latex_format_aspect = _make_real_aspect("real_latex_format")

real_lua_format_aspect = _make_real_aspect("real_lua_format")
real_cmake_format_aspect = _make_real_aspect("real_cmake_format")
real_haskell_format_aspect = _make_real_aspect("real_haskell_format")
real_haskell_lint_aspect = _make_real_aspect("real_haskell_lint")

real_cue_typecheck_aspect = _make_real_aspect("real_cue_typecheck")

real_rego_format_aspect = _make_real_aspect("real_rego_format")

real_rego_typecheck_aspect = _make_real_aspect("real_rego_typecheck")

real_rego_lint_aspect = _make_real_aspect("real_rego_lint")

real_pkl_format_aspect = _make_real_aspect("real_pkl_format")

real_text_lint_aspect = _make_real_aspect("real_text_lint")

real_lint_probe_aspect = _make_real_probe_aspect("lint")

real_format_probe_aspect = _make_real_probe_aspect("format")

real_typecheck_probe_aspect = _make_real_probe_aspect("typecheck")
