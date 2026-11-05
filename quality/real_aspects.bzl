"""Target-scoped real capability aspects over real adapters."""

load("@rules_powershell//powershell:defs.bzl", _PwshInfo = "PwshInfo")
load("@rules_py//py:defs.bzl", _PyInfo = "PyInfo")
load("@rules_go//go:def.bzl", "GoInfo")
load("@rules_java//java/common:java_info.bzl", "JavaInfo")
load("@rules_rust//rust:defs.bzl", _rust_common = "rust_common")
load("//quality:compile_check.bzl", "COMPILE_CHECK_TOOLS", "CompileCheckInfo", "compile_check_aspect")
load("//quality:families/cc.bzl", _real_cc_format_aspect = "real_cc_format_aspect", _real_cc_lint_aspect = "real_cc_lint_aspect", _real_cc_typecheck_aspect = "real_cc_typecheck_aspect", _CC_HOOKS = "HOOKS", _CC_RECOGNIZER = "RECOGNIZER", _CC_SHARD_SPECS = "SHARD_SPECS", _CC_TOOLS = "TOOLS")
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
load("//quality:families/pkl.bzl", _real_pkl_format_aspect = "real_pkl_format_aspect", _real_pkl_lint_aspect = "real_pkl_lint_aspect", _real_pkl_typecheck_aspect = "real_pkl_typecheck_aspect", _PKL_HOOKS = "HOOKS", _PKL_SHARD_SPECS = "SHARD_SPECS", _PKL_TOOLS = "TOOLS")
load("//quality:families/jsonnet.bzl", _real_jsonnet_format_aspect = "real_jsonnet_format_aspect", _real_jsonnet_lint_aspect = "real_jsonnet_lint_aspect", _JSONNET_HOOKS = "HOOKS", _JSONNET_SHARD_SPECS = "SHARD_SPECS", _JSONNET_TOOLS = "TOOLS")
load("//quality:families/rego.bzl", _real_rego_format_aspect = "real_rego_format_aspect", _real_rego_lint_aspect = "real_rego_lint_aspect", _real_rego_typecheck_aspect = "real_rego_typecheck_aspect", _REGO_HOOKS = "HOOKS", _REGO_SHARD_SPECS = "SHARD_SPECS", _REGO_TOOLS = "TOOLS")
load("//quality:families/bicep.bzl", _real_bicep_format_aspect = "real_bicep_format_aspect", _real_bicep_lint_aspect = "real_bicep_lint_aspect", _BICEP_HOOKS = "HOOKS", _BICEP_SHARD_SPECS = "SHARD_SPECS", _BICEP_TOOLS = "TOOLS")
load("//quality:families/terraform.bzl", _real_terraform_format_aspect = "real_terraform_format_aspect", _real_terraform_lint_aspect = "real_terraform_lint_aspect", _real_terraform_typecheck_aspect = "real_terraform_typecheck_aspect", _TERRAFORM_HOOKS = "HOOKS", _TERRAFORM_SHARD_SPECS = "SHARD_SPECS", _TERRAFORM_TOOLS = "TOOLS")
load("//quality:families/lua.bzl", _real_lua_format_aspect = "real_lua_format_aspect", _real_lua_lint_aspect = "real_lua_lint_aspect", _real_lua_typecheck_aspect = "real_lua_typecheck_aspect", _LUA_HOOKS = "HOOKS", _LUA_SHARD_SPECS = "SHARD_SPECS", _LUA_TOOLS = "TOOLS")
load("//quality:families/php.bzl", _real_php_lint_aspect = "real_php_lint_aspect", _real_php_typecheck_aspect = "real_php_typecheck_aspect", _PHP_HOOKS = "HOOKS", _PHP_SHARD_SPECS = "SHARD_SPECS", _PHP_TOOLS = "TOOLS")
load("//quality:families/kubernetes.bzl", _real_kubernetes_lint_aspect = "real_kubernetes_lint_aspect", _KUBERNETES_HOOKS = "HOOKS", _KUBERNETES_SHARD_SPECS = "SHARD_SPECS", _KUBERNETES_TOOLS = "TOOLS")
load("//quality:families/sql.bzl", _real_sql_format_aspect = "real_sql_format_aspect", _real_sql_lint_aspect = "real_sql_lint_aspect", _SQL_HOOKS = "HOOKS", _SQL_SHARD_SPECS = "SHARD_SPECS", _SQL_TOOLS = "TOOLS")
load("//quality:families/dart.bzl", _real_dart_format_aspect = "real_dart_format_aspect", _real_dart_lint_aspect = "real_dart_lint_aspect", _real_dart_typecheck_aspect = "real_dart_typecheck_aspect", _DART_HOOKS = "HOOKS", _DART_SHARD_SPECS = "SHARD_SPECS", _DART_TOOLS = "TOOLS")
load("//quality:families/powershell.bzl", _real_powershell_format_aspect = "real_powershell_format_aspect", _real_powershell_lint_aspect = "real_powershell_lint_aspect", _POWERSHELL_HOOKS = "HOOKS", _POWERSHELL_SHARD_SPECS = "SHARD_SPECS", _POWERSHELL_TOOLS = "TOOLS")
load("//quality:families/ruby.bzl", _real_ruby_format_aspect = "real_ruby_format_aspect", _real_ruby_lint_aspect = "real_ruby_lint_aspect", _real_ruby_typecheck_aspect = "real_ruby_typecheck_aspect", _RUBY_HOOKS = "HOOKS", _RUBY_SHARD_SPECS = "SHARD_SPECS", _RUBY_TOOLS = "TOOLS")
load("//quality:families/python.bzl", _real_python_format_aspect = "real_python_format_aspect", _real_python_lint_aspect = "real_python_lint_aspect", _real_python_typecheck_aspect = "real_python_typecheck_aspect", _PYTHON_HOOKS = "HOOKS", _PYTHON_SHARD_SPECS = "SHARD_SPECS", _PYTHON_TOOLS = "TOOLS")
load("//quality:families/jvm.bzl", _real_jvm_format_aspect = "real_jvm_format_aspect", _real_jvm_lint_aspect = "real_jvm_lint_aspect", _real_jvm_typecheck_aspect = "real_jvm_typecheck_aspect", _real_scala_format_aspect = "real_scala_format_aspect", _real_scala_lint_aspect = "real_scala_lint_aspect", _JVM_HOOKS = "HOOKS", _JVM_SHARD_SPECS = "SHARD_SPECS", _JVM_TOOLS = "TOOLS")
load("//quality:families/js.bzl", _real_js_format_aspect = "real_js_format_aspect", _real_js_lint_aspect = "real_js_lint_aspect", _real_js_typecheck_aspect = "real_js_typecheck_aspect", _JS_HOOKS = "HOOKS", _JS_SHARD_SPECS = "SHARD_SPECS", _JS_TOOLS = "TOOLS")
load("//quality:families/julia.bzl", _real_julia_format_aspect = "real_julia_format_aspect", _real_julia_lint_aspect = "real_julia_lint_aspect", _real_julia_typecheck_aspect = "real_julia_typecheck_aspect", _JULIA_HOOKS = "HOOKS", _JULIA_SHARD_SPECS = "SHARD_SPECS", _JULIA_TOOLS = "TOOLS")
load("//quality:families/dotnet.bzl", _real_dotnet_format_aspect = "real_dotnet_format_aspect", _real_dotnet_lint_aspect = "real_dotnet_lint_aspect", _real_dotnet_typecheck_aspect = "real_dotnet_typecheck_aspect", _DOTNET_HOOKS = "HOOKS", _DOTNET_SHARD_SPECS = "SHARD_SPECS", _DOTNET_TOOLS = "TOOLS")
load("//quality:families/d.bzl", _real_d_format_aspect = "real_d_format_aspect", _real_d_lint_aspect = "real_d_lint_aspect", _real_d_typecheck_aspect = "real_d_typecheck_aspect", _D_SHARD_SPECS = "SHARD_SPECS", _D_TOOLS = "TOOLS")
load("//quality:families/fortran.bzl", _real_fortran_format_aspect = "real_fortran_format_aspect", _real_fortran_lint_aspect = "real_fortran_lint_aspect", _real_fortran_typecheck_aspect = "real_fortran_typecheck_aspect", _FORTRAN_SHARD_SPECS = "SHARD_SPECS", _FORTRAN_TOOLS = "TOOLS")
load("//quality:families/haskell.bzl", _real_haskell_format_aspect = "real_haskell_format_aspect", _real_haskell_lint_aspect = "real_haskell_lint_aspect", _real_haskell_typecheck_aspect = "real_haskell_typecheck_aspect", _HASKELL_SHARD_SPECS = "SHARD_SPECS", _HASKELL_TOOLS = "TOOLS")
load("//quality:families/generation.bzl", _real_avro_lint_aspect = "real_avro_lint_aspect", _real_avro_typecheck_aspect = "real_avro_typecheck_aspect", _real_capnp_lint_aspect = "real_capnp_lint_aspect", _real_capnp_typecheck_aspect = "real_capnp_typecheck_aspect", _real_flatbuffers_lint_aspect = "real_flatbuffers_lint_aspect", _real_flatbuffers_typecheck_aspect = "real_flatbuffers_typecheck_aspect", _real_protobuf_format_aspect = "real_protobuf_format_aspect", _real_protobuf_lint_aspect = "real_protobuf_lint_aspect", _real_thrift_lint_aspect = "real_thrift_lint_aspect", _real_thrift_typecheck_aspect = "real_thrift_typecheck_aspect", _GENERATION_HOOKS = "HOOKS", _GENERATION_RECOGNIZERS = "RECOGNIZERS", _GENERATION_SHARD_SPECS = "SHARD_SPECS", _GENERATION_TOOLS = "TOOLS")
load("//quality:families/zig.bzl", _real_zig_format_aspect = "real_zig_format_aspect", _real_zig_lint_aspect = "real_zig_lint_aspect", _real_zig_typecheck_aspect = "real_zig_typecheck_aspect", _ZIG_SHARD_SPECS = "SHARD_SPECS", _ZIG_TOOLS = "TOOLS")
load("//quality:families/beam.bzl", _real_elixir_format_aspect = "real_elixir_format_aspect", _real_elixir_lint_aspect = "real_elixir_lint_aspect", _real_erlang_format_aspect = "real_erlang_format_aspect", _real_erlang_lint_aspect = "real_erlang_lint_aspect", _real_erlang_typecheck_aspect = "real_erlang_typecheck_aspect", _BEAM_HOOKS = "HOOKS", _BEAM_SHARD_SPECS = "SHARD_SPECS", _BEAM_TOOLS = "TOOLS")
load("//quality:families/go.bzl", _real_go_format_aspect = "real_go_format_aspect", _real_go_lint_aspect = "real_go_lint_aspect", _real_go_typecheck_aspect = "real_go_typecheck_aspect", _GO_HOOKS = "HOOKS", _GO_SHARD_SPECS = "SHARD_SPECS", _GO_TOOLS = "TOOLS")
load("//quality:families/rust.bzl", _real_rust_format_aspect = "real_rust_format_aspect", _real_rust_lint_aspect = "real_rust_lint_aspect", _real_rust_typecheck_aspect = "real_rust_typecheck_aspect", _RUST_HARNESS_SOURCES = "harness_sources", _RUST_HOOKS = "HOOKS", _RUST_RECOGNIZER = "RECOGNIZER", _RUST_SHARD_SPECS = "SHARD_SPECS", _RUST_TOOLCHAIN = "TOOLCHAIN", _RUST_TOOLS = "TOOLS", _inherits_test_crate_sources = "inherits_test_crate_sources", _merge_inherited_sources = "merge_inherited_sources", _rustc_diagnostics_group = "rustc_diagnostics_group")
load("//quality:families/ocaml.bzl", _real_ocaml_format_aspect = "real_ocaml_format_aspect", _real_ocaml_lint_aspect = "real_ocaml_lint_aspect", _real_ocaml_typecheck_aspect = "real_ocaml_typecheck_aspect", _OCAML_HOOKS = "HOOKS", _OCAML_SHARD_SPECS = "SHARD_SPECS", _OCAML_TOOLS = "TOOLS")
load("//quality:family_hooks.bzl", _stage = "stage", _without = "without")
load("//quality:pipeline.bzl", "drop_pipeline_tool", "real_request_tool_env")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect", "real_upstream_direct_sources", _RealShardProbeInfo = "RealShardProbeInfo", _real_policy_error = "real_policy_error", _real_quality_resource_set = "real_quality_resource_set", _real_shard_result_stems = "real_shard_result_stems", _real_tool_table_error = "real_tool_table_error", _real_tool_table_shape_error = "real_tool_table_shape_error", _real_wired_capabilities = "real_wired_capabilities", _real_wired_tools = "real_wired_tools")
load("//quality:sources.bzl", "QualityToolsInfo")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo")
load("//tools/docs/dotnet:shard.bzl", "DotnetDocsInfo", "dotnet_docs_collect_aspect", "dotnet_docs_problem", "dotnet_shard_action")
load("//tools/docs/go:defs.bzl", "go_shard_action")
load("//tools/docs/jvm:shard.bzl", "JAVA_TOOLCHAIN", "JVM_LANGUAGES", "KOTLIN_TOOLCHAIN", "jvm_shard_action")
load("//tools/docs/powershell:defs.bzl", "powershell_shard_action")
load("//tools/docs/python:shard.bzl", "python_package_name_ok", "python_shard_action")
load("//tools/docs/rust:defs.bzl", "rust_doc_inputs_aspect", "rust_shard_action")
load("//tools/docs/site:render.bzl", "DocsApiInfo", "DocsSiteInfo")

_JVM_CHECK_TOOLS = ["javac", "kotlinc", "scalac"]

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

# The core runs the hooks in this order and no other.
_HOOK_ORDER = [
    "spotbugs",
    "dialyzer",
    "xref",
    "sorbet",
    "scalafix",
    "fsharplint",
    "jet",
] + _JVM_CHECK_TOOLS + ["errcheck", "gotype", "govet", "staticcheck"] + COMPILE_CHECK_TOOLS + [
    "tsc",
    "roslyn",
    "csc",
    "fsc",
    "clang_syntax",
    "clang_tidy",
    "nvcc_syntax",
    "prettier",
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

_CONTEXT_HOOKS = _JS_HOOKS | _CORE_HOOKS | _GENERATION_HOOKS | _CUE_HOOKS | _PKL_HOOKS | _JSONNET_HOOKS | _REGO_HOOKS | _BICEP_HOOKS | _TERRAFORM_HOOKS | _LUA_HOOKS | _PHP_HOOKS | _KUBERNETES_HOOKS | _SQL_HOOKS | _DART_HOOKS | _OCAML_HOOKS | _BEAM_HOOKS | _GO_HOOKS | _POWERSHELL_HOOKS | _RUBY_HOOKS | _JULIA_HOOKS | _PYTHON_HOOKS | _CC_HOOKS | _DOTNET_HOOKS | _RUST_HOOKS | _JVM_HOOKS | {
    tool: _compile_check_hook(tool)
    for tool in COMPILE_CHECK_TOOLS
}

_CFG = real_config(
    tools = _JS_TOOLS | _CORE_TOOLS | _ANSIBLE_TOOLS | _CLOUDFORMATION_TOOLS | _MAKE_TOOLS | _DOCKERFILE_TOOLS | _HTML_TEMPLATE_TOOLS | _XML_TOOLS | _YAML_TOOLS | _NIX_TOOLS | _SHELL_TOOLS | _LATEX_TOOLS | _CMAKE_TOOLS | _TYPST_TOOLS | _TEXT_TOOLS | _SWIFT_TOOLS | _PERL_TOOLS | _CUE_TOOLS | _PKL_TOOLS | _JSONNET_TOOLS | _REGO_TOOLS | _BICEP_TOOLS | _TERRAFORM_TOOLS | _LUA_TOOLS | _PHP_TOOLS | _KUBERNETES_TOOLS | _SQL_TOOLS | _D_TOOLS | _DART_TOOLS | _OCAML_TOOLS | _BEAM_TOOLS | _GO_TOOLS | _POWERSHELL_TOOLS | _RUBY_TOOLS | _JULIA_TOOLS | _PYTHON_TOOLS | _JVM_TOOLS | _CC_TOOLS | _DOTNET_TOOLS | _RUST_TOOLS,
    shard_specs = _JS_SHARD_SPECS | _CORE_SHARD_SPECS | _ANSIBLE_SHARD_SPECS | _CLOUDFORMATION_SHARD_SPECS | _MAKE_SHARD_SPECS | _DOCKERFILE_SHARD_SPECS | _HTML_TEMPLATE_SHARD_SPECS | _XML_SHARD_SPECS | _YAML_SHARD_SPECS | _NIX_SHARD_SPECS | _SHELL_SHARD_SPECS | _LATEX_SHARD_SPECS | _CMAKE_SHARD_SPECS | _TYPST_SHARD_SPECS | _TEXT_SHARD_SPECS | _SWIFT_SHARD_SPECS | _PERL_SHARD_SPECS | _CUE_SHARD_SPECS | _PKL_SHARD_SPECS | _JSONNET_SHARD_SPECS | _REGO_SHARD_SPECS | _BICEP_SHARD_SPECS | _TERRAFORM_SHARD_SPECS | _LUA_SHARD_SPECS | _PHP_SHARD_SPECS | _KUBERNETES_SHARD_SPECS | _SQL_SHARD_SPECS | _D_SHARD_SPECS | _DART_SHARD_SPECS | _OCAML_SHARD_SPECS | _BEAM_SHARD_SPECS | _GO_SHARD_SPECS | _POWERSHELL_SHARD_SPECS | _RUBY_SHARD_SPECS | _JULIA_SHARD_SPECS | _PYTHON_SHARD_SPECS | _JVM_SHARD_SPECS | _CC_SHARD_SPECS | _DOTNET_SHARD_SPECS | _RUST_SHARD_SPECS,
    hook_order = _HOOK_ORDER,
    context_hooks = _CONTEXT_HOOKS,
    source_recognizers = _GENERATION_RECOGNIZERS + [_RUST_RECOGNIZER, _CC_RECOGNIZER],
    harness_sources = _RUST_HARNESS_SOURCES,
    toolchain = _RUST_TOOLCHAIN,
)

inherits_test_crate_sources = _inherits_test_crate_sources

merge_inherited_sources = _merge_inherited_sources

rustc_diagnostics_group = _rustc_diagnostics_group

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

real_zig_typecheck_aspect = _real_zig_typecheck_aspect

real_cc_format_aspect = _real_cc_format_aspect

real_cc_lint_aspect = _real_cc_lint_aspect

real_cc_typecheck_aspect = _real_cc_typecheck_aspect

real_d_typecheck_aspect = _real_d_typecheck_aspect

real_dotnet_format_aspect = _real_dotnet_format_aspect

real_dotnet_lint_aspect = _real_dotnet_lint_aspect

real_dotnet_typecheck_aspect = _real_dotnet_typecheck_aspect

real_dart_format_aspect = _real_dart_format_aspect

real_dart_lint_aspect = _real_dart_lint_aspect

real_dart_typecheck_aspect = _real_dart_typecheck_aspect

real_d_format_aspect = _real_d_format_aspect

real_d_lint_aspect = _real_d_lint_aspect

real_perl_format_aspect = _real_perl_format_aspect

real_perl_lint_aspect = _real_perl_lint_aspect

real_python_format_aspect = _real_python_format_aspect

real_python_typecheck_aspect = _real_python_typecheck_aspect

real_julia_format_aspect = _real_julia_format_aspect

real_swift_format_aspect = _real_swift_format_aspect

real_swift_lint_aspect = _real_swift_lint_aspect

real_zig_format_aspect = _real_zig_format_aspect

real_zig_lint_aspect = _real_zig_lint_aspect

real_julia_lint_aspect = _real_julia_lint_aspect

real_julia_typecheck_aspect = _real_julia_typecheck_aspect

real_ruby_typecheck_aspect = _real_ruby_typecheck_aspect

real_ruby_format_aspect = _real_ruby_format_aspect

real_ruby_lint_aspect = _real_ruby_lint_aspect

real_html_template_format_aspect = _real_html_template_format_aspect

real_html_template_lint_aspect = _real_html_template_lint_aspect

real_xml_lint_aspect = _real_xml_lint_aspect

real_yaml_format_aspect = _real_yaml_format_aspect

real_yaml_lint_aspect = _real_yaml_lint_aspect

real_js_lint_aspect = _real_js_lint_aspect

real_js_format_aspect = _real_js_format_aspect

real_python_lint_aspect = _real_python_lint_aspect

real_jvm_lint_aspect = _real_jvm_lint_aspect

real_jvm_format_aspect = _real_jvm_format_aspect

real_jvm_typecheck_aspect = _real_jvm_typecheck_aspect

real_rust_lint_aspect = _real_rust_lint_aspect

real_rust_format_aspect = _real_rust_format_aspect

real_js_typecheck_aspect = _real_js_typecheck_aspect
real_rust_typecheck_aspect = _real_rust_typecheck_aspect

real_scala_format_aspect = _real_scala_format_aspect

real_scala_lint_aspect = _real_scala_lint_aspect

real_shell_lint_aspect = _real_shell_lint_aspect

real_shell_format_aspect = _real_shell_format_aspect

real_dockerfile_lint_aspect = _real_dockerfile_lint_aspect
real_make_format_aspect = _real_make_format_aspect
real_make_lint_aspect = _real_make_lint_aspect

real_nix_format_aspect = _real_nix_format_aspect

real_nix_lint_aspect = _real_nix_lint_aspect

real_ocaml_format_aspect = _real_ocaml_format_aspect

real_ocaml_typecheck_aspect = _real_ocaml_typecheck_aspect

real_ocaml_lint_aspect = _real_ocaml_lint_aspect

real_erlang_format_aspect = _real_erlang_format_aspect

real_erlang_lint_aspect = _real_erlang_lint_aspect

real_erlang_typecheck_aspect = _real_erlang_typecheck_aspect

real_elixir_format_aspect = _real_elixir_format_aspect

real_elixir_lint_aspect = _real_elixir_lint_aspect

real_ansible_lint_aspect = _real_ansible_lint_aspect

real_cloudformation_lint_aspect = _real_cloudformation_lint_aspect

real_powershell_format_aspect = _real_powershell_format_aspect

real_powershell_lint_aspect = _real_powershell_lint_aspect

real_protobuf_format_aspect = _real_protobuf_format_aspect

real_capnp_lint_aspect = _real_capnp_lint_aspect
real_capnp_typecheck_aspect = _real_capnp_typecheck_aspect
real_flatbuffers_lint_aspect = _real_flatbuffers_lint_aspect
real_flatbuffers_typecheck_aspect = _real_flatbuffers_typecheck_aspect
real_avro_lint_aspect = _real_avro_lint_aspect
real_avro_typecheck_aspect = _real_avro_typecheck_aspect
real_thrift_lint_aspect = _real_thrift_lint_aspect
real_thrift_typecheck_aspect = _real_thrift_typecheck_aspect
real_protobuf_lint_aspect = _real_protobuf_lint_aspect

real_go_format_aspect = _real_go_format_aspect

real_go_lint_aspect = _real_go_lint_aspect

real_go_typecheck_aspect = _real_go_typecheck_aspect

real_fortran_format_aspect = _real_fortran_format_aspect
real_fortran_lint_aspect = _real_fortran_lint_aspect

real_fortran_typecheck_aspect = _real_fortran_typecheck_aspect

real_typst_format_aspect = _real_typst_format_aspect

real_typst_lint_aspect = _real_typst_lint_aspect

real_latex_format_aspect = _real_latex_format_aspect

real_cmake_format_aspect = _real_cmake_format_aspect

real_cmake_lint_aspect = _real_cmake_lint_aspect
real_haskell_format_aspect = _real_haskell_format_aspect
real_haskell_lint_aspect = _real_haskell_lint_aspect
real_latex_lint_aspect = _real_latex_lint_aspect
real_haskell_typecheck_aspect = _real_haskell_typecheck_aspect

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
    return rust_shard_action(ctx, target, "", stem = "_dx_docs/" + ctx.label.name)

def _go_docs_shard(target, ctx):
    if GoInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    info = target[GoInfo]
    if not info.importpath:
        return None
    return go_shard_action(ctx, info, "", "_dx_docs/" + ctx.label.name, ctx.executable._driver, ctx.executable._go_extract).shard

def _python_docs_shard(target, ctx):
    if _PyInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    if not python_package_name_ok(ctx.label.name):
        return None
    action = python_shard_action(ctx, target[_PyInfo], ctx.label.name, "google", "_dx_docs/" + ctx.label.name, ctx.executable._py_driver, ctx.executable._py_extract)
    return action.shard if action else None

def _jvm_docs_shard(target, ctx):
    if JavaInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary") or ctx.label.name.endswith("_upstream"):
        return None
    srcs = sorted(getattr(ctx.rule.files, "srcs", []), key = lambda f: f.path)
    if not srcs or getattr(ctx.rule.attr, "plugins", []):
        return None
    for language, spec in JVM_LANGUAGES.items():
        if all([f.extension == spec.extension for f in srcs]):
            if ctx.toolchains[JAVA_TOOLCHAIN] == None or (language == "kotlin" and ctx.toolchains[KOTLIN_TOOLCHAIN] == None):
                return None
            adapter = getattr(ctx.executable, "_" + language + "_extract")
            action = jvm_shard_action(ctx, language, target[JavaInfo], srcs, ctx.label.name, [], "_dx_docs/" + ctx.label.name, adapter, ctx.executable._jvm_convert)
            return action.shard
    return None

def _dotnet_docs_shard(target, ctx):
    if DocsShardInfo in target or DotnetDocsInfo not in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    info = target[DotnetDocsInfo]
    if dotnet_docs_problem(info, info.language):
        return None
    driver = ctx.executable._csharp_driver if info.language == "csharp" else ctx.executable._fsharp_driver
    return dotnet_shard_action(ctx, info, info.assembly, "_dx_docs/" + ctx.label.name, driver, ctx.executable._dotnet_extract).shard

def _powershell_docs_shard(target, ctx):
    if _PwshInfo not in target or DocsShardInfo in target:
        return None
    if ctx.rule.kind.endswith("_test") or ctx.rule.kind.endswith("_binary"):
        return None
    files = sorted(target[_PwshInfo].srcs.to_list(), key = lambda f: f.path)
    sources = [f for f in files if f.extension in ("ps1", "psm1")]
    manifests = [f for f in files if f.extension == "psd1"]
    if not sources or len(manifests) > 1:
        return None
    package = manifests[0].basename[:-len(".psd1")] if manifests else ctx.label.name
    pwsh = ctx.attr._pwsh[platform_common.ToolchainInfo]
    return powershell_shard_action(ctx, sources, manifests[0] if manifests else None, package, "", "_dx_docs/" + ctx.label.name, pwsh, ctx.file._pwsh_script, ctx.executable._pwsh_extract).shard

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
    rust = _rust_docs_shard(target, ctx)
    if rust:
        parts.append({
            "dx_results": [rust.shard],
            "dx_docs_shards": [rust.shard],
            "rustdoc_json": rust.outputs["rustdoc_json"].to_list(),
            "srcs_manifest": rust.outputs["srcs_manifest"].to_list(),
        })
    go_shard = _go_docs_shard(target, ctx)
    if go_shard:
        parts.append({"dx_results": [go_shard], "dx_docs_shards": [go_shard]})
    py_shard = _python_docs_shard(target, ctx)
    if py_shard:
        parts.append({"dx_results": [py_shard], "dx_docs_shards": [py_shard]})
    jvm_shard = _jvm_docs_shard(target, ctx)
    if jvm_shard:
        parts.append({"dx_results": [jvm_shard], "dx_docs_shards": [jvm_shard]})
    dotnet_shard = _dotnet_docs_shard(target, ctx)
    if dotnet_shard:
        parts.append({"dx_results": [dotnet_shard], "dx_docs_shards": [dotnet_shard]})
    pwsh_shard = _powershell_docs_shard(target, ctx)
    if pwsh_shard:
        parts.append({"dx_results": [pwsh_shard], "dx_docs_shards": [pwsh_shard]})
    if not parts:
        return []
    groups = {}
    for part in parts:
        for name, files in part.items():
            groups[name] = groups.get(name, []) + files
    return [OutputGroupInfo(**{name: depset(files) for name, files in groups.items()})]

real_docs_aspect = aspect(
    implementation = _real_docs_aspect_impl,
    doc = "Puts the docs sites, shards and API trees of a target in dx_results and a manifest naming each site tree in dx_docs_sites. Rust, Go, Python, JVM, C#, F# and PowerShell library shards go in dx_docs_shards. Rust shards also put their rustdoc JSON and source manifest in rustdoc_json and srcs_manifest.",
    attrs = {
        "_csharp_driver": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/dotnet/csharp:csharp_api_pinned"),
            executable = True,
        ),
        "_dotnet_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/dotnet/extract:dotnet_extract"),
            executable = True,
        ),
        "_driver": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/go/godoc:godoc_json"),
            executable = True,
        ),
        "_error_format": attr.label(default = Label("@rules_rust//rust/settings:error_format")),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/rust/extract:rust_extract"),
            executable = True,
        ),
        "_fsharp_driver": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/dotnet/fsharp:fsharp_api_pinned"),
            executable = True,
        ),
        "_go_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/go/extract:go_extract"),
            executable = True,
        ),
        "_process_wrapper": attr.label(
            default = Label("@rules_rust//util/process_wrapper"),
            cfg = "exec",
            executable = True,
        ),
        "_java_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/jvm/javadoc:javadoc_extract"),
            executable = True,
        ),
        "_jvm_convert": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/jvm/convert:jvm_convert"),
            executable = True,
        ),
        "_kotlin_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/jvm/dokka:dokka_extract"),
            executable = True,
        ),
        "_pwsh": attr.label(
            cfg = "exec",
            default = Label("@rules_powershell//powershell:current_pwsh_toolchain"),
        ),
        "_pwsh_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/powershell/extract:powershell_extract"),
            executable = True,
        ),
        "_pwsh_script": attr.label(
            allow_single_file = True,
            default = Label("//tools/docs/powershell:extract.ps1"),
        ),
        "_py_driver": attr.label(
            cfg = "exec",
            default = Label("//quality/tools/python:griffe_dump"),
            executable = True,
        ),
        "_py_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/python/extract:python_extract"),
            executable = True,
        ),
        "_scala_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/jvm/scaladoc:scaladoc_extract"),
            executable = True,
        ),
    },
    fragments = ["cpp"],
    requires = [rust_doc_inputs_aspect, dotnet_docs_collect_aspect],
    toolchains = [
        config_common.toolchain_type(JAVA_TOOLCHAIN, mandatory = False),
        config_common.toolchain_type(KOTLIN_TOOLCHAIN, mandatory = False),
        config_common.toolchain_type(_RUST_DOCS_TOOLCHAIN, mandatory = False),
        config_common.toolchain_type(_CC_DOCS_TOOLCHAIN, mandatory = False),
    ],
)

real_cue_format_aspect = _real_cue_format_aspect

real_cue_typecheck_aspect = _real_cue_typecheck_aspect

real_pkl_format_aspect = _real_pkl_format_aspect

real_pkl_lint_aspect = _real_pkl_lint_aspect

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
