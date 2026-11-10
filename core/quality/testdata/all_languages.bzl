"""Target-scoped real capability aspects over real adapters."""

# BEGIN GENERATED LANG ASPECT LOADS
load("//ansible/private/quality:ansible.bzl", _real_ansible_lint_aspect = "real_ansible_lint_aspect")
load("//bicep/private/quality:bicep.bzl", _real_bicep_format_aspect = "real_bicep_format_aspect", _real_bicep_lint_aspect = "real_bicep_lint_aspect")
load("//cloudformation/private/quality:cloudformation.bzl", _real_cloudformation_lint_aspect = "real_cloudformation_lint_aspect")
load("//cmake/private/quality:cmake.bzl", _real_cmake_format_aspect = "real_cmake_format_aspect", _real_cmake_lint_aspect = "real_cmake_lint_aspect")
load("//core/quality:families/beam.bzl", _real_beam_format_probe_aspect = "real_beam_format_probe_aspect", _real_beam_lint_probe_aspect = "real_beam_lint_probe_aspect", _real_beam_typecheck_probe_aspect = "real_beam_typecheck_probe_aspect", _real_elixir_format_aspect = "real_elixir_format_aspect", _real_elixir_lint_aspect = "real_elixir_lint_aspect", _real_erlang_format_aspect = "real_erlang_format_aspect", _real_erlang_lint_aspect = "real_erlang_lint_aspect", _real_erlang_typecheck_aspect = "real_erlang_typecheck_aspect")
load("//core/quality:families/cc.bzl", _real_cc_format_aspect = "real_cc_format_aspect", _real_cc_format_probe_aspect = "real_cc_format_probe_aspect", _real_cc_lint_aspect = "real_cc_lint_aspect", _real_cc_lint_probe_aspect = "real_cc_lint_probe_aspect", _real_cc_typecheck_aspect = "real_cc_typecheck_aspect", _real_cc_typecheck_probe_aspect = "real_cc_typecheck_probe_aspect")
load("//core/quality:families/dotnet.bzl", _real_dotnet_format_aspect = "real_dotnet_format_aspect", _real_dotnet_format_probe_aspect = "real_dotnet_format_probe_aspect", _real_dotnet_lint_aspect = "real_dotnet_lint_aspect", _real_dotnet_lint_probe_aspect = "real_dotnet_lint_probe_aspect", _real_dotnet_typecheck_aspect = "real_dotnet_typecheck_aspect", _real_dotnet_typecheck_probe_aspect = "real_dotnet_typecheck_probe_aspect")
load("//core/quality:families/generation.bzl", _real_avro_lint_aspect = "real_avro_lint_aspect", _real_avro_typecheck_aspect = "real_avro_typecheck_aspect", _real_capnp_lint_aspect = "real_capnp_lint_aspect", _real_capnp_typecheck_aspect = "real_capnp_typecheck_aspect", _real_flatbuffers_lint_aspect = "real_flatbuffers_lint_aspect", _real_flatbuffers_typecheck_aspect = "real_flatbuffers_typecheck_aspect", _real_generation_format_probe_aspect = "real_generation_format_probe_aspect", _real_generation_lint_probe_aspect = "real_generation_lint_probe_aspect", _real_generation_typecheck_probe_aspect = "real_generation_typecheck_probe_aspect", _real_protobuf_format_aspect = "real_protobuf_format_aspect", _real_protobuf_lint_aspect = "real_protobuf_lint_aspect", _real_protobuf_typecheck_aspect = "real_protobuf_typecheck_aspect", _real_thrift_lint_aspect = "real_thrift_lint_aspect", _real_thrift_typecheck_aspect = "real_thrift_typecheck_aspect")
load("//core/quality:families/jvm.bzl", _real_jvm_format_aspect = "real_jvm_format_aspect", _real_jvm_format_probe_aspect = "real_jvm_format_probe_aspect", _real_jvm_lint_aspect = "real_jvm_lint_aspect", _real_jvm_lint_probe_aspect = "real_jvm_lint_probe_aspect", _real_jvm_typecheck_aspect = "real_jvm_typecheck_aspect", _real_jvm_typecheck_probe_aspect = "real_jvm_typecheck_probe_aspect", _real_scala_format_aspect = "real_scala_format_aspect", _real_scala_lint_aspect = "real_scala_lint_aspect")
load("//core/quality:families/lua.bzl", _real_lua_format_aspect = "real_lua_format_aspect", _real_lua_format_probe_aspect = "real_lua_format_probe_aspect", _real_lua_lint_aspect = "real_lua_lint_aspect", _real_lua_lint_probe_aspect = "real_lua_lint_probe_aspect", _real_lua_typecheck_aspect = "real_lua_typecheck_aspect", _real_lua_typecheck_probe_aspect = "real_lua_typecheck_probe_aspect")
load("//core/quality:families/ocaml.bzl", _real_ocaml_format_aspect = "real_ocaml_format_aspect", _real_ocaml_format_probe_aspect = "real_ocaml_format_probe_aspect", _real_ocaml_lint_aspect = "real_ocaml_lint_aspect", _real_ocaml_lint_probe_aspect = "real_ocaml_lint_probe_aspect", _real_ocaml_typecheck_aspect = "real_ocaml_typecheck_aspect", _real_ocaml_typecheck_probe_aspect = "real_ocaml_typecheck_probe_aspect")
load("//core/quality:families/perl.bzl", _real_perl_format_aspect = "real_perl_format_aspect", _real_perl_format_probe_aspect = "real_perl_format_probe_aspect", _real_perl_lint_aspect = "real_perl_lint_aspect", _real_perl_lint_probe_aspect = "real_perl_lint_probe_aspect")
load("//core/quality:families/php.bzl", _real_php_lint_aspect = "real_php_lint_aspect", _real_php_lint_probe_aspect = "real_php_lint_probe_aspect", _real_php_typecheck_aspect = "real_php_typecheck_aspect", _real_php_typecheck_probe_aspect = "real_php_typecheck_probe_aspect")
load("//core/quality:families/pkl.bzl", _real_pkl_format_aspect = "real_pkl_format_aspect", _real_pkl_format_probe_aspect = "real_pkl_format_probe_aspect", _real_pkl_lint_aspect = "real_pkl_lint_aspect", _real_pkl_lint_probe_aspect = "real_pkl_lint_probe_aspect", _real_pkl_typecheck_aspect = "real_pkl_typecheck_aspect", _real_pkl_typecheck_probe_aspect = "real_pkl_typecheck_probe_aspect")
load("//core/quality:families/powershell.bzl", _real_powershell_format_aspect = "real_powershell_format_aspect", _real_powershell_format_probe_aspect = "real_powershell_format_probe_aspect", _real_powershell_lint_aspect = "real_powershell_lint_aspect", _real_powershell_lint_probe_aspect = "real_powershell_lint_probe_aspect")
load("//core/quality:families/python.bzl", _real_python_format_aspect = "real_python_format_aspect", _real_python_format_probe_aspect = "real_python_format_probe_aspect", _real_python_lint_aspect = "real_python_lint_aspect", _real_python_lint_probe_aspect = "real_python_lint_probe_aspect", _real_python_typecheck_aspect = "real_python_typecheck_aspect", _real_python_typecheck_probe_aspect = "real_python_typecheck_probe_aspect")
load("//core/quality:families/ruby.bzl", _real_ruby_format_aspect = "real_ruby_format_aspect", _real_ruby_format_probe_aspect = "real_ruby_format_probe_aspect", _real_ruby_lint_aspect = "real_ruby_lint_aspect", _real_ruby_lint_probe_aspect = "real_ruby_lint_probe_aspect", _real_ruby_typecheck_aspect = "real_ruby_typecheck_aspect", _real_ruby_typecheck_probe_aspect = "real_ruby_typecheck_probe_aspect")
load("//core/quality:families/sql.bzl", _real_sql_format_aspect = "real_sql_format_aspect", _real_sql_lint_aspect = "real_sql_lint_aspect")
load("//core/quality:families/swift.bzl", _real_swift_format_aspect = "real_swift_format_aspect", _real_swift_format_probe_aspect = "real_swift_format_probe_aspect", _real_swift_lint_aspect = "real_swift_lint_aspect", _real_swift_lint_probe_aspect = "real_swift_lint_probe_aspect", _real_swift_typecheck_aspect = "real_swift_typecheck_aspect", _real_swift_typecheck_probe_aspect = "real_swift_typecheck_probe_aspect")
load("//core/quality:families/zig.bzl", _real_zig_format_aspect = "real_zig_format_aspect", _real_zig_format_probe_aspect = "real_zig_format_probe_aspect", _real_zig_lint_aspect = "real_zig_lint_aspect", _real_zig_lint_probe_aspect = "real_zig_lint_probe_aspect", _real_zig_typecheck_aspect = "real_zig_typecheck_aspect", _real_zig_typecheck_probe_aspect = "real_zig_typecheck_probe_aspect")
load("//d/private/quality:d.bzl", _real_d_format_aspect = "real_d_format_aspect", _real_d_format_probe_aspect = "real_d_format_probe_aspect", _real_d_lint_aspect = "real_d_lint_aspect", _real_d_lint_probe_aspect = "real_d_lint_probe_aspect", _real_d_typecheck_aspect = "real_d_typecheck_aspect", _real_d_typecheck_probe_aspect = "real_d_typecheck_probe_aspect")
load("//dart/private/quality:dart.bzl", _real_dart_format_aspect = "real_dart_format_aspect", _real_dart_format_probe_aspect = "real_dart_format_probe_aspect", _real_dart_lint_aspect = "real_dart_lint_aspect", _real_dart_lint_probe_aspect = "real_dart_lint_probe_aspect", _real_dart_typecheck_aspect = "real_dart_typecheck_aspect", _real_dart_typecheck_probe_aspect = "real_dart_typecheck_probe_aspect")
load("//dockerfile/private/quality:dockerfile.bzl", _real_dockerfile_lint_aspect = "real_dockerfile_lint_aspect")
load("//fortran/private/quality:fortran.bzl", _real_fortran_format_aspect = "real_fortran_format_aspect", _real_fortran_format_probe_aspect = "real_fortran_format_probe_aspect", _real_fortran_lint_aspect = "real_fortran_lint_aspect", _real_fortran_lint_probe_aspect = "real_fortran_lint_probe_aspect", _real_fortran_typecheck_aspect = "real_fortran_typecheck_aspect", _real_fortran_typecheck_probe_aspect = "real_fortran_typecheck_probe_aspect")
load("//go/private/quality:go.bzl", _real_go_format_aspect = "real_go_format_aspect", _real_go_format_probe_aspect = "real_go_format_probe_aspect", _real_go_lint_aspect = "real_go_lint_aspect", _real_go_lint_probe_aspect = "real_go_lint_probe_aspect", _real_go_typecheck_aspect = "real_go_typecheck_aspect", _real_go_typecheck_probe_aspect = "real_go_typecheck_probe_aspect")
load("//haskell/private/quality:haskell.bzl", _real_haskell_format_aspect = "real_haskell_format_aspect", _real_haskell_format_probe_aspect = "real_haskell_format_probe_aspect", _real_haskell_lint_aspect = "real_haskell_lint_aspect", _real_haskell_lint_probe_aspect = "real_haskell_lint_probe_aspect", _real_haskell_typecheck_aspect = "real_haskell_typecheck_aspect", _real_haskell_typecheck_probe_aspect = "real_haskell_typecheck_probe_aspect")
load("//javascript/private/quality:js.bzl", _real_js_format_aspect = "real_js_format_aspect", _real_js_format_probe_aspect = "real_js_format_probe_aspect", _real_js_lint_aspect = "real_js_lint_aspect", _real_js_lint_probe_aspect = "real_js_lint_probe_aspect", _real_js_typecheck_aspect = "real_js_typecheck_aspect", _real_js_typecheck_probe_aspect = "real_js_typecheck_probe_aspect")
load("//julia/private/quality:julia.bzl", _real_julia_format_aspect = "real_julia_format_aspect", _real_julia_format_probe_aspect = "real_julia_format_probe_aspect", _real_julia_lint_aspect = "real_julia_lint_aspect", _real_julia_lint_probe_aspect = "real_julia_lint_probe_aspect", _real_julia_typecheck_aspect = "real_julia_typecheck_aspect", _real_julia_typecheck_probe_aspect = "real_julia_typecheck_probe_aspect")
load("//kubernetes/private/quality:kubernetes.bzl", _real_kubernetes_format_aspect = "real_kubernetes_format_aspect", _real_kubernetes_lint_aspect = "real_kubernetes_lint_aspect")
load("//make/private/quality:make.bzl", _real_make_format_aspect = "real_make_format_aspect", _real_make_lint_aspect = "real_make_lint_aspect")
load("//rego/private/quality:rego.bzl", _real_rego_format_aspect = "real_rego_format_aspect", _real_rego_lint_aspect = "real_rego_lint_aspect", _real_rego_typecheck_aspect = "real_rego_typecheck_aspect")
load("//rust/private/quality:rust.bzl", _real_rust_format_aspect = "real_rust_format_aspect", _real_rust_format_probe_aspect = "real_rust_format_probe_aspect", _real_rust_lint_aspect = "real_rust_lint_aspect", _real_rust_lint_probe_aspect = "real_rust_lint_probe_aspect", _real_rust_typecheck_aspect = "real_rust_typecheck_aspect", _real_rust_typecheck_probe_aspect = "real_rust_typecheck_probe_aspect")
load("//terraform/private/quality:terraform.bzl", _real_terraform_format_aspect = "real_terraform_format_aspect", _real_terraform_lint_aspect = "real_terraform_lint_aspect", _real_terraform_typecheck_aspect = "real_terraform_typecheck_aspect")
load("//tools/docs/avro:aspect.bzl", _real_avro_docs_aspect = "real_avro_docs_aspect")
load("//tools/docs/beam:aspect.bzl", _real_beam_docs_aspect = "real_beam_docs_aspect")
load("//tools/docs/cc:aspect.bzl", _real_cc_docs_aspect = "real_cc_docs_aspect")
load("//tools/docs/components:aspect.bzl", _real_components_docs_aspect = "real_components_docs_aspect")
load("//tools/docs/d:aspect.bzl", _real_d_docs_aspect = "real_d_docs_aspect")
load("//tools/docs/dart:aspect.bzl", _real_dart_docs_aspect = "real_dart_docs_aspect")
load("//tools/docs/dotnet:aspect.bzl", _real_dotnet_docs_aspect = "real_dotnet_docs_aspect")
load("//tools/docs/fortran:aspect.bzl", _real_fortran_docs_aspect = "real_fortran_docs_aspect")
load("//tools/docs/go:aspect.bzl", _real_go_docs_aspect = "real_go_docs_aspect")
load("//tools/docs/haskell:aspect.bzl", _real_haskell_docs_aspect = "real_haskell_docs_aspect")
load("//tools/docs/javascript:aspect.bzl", _real_javascript_docs_aspect = "real_javascript_docs_aspect")
load("//tools/docs/julia:aspect.bzl", _real_julia_docs_aspect = "real_julia_docs_aspect")
load("//tools/docs/jvm:aspect.bzl", _real_jvm_docs_aspect = "real_jvm_docs_aspect")
load("//tools/docs/lua:aspect.bzl", _real_lua_docs_aspect = "real_lua_docs_aspect")
load("//tools/docs/ocaml:aspect.bzl", _real_ocaml_docs_aspect = "real_ocaml_docs_aspect")
load("//tools/docs/perl:aspect.bzl", _real_perl_docs_aspect = "real_perl_docs_aspect")
load("//tools/docs/php:aspect.bzl", _real_php_docs_aspect = "real_php_docs_aspect")
load("//tools/docs/powershell:aspect.bzl", _real_powershell_docs_aspect = "real_powershell_docs_aspect")
load("//tools/docs/protobuf:aspect.bzl", _real_protobuf_docs_aspect = "real_protobuf_docs_aspect")
load("//tools/docs/python:aspect.bzl", _real_python_docs_aspect = "real_python_docs_aspect")
load("//tools/docs/ruby:aspect.bzl", _real_ruby_docs_aspect = "real_ruby_docs_aspect")
load("//tools/docs/rust:aspect.bzl", _real_rust_docs_aspect = "real_rust_docs_aspect")
load("//tools/docs/swift:aspect.bzl", _real_swift_docs_aspect = "real_swift_docs_aspect")
load("//tools/docs/typescript:aspect.bzl", _real_typescript_docs_aspect = "real_typescript_docs_aspect")
load("//tools/docs/zig:aspect.bzl", _real_zig_docs_aspect = "real_zig_docs_aspect")
# END GENERATED LANG ASPECT LOADS

load("//core/quality:compile_check.bzl", "COMPILE_CHECK_TOOLS", "compile_check_hook")
load("//core/quality:families/cc.bzl", _CC_HOOKS = "HOOKS", _CC_RECOGNIZER = "RECOGNIZER", _CC_SHARD_SPECS = "SHARD_SPECS", _CC_TOOLS = "TOOLS")
load("//core/quality:families/core.bzl", _real_format_aspect = "real_format_aspect", _real_lint_aspect = "real_lint_aspect", _real_typecheck_aspect = "real_typecheck_aspect", _CORE_HOOKS = "HOOKS", _CORE_SHARD_SPECS = "SHARD_SPECS", _CORE_TOOLS = "TOOLS")
load("//ansible/private/quality:ansible.bzl", _ANSIBLE_SHARD_SPECS = "SHARD_SPECS", _ANSIBLE_TOOLS = "TOOLS")
load("//cloudformation/private/quality:cloudformation.bzl", _CLOUDFORMATION_SHARD_SPECS = "SHARD_SPECS", _CLOUDFORMATION_TOOLS = "TOOLS")
load("//cmake/private/quality:cmake.bzl", _CMAKE_SHARD_SPECS = "SHARD_SPECS", _CMAKE_TOOLS = "TOOLS")
load("//dockerfile/private/quality:dockerfile.bzl", _DOCKERFILE_SHARD_SPECS = "SHARD_SPECS", _DOCKERFILE_TOOLS = "TOOLS")
load("//core/quality:families/oci_image.bzl", _real_oci_image_lint_aspect = "real_oci_image_lint_aspect", _real_oci_image_typecheck_aspect = "real_oci_image_typecheck_aspect", _OCI_IMAGE_HOOKS = "HOOKS", _OCI_IMAGE_SHARD_SPECS = "SHARD_SPECS", _OCI_IMAGE_TOOLS = "TOOLS")
load("//core/quality:families/html_template.bzl", _real_html_template_format_aspect = "real_html_template_format_aspect", _real_html_template_lint_aspect = "real_html_template_lint_aspect", _HTML_TEMPLATE_SHARD_SPECS = "SHARD_SPECS", _HTML_TEMPLATE_TOOLS = "TOOLS")
load("//core/quality:families/latex.bzl", _real_latex_format_aspect = "real_latex_format_aspect", _real_latex_lint_aspect = "real_latex_lint_aspect", _LATEX_SHARD_SPECS = "SHARD_SPECS", _LATEX_TOOLS = "TOOLS")
load("//make/private/quality:make.bzl", _MAKE_SHARD_SPECS = "SHARD_SPECS", _MAKE_TOOLS = "TOOLS")
load("//core/quality:families/nix.bzl", _real_nix_format_aspect = "real_nix_format_aspect", _real_nix_lint_aspect = "real_nix_lint_aspect", _NIX_SHARD_SPECS = "SHARD_SPECS", _NIX_TOOLS = "TOOLS")
load("//core/quality:families/perl.bzl", _PERL_SHARD_SPECS = "SHARD_SPECS", _PERL_TOOLS = "TOOLS")
load("//core/quality:families/shell.bzl", _real_shell_lint_aspect = "real_shell_lint_aspect", _real_shell_format_aspect = "real_shell_format_aspect", _SHELL_SHARD_SPECS = "SHARD_SPECS", _SHELL_TOOLS = "TOOLS")
load("//core/quality:families/swift.bzl", _SWIFT_SHARD_SPECS = "SHARD_SPECS", _SWIFT_TOOLS = "TOOLS")
load("//core/quality:families/text.bzl", _real_text_lint_aspect = "real_text_lint_aspect", _TEXT_SHARD_SPECS = "SHARD_SPECS", _TEXT_TOOLS = "TOOLS")
load("//core/quality:families/typst.bzl", _real_typst_format_aspect = "real_typst_format_aspect", _real_typst_lint_aspect = "real_typst_lint_aspect", _TYPST_SHARD_SPECS = "SHARD_SPECS", _TYPST_TOOLS = "TOOLS")
load("//core/quality:families/xml.bzl", _real_xml_lint_aspect = "real_xml_lint_aspect", _XML_SHARD_SPECS = "SHARD_SPECS", _XML_TOOLS = "TOOLS")
load("//core/quality:families/yaml.bzl", _real_yaml_format_aspect = "real_yaml_format_aspect", _real_yaml_lint_aspect = "real_yaml_lint_aspect", _YAML_SHARD_SPECS = "SHARD_SPECS", _YAML_TOOLS = "TOOLS")
load("//core/quality:families/cue.bzl", _real_cue_format_aspect = "real_cue_format_aspect", _real_cue_typecheck_aspect = "real_cue_typecheck_aspect", _CUE_HOOKS = "HOOKS", _CUE_SHARD_SPECS = "SHARD_SPECS", _CUE_TOOLS = "TOOLS")
load("//core/quality:families/pkl.bzl", _PKL_HOOKS = "HOOKS", _PKL_SHARD_SPECS = "SHARD_SPECS", _PKL_TOOLS = "TOOLS")
load("//core/quality:families/jsonnet.bzl", _real_jsonnet_format_aspect = "real_jsonnet_format_aspect", _real_jsonnet_lint_aspect = "real_jsonnet_lint_aspect", _JSONNET_HOOKS = "HOOKS", _JSONNET_SHARD_SPECS = "SHARD_SPECS", _JSONNET_TOOLS = "TOOLS")
load("//rego/private/quality:rego.bzl", _REGO_HOOKS = "HOOKS", _REGO_SHARD_SPECS = "SHARD_SPECS", _REGO_TOOLS = "TOOLS")
load("//bicep/private/quality:bicep.bzl", _BICEP_HOOKS = "HOOKS", _BICEP_SHARD_SPECS = "SHARD_SPECS", _BICEP_TOOLS = "TOOLS")
load("//terraform/private/quality:terraform.bzl", _TERRAFORM_HOOKS = "HOOKS", _TERRAFORM_SHARD_SPECS = "SHARD_SPECS", _TERRAFORM_TOOLS = "TOOLS")
load("//core/quality:families/lua.bzl", _LUA_HOOKS = "HOOKS", _LUA_SHARD_SPECS = "SHARD_SPECS", _LUA_TOOLS = "TOOLS")
load("//core/quality:families/php.bzl", _PHP_HOOKS = "HOOKS", _PHP_SHARD_SPECS = "SHARD_SPECS", _PHP_TOOLS = "TOOLS")
load("//kubernetes/private/quality:kubernetes.bzl", _KUBERNETES_HOOKS = "HOOKS", _KUBERNETES_SHARD_SPECS = "SHARD_SPECS", _KUBERNETES_TOOLS = "TOOLS")
load("//core/quality:families/sql.bzl", _SQL_HOOKS = "HOOKS", _SQL_SHARD_SPECS = "SHARD_SPECS", _SQL_TOOLS = "TOOLS")
load("//dart/private/quality:dart.bzl", _DART_HOOKS = "HOOKS", _DART_SHARD_SPECS = "SHARD_SPECS", _DART_TOOLS = "TOOLS")
load("//core/quality:families/powershell.bzl", _POWERSHELL_HOOKS = "HOOKS", _POWERSHELL_SHARD_SPECS = "SHARD_SPECS", _POWERSHELL_TOOLS = "TOOLS")
load("//core/quality:families/ruby.bzl", _RUBY_HOOKS = "HOOKS", _RUBY_SHARD_SPECS = "SHARD_SPECS", _RUBY_TOOLS = "TOOLS")
load("//core/quality:families/python.bzl", _PYTHON_HOOKS = "HOOKS", _PYTHON_SHARD_SPECS = "SHARD_SPECS", _PYTHON_TOOLS = "TOOLS")
load("//core/quality:families/jvm.bzl", _JVM_HOOKS = "HOOKS", _JVM_SHARD_SPECS = "SHARD_SPECS", _JVM_TOOLS = "TOOLS")
load("//javascript/private/quality:js.bzl", _JS_HOOKS = "HOOKS", _JS_SHARD_SPECS = "SHARD_SPECS", _JS_TOOLS = "TOOLS")
load("//julia/private/quality:julia.bzl", _JULIA_HOOKS = "HOOKS", _JULIA_SHARD_SPECS = "SHARD_SPECS", _JULIA_TOOLS = "TOOLS")
load("//core/quality:families/dotnet.bzl", _DOTNET_HOOKS = "HOOKS", _DOTNET_SHARD_SPECS = "SHARD_SPECS", _DOTNET_TOOLS = "TOOLS")
load("//d/private/quality:d.bzl", _D_SHARD_SPECS = "SHARD_SPECS", _D_TOOLS = "TOOLS")
load("//fortran/private/quality:fortran.bzl", _FORTRAN_SHARD_SPECS = "SHARD_SPECS", _FORTRAN_TOOLS = "TOOLS")
load("//haskell/private/quality:haskell.bzl", _HASKELL_SHARD_SPECS = "SHARD_SPECS", _HASKELL_TOOLS = "TOOLS")
load("//core/quality:families/generation.bzl", _GENERATION_HOOKS = "HOOKS", _GENERATION_RECOGNIZERS = "RECOGNIZERS", _GENERATION_SHARD_SPECS = "SHARD_SPECS", _GENERATION_TOOLS = "TOOLS")
load("//core/quality:families/zig.bzl", _ZIG_SHARD_SPECS = "SHARD_SPECS", _ZIG_TOOLS = "TOOLS")
load("//core/quality:families/beam.bzl", _BEAM_HOOKS = "HOOKS", _BEAM_SHARD_SPECS = "SHARD_SPECS", _BEAM_TOOLS = "TOOLS")
load("//go/private/quality:go.bzl", _GO_HOOKS = "HOOKS", _GO_SHARD_SPECS = "SHARD_SPECS", _GO_TOOLS = "TOOLS")
load("//rust/private/quality:rust.bzl", _RUST_HARNESS_SOURCES = "harness_sources", _RUST_HOOKS = "HOOKS", _RUST_RECOGNIZER = "RECOGNIZER", _RUST_SHARD_SPECS = "SHARD_SPECS", _RUST_TOOLCHAIN = "TOOLCHAIN", _RUST_TOOLS = "TOOLS", _inherits_test_crate_sources = "inherits_test_crate_sources", _merge_inherited_sources = "merge_inherited_sources", _rustc_diagnostics_group = "rustc_diagnostics_group")
load("//core/quality:families/ocaml.bzl", _OCAML_HOOKS = "HOOKS", _OCAML_SHARD_SPECS = "SHARD_SPECS", _OCAML_TOOLS = "TOOLS")
load("//core/quality:families/core_probe.bzl", _real_core_format_probe_aspect = "real_core_format_probe_aspect", _real_core_lint_probe_aspect = "real_core_lint_probe_aspect", _real_core_typecheck_probe_aspect = "real_core_typecheck_probe_aspect")
load("//core/quality:pipeline.bzl", "drop_pipeline_tool", "real_request_tool_env")
load("//core/quality:real_core.bzl", "real_config", "real_context", "real_make_aspect", "real_make_probe_aspect", "real_upstream_direct_sources", _RealShardProbeInfo = "RealShardProbeInfo", _real_policy_error = "real_policy_error", _real_quality_resource_set = "real_quality_resource_set", _real_shard_result_stems = "real_shard_result_stems", _real_skip_notes = "real_skip_notes", _real_tool_table_error = "real_tool_table_error", _real_unavailable_tools = "real_unavailable_tools", _real_tool_table_shape_error = "real_tool_table_shape_error", _real_wired_capabilities = "real_wired_capabilities", _real_wired_tools = "real_wired_tools")
load("//core/quality:sources.bzl", "QualityToolsInfo")
load("//tools/docs:aspect.bzl", _real_docs_aspect = "real_docs_aspect")

# BEGIN GENERATED LANG ASPECT EXPORTS
real_ansible_lint_aspect = _real_ansible_lint_aspect

real_bicep_format_aspect = _real_bicep_format_aspect

real_bicep_lint_aspect = _real_bicep_lint_aspect

real_cloudformation_lint_aspect = _real_cloudformation_lint_aspect

real_cmake_format_aspect = _real_cmake_format_aspect

real_cmake_lint_aspect = _real_cmake_lint_aspect

real_beam_format_probe_aspect = _real_beam_format_probe_aspect

real_beam_lint_probe_aspect = _real_beam_lint_probe_aspect

real_beam_typecheck_probe_aspect = _real_beam_typecheck_probe_aspect

real_elixir_format_aspect = _real_elixir_format_aspect

real_elixir_lint_aspect = _real_elixir_lint_aspect

real_erlang_format_aspect = _real_erlang_format_aspect

real_erlang_lint_aspect = _real_erlang_lint_aspect

real_erlang_typecheck_aspect = _real_erlang_typecheck_aspect

real_cc_format_aspect = _real_cc_format_aspect

real_cc_format_probe_aspect = _real_cc_format_probe_aspect

real_cc_lint_aspect = _real_cc_lint_aspect

real_cc_lint_probe_aspect = _real_cc_lint_probe_aspect

real_cc_typecheck_aspect = _real_cc_typecheck_aspect

real_cc_typecheck_probe_aspect = _real_cc_typecheck_probe_aspect

real_dotnet_format_aspect = _real_dotnet_format_aspect

real_dotnet_format_probe_aspect = _real_dotnet_format_probe_aspect

real_dotnet_lint_aspect = _real_dotnet_lint_aspect

real_dotnet_lint_probe_aspect = _real_dotnet_lint_probe_aspect

real_dotnet_typecheck_aspect = _real_dotnet_typecheck_aspect

real_dotnet_typecheck_probe_aspect = _real_dotnet_typecheck_probe_aspect

real_avro_lint_aspect = _real_avro_lint_aspect

real_avro_typecheck_aspect = _real_avro_typecheck_aspect

real_capnp_lint_aspect = _real_capnp_lint_aspect

real_capnp_typecheck_aspect = _real_capnp_typecheck_aspect

real_flatbuffers_lint_aspect = _real_flatbuffers_lint_aspect

real_flatbuffers_typecheck_aspect = _real_flatbuffers_typecheck_aspect

real_generation_format_probe_aspect = _real_generation_format_probe_aspect

real_generation_lint_probe_aspect = _real_generation_lint_probe_aspect

real_generation_typecheck_probe_aspect = _real_generation_typecheck_probe_aspect

real_protobuf_format_aspect = _real_protobuf_format_aspect

real_protobuf_lint_aspect = _real_protobuf_lint_aspect

real_protobuf_typecheck_aspect = _real_protobuf_typecheck_aspect

real_thrift_lint_aspect = _real_thrift_lint_aspect

real_thrift_typecheck_aspect = _real_thrift_typecheck_aspect

real_jvm_format_aspect = _real_jvm_format_aspect

real_jvm_format_probe_aspect = _real_jvm_format_probe_aspect

real_jvm_lint_aspect = _real_jvm_lint_aspect

real_jvm_lint_probe_aspect = _real_jvm_lint_probe_aspect

real_jvm_typecheck_aspect = _real_jvm_typecheck_aspect

real_jvm_typecheck_probe_aspect = _real_jvm_typecheck_probe_aspect

real_scala_format_aspect = _real_scala_format_aspect

real_scala_lint_aspect = _real_scala_lint_aspect

real_lua_format_aspect = _real_lua_format_aspect

real_lua_format_probe_aspect = _real_lua_format_probe_aspect

real_lua_lint_aspect = _real_lua_lint_aspect

real_lua_lint_probe_aspect = _real_lua_lint_probe_aspect

real_lua_typecheck_aspect = _real_lua_typecheck_aspect

real_lua_typecheck_probe_aspect = _real_lua_typecheck_probe_aspect

real_ocaml_format_aspect = _real_ocaml_format_aspect

real_ocaml_format_probe_aspect = _real_ocaml_format_probe_aspect

real_ocaml_lint_aspect = _real_ocaml_lint_aspect

real_ocaml_lint_probe_aspect = _real_ocaml_lint_probe_aspect

real_ocaml_typecheck_aspect = _real_ocaml_typecheck_aspect

real_ocaml_typecheck_probe_aspect = _real_ocaml_typecheck_probe_aspect

real_perl_format_aspect = _real_perl_format_aspect

real_perl_format_probe_aspect = _real_perl_format_probe_aspect

real_perl_lint_aspect = _real_perl_lint_aspect

real_perl_lint_probe_aspect = _real_perl_lint_probe_aspect

real_php_lint_aspect = _real_php_lint_aspect

real_php_lint_probe_aspect = _real_php_lint_probe_aspect

real_php_typecheck_aspect = _real_php_typecheck_aspect

real_php_typecheck_probe_aspect = _real_php_typecheck_probe_aspect

real_pkl_format_aspect = _real_pkl_format_aspect

real_pkl_format_probe_aspect = _real_pkl_format_probe_aspect

real_pkl_lint_aspect = _real_pkl_lint_aspect

real_pkl_lint_probe_aspect = _real_pkl_lint_probe_aspect

real_pkl_typecheck_aspect = _real_pkl_typecheck_aspect

real_pkl_typecheck_probe_aspect = _real_pkl_typecheck_probe_aspect

real_powershell_format_aspect = _real_powershell_format_aspect

real_powershell_format_probe_aspect = _real_powershell_format_probe_aspect

real_powershell_lint_aspect = _real_powershell_lint_aspect

real_powershell_lint_probe_aspect = _real_powershell_lint_probe_aspect

real_python_format_aspect = _real_python_format_aspect

real_python_format_probe_aspect = _real_python_format_probe_aspect

real_python_lint_aspect = _real_python_lint_aspect

real_python_lint_probe_aspect = _real_python_lint_probe_aspect

real_python_typecheck_aspect = _real_python_typecheck_aspect

real_python_typecheck_probe_aspect = _real_python_typecheck_probe_aspect

real_ruby_format_aspect = _real_ruby_format_aspect

real_ruby_format_probe_aspect = _real_ruby_format_probe_aspect

real_ruby_lint_aspect = _real_ruby_lint_aspect

real_ruby_lint_probe_aspect = _real_ruby_lint_probe_aspect

real_ruby_typecheck_aspect = _real_ruby_typecheck_aspect

real_ruby_typecheck_probe_aspect = _real_ruby_typecheck_probe_aspect

real_sql_format_aspect = _real_sql_format_aspect

real_sql_lint_aspect = _real_sql_lint_aspect

real_swift_format_aspect = _real_swift_format_aspect

real_swift_format_probe_aspect = _real_swift_format_probe_aspect

real_swift_lint_aspect = _real_swift_lint_aspect

real_swift_lint_probe_aspect = _real_swift_lint_probe_aspect

real_swift_typecheck_aspect = _real_swift_typecheck_aspect

real_swift_typecheck_probe_aspect = _real_swift_typecheck_probe_aspect

real_zig_format_aspect = _real_zig_format_aspect

real_zig_format_probe_aspect = _real_zig_format_probe_aspect

real_zig_lint_aspect = _real_zig_lint_aspect

real_zig_lint_probe_aspect = _real_zig_lint_probe_aspect

real_zig_typecheck_aspect = _real_zig_typecheck_aspect

real_zig_typecheck_probe_aspect = _real_zig_typecheck_probe_aspect

real_d_format_aspect = _real_d_format_aspect

real_d_format_probe_aspect = _real_d_format_probe_aspect

real_d_lint_aspect = _real_d_lint_aspect

real_d_lint_probe_aspect = _real_d_lint_probe_aspect

real_d_typecheck_aspect = _real_d_typecheck_aspect

real_d_typecheck_probe_aspect = _real_d_typecheck_probe_aspect

real_dart_format_aspect = _real_dart_format_aspect

real_dart_format_probe_aspect = _real_dart_format_probe_aspect

real_dart_lint_aspect = _real_dart_lint_aspect

real_dart_lint_probe_aspect = _real_dart_lint_probe_aspect

real_dart_typecheck_aspect = _real_dart_typecheck_aspect

real_dart_typecheck_probe_aspect = _real_dart_typecheck_probe_aspect

real_dockerfile_lint_aspect = _real_dockerfile_lint_aspect

real_fortran_format_aspect = _real_fortran_format_aspect

real_fortran_format_probe_aspect = _real_fortran_format_probe_aspect

real_fortran_lint_aspect = _real_fortran_lint_aspect

real_fortran_lint_probe_aspect = _real_fortran_lint_probe_aspect

real_fortran_typecheck_aspect = _real_fortran_typecheck_aspect

real_fortran_typecheck_probe_aspect = _real_fortran_typecheck_probe_aspect

real_go_format_aspect = _real_go_format_aspect

real_go_format_probe_aspect = _real_go_format_probe_aspect

real_go_lint_aspect = _real_go_lint_aspect

real_go_lint_probe_aspect = _real_go_lint_probe_aspect

real_go_typecheck_aspect = _real_go_typecheck_aspect

real_go_typecheck_probe_aspect = _real_go_typecheck_probe_aspect

real_haskell_format_aspect = _real_haskell_format_aspect

real_haskell_format_probe_aspect = _real_haskell_format_probe_aspect

real_haskell_lint_aspect = _real_haskell_lint_aspect

real_haskell_lint_probe_aspect = _real_haskell_lint_probe_aspect

real_haskell_typecheck_aspect = _real_haskell_typecheck_aspect

real_haskell_typecheck_probe_aspect = _real_haskell_typecheck_probe_aspect

real_js_format_aspect = _real_js_format_aspect

real_js_format_probe_aspect = _real_js_format_probe_aspect

real_js_lint_aspect = _real_js_lint_aspect

real_js_lint_probe_aspect = _real_js_lint_probe_aspect

real_js_typecheck_aspect = _real_js_typecheck_aspect

real_js_typecheck_probe_aspect = _real_js_typecheck_probe_aspect

real_julia_format_aspect = _real_julia_format_aspect

real_julia_format_probe_aspect = _real_julia_format_probe_aspect

real_julia_lint_aspect = _real_julia_lint_aspect

real_julia_lint_probe_aspect = _real_julia_lint_probe_aspect

real_julia_typecheck_aspect = _real_julia_typecheck_aspect

real_julia_typecheck_probe_aspect = _real_julia_typecheck_probe_aspect

real_kubernetes_format_aspect = _real_kubernetes_format_aspect

real_kubernetes_lint_aspect = _real_kubernetes_lint_aspect

real_make_format_aspect = _real_make_format_aspect

real_make_lint_aspect = _real_make_lint_aspect

real_rego_format_aspect = _real_rego_format_aspect

real_rego_lint_aspect = _real_rego_lint_aspect

real_rego_typecheck_aspect = _real_rego_typecheck_aspect

real_rust_format_aspect = _real_rust_format_aspect

real_rust_format_probe_aspect = _real_rust_format_probe_aspect

real_rust_lint_aspect = _real_rust_lint_aspect

real_rust_lint_probe_aspect = _real_rust_lint_probe_aspect

real_rust_typecheck_aspect = _real_rust_typecheck_aspect

real_rust_typecheck_probe_aspect = _real_rust_typecheck_probe_aspect

real_terraform_format_aspect = _real_terraform_format_aspect

real_terraform_lint_aspect = _real_terraform_lint_aspect

real_terraform_typecheck_aspect = _real_terraform_typecheck_aspect

real_avro_docs_aspect = _real_avro_docs_aspect

real_beam_docs_aspect = _real_beam_docs_aspect

real_cc_docs_aspect = _real_cc_docs_aspect

real_components_docs_aspect = _real_components_docs_aspect

real_d_docs_aspect = _real_d_docs_aspect

real_dart_docs_aspect = _real_dart_docs_aspect

real_dotnet_docs_aspect = _real_dotnet_docs_aspect

real_fortran_docs_aspect = _real_fortran_docs_aspect

real_go_docs_aspect = _real_go_docs_aspect

real_haskell_docs_aspect = _real_haskell_docs_aspect

real_javascript_docs_aspect = _real_javascript_docs_aspect

real_julia_docs_aspect = _real_julia_docs_aspect

real_jvm_docs_aspect = _real_jvm_docs_aspect

real_lua_docs_aspect = _real_lua_docs_aspect

real_ocaml_docs_aspect = _real_ocaml_docs_aspect

real_perl_docs_aspect = _real_perl_docs_aspect

real_php_docs_aspect = _real_php_docs_aspect

real_powershell_docs_aspect = _real_powershell_docs_aspect

real_protobuf_docs_aspect = _real_protobuf_docs_aspect

real_python_docs_aspect = _real_python_docs_aspect

real_ruby_docs_aspect = _real_ruby_docs_aspect

real_rust_docs_aspect = _real_rust_docs_aspect

real_swift_docs_aspect = _real_swift_docs_aspect

real_typescript_docs_aspect = _real_typescript_docs_aspect

real_zig_docs_aspect = _real_zig_docs_aspect
# END GENERATED LANG ASPECT EXPORTS

_JVM_CHECK_TOOLS = ["javac", "kotlinc", "scalac"]

# The core runs the hooks in this order and no other.
_HOOK_ORDER = [
    "spotbugs",
    "dialyzer",
    "sorbet",
    "scalafix",
    "fsharplint",
    "jet",
] + _JVM_CHECK_TOOLS + ["errcheck", "gotype", "govet", "staticcheck"] + COMPILE_CHECK_TOOLS + [
    "eslint",
    "tsc",
    "roslyn",
    "csc",
    "fsc",
    "erlc",
    "elixirc",
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
    "taplo",
    "vale",
    "gofumpt",
    "trivy_image_config",
]

_CONTEXT_HOOKS = _JS_HOOKS | _CORE_HOOKS | _GENERATION_HOOKS | _CUE_HOOKS | _PKL_HOOKS | _JSONNET_HOOKS | _REGO_HOOKS | _BICEP_HOOKS | _TERRAFORM_HOOKS | _LUA_HOOKS | _PHP_HOOKS | _KUBERNETES_HOOKS | _OCI_IMAGE_HOOKS | _SQL_HOOKS | _DART_HOOKS | _OCAML_HOOKS | _BEAM_HOOKS | _GO_HOOKS | _POWERSHELL_HOOKS | _RUBY_HOOKS | _JULIA_HOOKS | _PYTHON_HOOKS | _CC_HOOKS | _DOTNET_HOOKS | _RUST_HOOKS | _JVM_HOOKS | {
    tool: compile_check_hook(tool)
    for tool in COMPILE_CHECK_TOOLS
}

_CFG = real_config(
    tools = _JS_TOOLS | _CORE_TOOLS | _ANSIBLE_TOOLS | _CLOUDFORMATION_TOOLS | _MAKE_TOOLS | _DOCKERFILE_TOOLS | _OCI_IMAGE_TOOLS | _HTML_TEMPLATE_TOOLS | _XML_TOOLS | _YAML_TOOLS | _NIX_TOOLS | _SHELL_TOOLS | _LATEX_TOOLS | _CMAKE_TOOLS | _TYPST_TOOLS | _TEXT_TOOLS | _SWIFT_TOOLS | _PERL_TOOLS | _CUE_TOOLS | _PKL_TOOLS | _JSONNET_TOOLS | _REGO_TOOLS | _BICEP_TOOLS | _TERRAFORM_TOOLS | _LUA_TOOLS | _PHP_TOOLS | _KUBERNETES_TOOLS | _SQL_TOOLS | _D_TOOLS | _DART_TOOLS | _OCAML_TOOLS | _BEAM_TOOLS | _GO_TOOLS | _POWERSHELL_TOOLS | _RUBY_TOOLS | _JULIA_TOOLS | _PYTHON_TOOLS | _JVM_TOOLS | _CC_TOOLS | _DOTNET_TOOLS | _RUST_TOOLS | _FORTRAN_TOOLS | _HASKELL_TOOLS | _GENERATION_TOOLS | _ZIG_TOOLS,
    shard_specs = _JS_SHARD_SPECS | _CORE_SHARD_SPECS | _ANSIBLE_SHARD_SPECS | _CLOUDFORMATION_SHARD_SPECS | _MAKE_SHARD_SPECS | _DOCKERFILE_SHARD_SPECS | _OCI_IMAGE_SHARD_SPECS | _HTML_TEMPLATE_SHARD_SPECS | _XML_SHARD_SPECS | _YAML_SHARD_SPECS | _NIX_SHARD_SPECS | _SHELL_SHARD_SPECS | _LATEX_SHARD_SPECS | _CMAKE_SHARD_SPECS | _TYPST_SHARD_SPECS | _TEXT_SHARD_SPECS | _SWIFT_SHARD_SPECS | _PERL_SHARD_SPECS | _CUE_SHARD_SPECS | _PKL_SHARD_SPECS | _JSONNET_SHARD_SPECS | _REGO_SHARD_SPECS | _BICEP_SHARD_SPECS | _TERRAFORM_SHARD_SPECS | _LUA_SHARD_SPECS | _PHP_SHARD_SPECS | _KUBERNETES_SHARD_SPECS | _SQL_SHARD_SPECS | _D_SHARD_SPECS | _DART_SHARD_SPECS | _OCAML_SHARD_SPECS | _BEAM_SHARD_SPECS | _GO_SHARD_SPECS | _POWERSHELL_SHARD_SPECS | _RUBY_SHARD_SPECS | _JULIA_SHARD_SPECS | _PYTHON_SHARD_SPECS | _JVM_SHARD_SPECS | _CC_SHARD_SPECS | _DOTNET_SHARD_SPECS | _RUST_SHARD_SPECS | _FORTRAN_SHARD_SPECS | _HASKELL_SHARD_SPECS | _GENERATION_SHARD_SPECS | _ZIG_SHARD_SPECS,
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
real_skip_notes = _real_skip_notes
real_unavailable_tools = _real_unavailable_tools
RealShardProbeInfo = _RealShardProbeInfo

def _make_real_aspect(shard_name):
    return real_make_aspect(_CFG, shard_name)

def _make_real_probe_aspect(capability):
    return real_make_probe_aspect(_CFG, capability)

real_lint_aspect = _real_lint_aspect

real_format_aspect = _real_format_aspect

real_typecheck_aspect = _real_typecheck_aspect

real_html_template_format_aspect = _real_html_template_format_aspect

real_html_template_lint_aspect = _real_html_template_lint_aspect

real_xml_lint_aspect = _real_xml_lint_aspect

real_yaml_format_aspect = _real_yaml_format_aspect

real_yaml_lint_aspect = _real_yaml_lint_aspect

real_shell_lint_aspect = _real_shell_lint_aspect

real_shell_format_aspect = _real_shell_format_aspect

real_oci_image_lint_aspect = _real_oci_image_lint_aspect
real_oci_image_typecheck_aspect = _real_oci_image_typecheck_aspect

real_nix_format_aspect = _real_nix_format_aspect

real_nix_lint_aspect = _real_nix_lint_aspect

real_typst_format_aspect = _real_typst_format_aspect

real_typst_lint_aspect = _real_typst_lint_aspect

real_latex_format_aspect = _real_latex_format_aspect

real_latex_lint_aspect = _real_latex_lint_aspect

real_text_lint_aspect = _real_text_lint_aspect

real_lint_probe_aspect = _make_real_probe_aspect("lint")

real_format_probe_aspect = _make_real_probe_aspect("format")

real_typecheck_probe_aspect = _make_real_probe_aspect("typecheck")

real_docs_aspect = _real_docs_aspect

real_docs_aspects = [
    real_docs_aspect,
    real_avro_docs_aspect,
    real_cc_docs_aspect,
    real_components_docs_aspect,
    real_d_docs_aspect,
    real_dart_docs_aspect,
    real_dotnet_docs_aspect,
    real_fortran_docs_aspect,
    real_go_docs_aspect,
    real_haskell_docs_aspect,
    real_javascript_docs_aspect,
    real_julia_docs_aspect,
    real_beam_docs_aspect,
    real_jvm_docs_aspect,
    real_lua_docs_aspect,
    real_ocaml_docs_aspect,
    real_perl_docs_aspect,
    real_php_docs_aspect,
    real_powershell_docs_aspect,
    real_protobuf_docs_aspect,
    real_python_docs_aspect,
    real_ruby_docs_aspect,
    real_rust_docs_aspect,
    real_swift_docs_aspect,
    real_typescript_docs_aspect,
    real_zig_docs_aspect,
]

real_cue_format_aspect = _real_cue_format_aspect

real_cue_typecheck_aspect = _real_cue_typecheck_aspect

real_jsonnet_format_aspect = _real_jsonnet_format_aspect

real_jsonnet_lint_aspect = _real_jsonnet_lint_aspect

real_core_format_probe_aspect = _real_core_format_probe_aspect

real_core_lint_probe_aspect = _real_core_lint_probe_aspect

real_core_typecheck_probe_aspect = _real_core_typecheck_probe_aspect
