"""Analysis proofs for the consumer-selected workspace policy."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//cc/private:clang_syntax.bzl", "clang_syntax_aspect")
load(
    "//core/quality/testdata:all_languages.bzl",
    "real_cc_format_aspect",
    "real_cc_lint_aspect",
    "real_cc_typecheck_aspect",
    "real_cue_format_aspect",
    "real_dockerfile_lint_aspect",
    "real_oci_image_lint_aspect",
    "real_make_lint_aspect",
    "real_dotnet_format_aspect",
    "real_dotnet_lint_aspect",
    "real_dotnet_typecheck_aspect",
    "real_capnp_lint_aspect",
    "real_flatbuffers_lint_aspect",
    "real_thrift_lint_aspect",
    "real_avro_lint_aspect",
    "real_format_aspect",
    "real_go_format_aspect",
    "real_go_lint_aspect",
    "real_go_typecheck_aspect",
    "real_haskell_format_aspect",
    "real_haskell_lint_aspect",
    "real_latex_lint_aspect",
    "real_haskell_typecheck_aspect",
    "real_html_template_format_aspect",
    "real_html_template_lint_aspect",
    "real_js_format_aspect",
    "real_js_lint_aspect",
    "real_jsonnet_format_aspect",
    "real_fortran_format_aspect",
    "real_fortran_lint_aspect",
    "real_jsonnet_lint_aspect",
    "real_latex_format_aspect",
    "real_typst_format_aspect",
    "real_typst_lint_aspect",
    "real_jvm_format_aspect",
    "real_jvm_lint_aspect",
    "real_lint_aspect",
    "real_lua_format_aspect",
    "real_lua_lint_aspect",
    "real_ocaml_lint_aspect",
    "real_cmake_format_aspect",
    "real_make_format_aspect",
    "real_cmake_lint_aspect",
    "real_nix_format_aspect",
    "real_nix_lint_aspect",
    "real_ocaml_format_aspect",
    "real_ocaml_typecheck_aspect",
    "real_kubernetes_format_aspect",
    "real_kubernetes_lint_aspect",
    "real_ansible_lint_aspect",
    "real_bicep_format_aspect",
    "real_bicep_lint_aspect",
    "real_cloudformation_lint_aspect",
    "real_sql_format_aspect",
    "real_sql_lint_aspect",
    "real_pkl_format_aspect",
    "real_pkl_lint_aspect",
    "real_pkl_typecheck_aspect",
    "real_terraform_typecheck_aspect",
    "real_powershell_format_aspect",
    "real_powershell_lint_aspect",
    "real_protobuf_format_aspect",
    "real_protobuf_lint_aspect",
    "real_protobuf_typecheck_aspect",
    "real_python_lint_aspect",
    "real_typecheck_probe_aspect",
    "real_dart_format_aspect",
    "real_dart_lint_aspect",
    "real_dart_typecheck_aspect",
    "real_d_format_aspect",
    "real_d_lint_aspect",
    "real_perl_format_aspect",
    "real_perl_lint_aspect",
    "real_python_format_aspect",
    "real_python_typecheck_aspect",
    "real_julia_format_aspect",
    "real_swift_format_aspect",
    "real_swift_lint_aspect",
    "real_zig_format_aspect",
    "real_zig_lint_aspect",
    "real_julia_lint_aspect",
    "real_rego_format_aspect",
    "real_rego_lint_aspect",
    "real_js_typecheck_aspect",
    "real_jvm_typecheck_aspect",
    "real_d_typecheck_aspect",
    "real_fortran_typecheck_aspect",
    "real_zig_typecheck_aspect",
    "real_rego_typecheck_aspect",
    "real_capnp_typecheck_aspect",
    "real_flatbuffers_typecheck_aspect",
    "real_thrift_typecheck_aspect",
    "real_avro_typecheck_aspect",
    "real_cue_typecheck_aspect",
    "real_erlang_format_aspect",
    "real_erlang_lint_aspect",
    "real_erlang_typecheck_aspect",
    "real_elixir_format_aspect",
    "real_elixir_lint_aspect",
    "real_php_lint_aspect",
    "real_php_typecheck_aspect",
    "real_julia_typecheck_aspect",
    "real_lua_typecheck_aspect",
    "real_ruby_typecheck_aspect",
    "real_ruby_format_aspect",
    "real_ruby_lint_aspect",
    "real_rust_format_aspect",
    "real_rust_lint_aspect",
    "real_scala_format_aspect",
    "real_scala_lint_aspect",
    "real_shell_format_aspect",
    "real_shell_lint_aspect",
    "real_terraform_format_aspect",
    "real_terraform_lint_aspect",
    "real_text_lint_aspect",
    "real_xml_lint_aspect",
    "real_yaml_format_aspect",
    "real_yaml_lint_aspect",
)

def _policy_transition_impl(_settings, attr):
    """Selects the workspace policy named by another attribute of the same rule."""
    return {"//config:workspace": str(attr.policy_target)}

_policy_transition = transition(
    implementation = _policy_transition_impl,
    inputs = [],
    outputs = ["//config:workspace"],
)

_ASPECTS = [
    real_cc_format_aspect,
    real_cc_lint_aspect,
    real_cc_typecheck_aspect,
    real_cue_format_aspect,
    real_jsonnet_format_aspect,
    real_fortran_format_aspect,
    real_fortran_lint_aspect,
    real_jsonnet_lint_aspect,
    real_typst_format_aspect,
    real_typst_lint_aspect,
    real_latex_format_aspect,
    real_lua_format_aspect,
    real_lua_lint_aspect,
    real_ocaml_lint_aspect,
    real_cmake_format_aspect,
    real_make_format_aspect,
    real_cmake_lint_aspect,
    real_haskell_format_aspect,
    real_haskell_lint_aspect,
    real_latex_lint_aspect,
    real_haskell_typecheck_aspect,
    real_pkl_format_aspect,
    real_pkl_lint_aspect,
    real_pkl_typecheck_aspect,
    real_terraform_typecheck_aspect,
    real_dotnet_format_aspect,
    real_dotnet_lint_aspect,
    real_dotnet_typecheck_aspect,
    real_dockerfile_lint_aspect,
    real_oci_image_lint_aspect,
    real_make_lint_aspect,
    real_capnp_lint_aspect,
    real_flatbuffers_lint_aspect,
    real_thrift_lint_aspect,
    real_avro_lint_aspect,
    real_format_aspect,
    real_go_format_aspect,
    real_go_lint_aspect,
    real_go_typecheck_aspect,
    real_js_format_aspect,
    real_js_lint_aspect,
    real_jvm_format_aspect,
    real_jvm_lint_aspect,
    real_lint_aspect,
    real_nix_format_aspect,
    real_nix_lint_aspect,
    real_ocaml_format_aspect,
    real_ocaml_typecheck_aspect,
    real_kubernetes_format_aspect,
    real_kubernetes_lint_aspect,
    real_ansible_lint_aspect,
    real_bicep_format_aspect,
    real_bicep_lint_aspect,
    real_cloudformation_lint_aspect,
    real_sql_format_aspect,
    real_sql_lint_aspect,
    real_protobuf_format_aspect,
    real_powershell_format_aspect,
    real_powershell_lint_aspect,
    real_protobuf_lint_aspect,
    real_protobuf_typecheck_aspect,
    real_python_lint_aspect,
    real_dart_format_aspect,
    real_dart_lint_aspect,
    real_dart_typecheck_aspect,
    real_d_format_aspect,
    real_d_lint_aspect,
    real_perl_format_aspect,
    real_perl_lint_aspect,
    real_python_format_aspect,
    real_python_typecheck_aspect,
    real_julia_format_aspect,
    real_swift_format_aspect,
    real_swift_lint_aspect,
    real_zig_format_aspect,
    real_zig_lint_aspect,
    real_julia_lint_aspect,
    real_rego_format_aspect,
    real_rego_lint_aspect,
    real_js_typecheck_aspect,
    real_jvm_typecheck_aspect,
    real_d_typecheck_aspect,
    real_fortran_typecheck_aspect,
    real_zig_typecheck_aspect,
    real_rego_typecheck_aspect,
    real_capnp_typecheck_aspect,
    real_flatbuffers_typecheck_aspect,
    real_thrift_typecheck_aspect,
    real_avro_typecheck_aspect,
    real_cue_typecheck_aspect,
    real_erlang_format_aspect,
    real_erlang_lint_aspect,
    real_erlang_typecheck_aspect,
    real_elixir_format_aspect,
    real_elixir_lint_aspect,
    real_php_lint_aspect,
    real_php_typecheck_aspect,
    real_julia_typecheck_aspect,
    real_lua_typecheck_aspect,
    real_ruby_typecheck_aspect,
    real_ruby_format_aspect,
    real_ruby_lint_aspect,
    real_rust_format_aspect,
    real_rust_lint_aspect,
    real_scala_format_aspect,
    real_scala_lint_aspect,
    real_shell_format_aspect,
    real_shell_lint_aspect,
    real_terraform_format_aspect,
    real_terraform_lint_aspect,
    real_text_lint_aspect,
    real_html_template_format_aspect,
    real_html_template_lint_aspect,
    real_xml_lint_aspect,
    real_yaml_format_aspect,
    real_yaml_lint_aspect,
]

def _dx_result_names(target):
    """Returns the sorted real-aspect result names published for one target."""
    if OutputGroupInfo not in target:
        return []
    groups = target[OutputGroupInfo]
    if "dx_results" not in groups:
        return []
    return sorted([f.basename for f in groups["dx_results"].to_list()])

def _consumer_policy_impl(ctx):
    dep = ctx.attr.subject
    subject = dep[0] if type(dep) == "list" else dep
    results = _dx_result_names(subject)
    if results != ctx.attr.expected_dx_results:
        fail("consumer policy " + str(ctx.attr.policy_target) + " publishes " +
             str(results) + " real result(s) for " + str(subject.label) +
             ", want " + str(ctx.attr.expected_dx_results))
    windows = ctx.target_platform_has_constraint(ctx.attr._windows_os[platform_common.ConstraintValueInfo])
    out = ctx.actions.declare_file(ctx.label.name + (".bat" if windows else ".sh"))
    ctx.actions.write(
        output = out,
        content = "@exit /b 0\r\n" if windows else "#!/bin/sh\nexit 0\n",
        is_executable = True,
    )
    return [DefaultInfo(executable = out)]

_consumer_policy_test = rule(
    implementation = _consumer_policy_impl,
    test = True,
    attrs = {
        "expected_dx_results": attr.string_list(
            mandatory = True,
        ),
        "policy_target": attr.label(
            mandatory = True,
        ),
        "subject": attr.label(
            aspects = _ASPECTS,
            cfg = _policy_transition,
            mandatory = True,
        ),
        "_windows_os": attr.label(
            default = "@platforms//os:windows",
        ),
    },
)

def _ruff_config_inputs(env):
    paths = []
    for action in analysistest.target_under_test(env).actions:
        if action.mnemonic.startswith("DxRealQuality"):
            paths += [f.short_path for f in action.inputs.to_list() if f.basename == "ruff.toml"]
    return sorted(paths)

def _policy_config_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.equals(env, [ctx.attr.expected_config], _ruff_config_inputs(env))
    return analysistest.end(env)

_policy_config_test = analysistest.make(
    _policy_config_impl,
    attrs = {"expected_config": attr.string(mandatory = True)},
    config_settings = {str(Label("//config:workspace")): str(Label("//core/quality/tests/fixtures/consumer_policy:python_ruff_policy"))},
    extra_target_under_test_aspects = [real_python_lint_aspect],
)

def _policy_clang_tidy_impl(ctx):
    env = analysistest.begin(ctx)
    tidy_inputs = []
    for action in analysistest.target_under_test(env).actions:
        if action.mnemonic == "ClangTidy":
            tidy_inputs += [f.basename for f in action.inputs.to_list()]
    asserts.true(env, ".clang-tidy" in tidy_inputs, "want .clang-tidy in the clang-tidy inputs, got " + str(tidy_inputs))
    return analysistest.end(env)

_policy_clang_tidy_test = analysistest.make(
    _policy_clang_tidy_impl,
    config_settings = {str(Label("//config:workspace")): str(Label("//core/quality/tests/fixtures/consumer_policy:cc_clang_tidy_bound_policy"))},
    extra_target_under_test_aspects = [clang_syntax_aspect],
)

def _policy_generated_impl(ctx):
    env = analysistest.begin(ctx)
    return analysistest.end(env)

_policy_generated_test = analysistest.make(
    _policy_generated_impl,
    config_settings = {str(Label("//config:workspace")): str(Label("//core/quality/tests/fixtures/generated_policy:policy"))},
    extra_target_under_test_aspects = [real_typecheck_probe_aspect],
)

def _tier_base_inputs(env):
    paths = {}
    for action in analysistest.target_under_test(env).actions:
        if action.mnemonic.startswith("DxRealQuality"):
            for f in action.inputs.to_list():
                if f.short_path.startswith("core/quality/tiers/ruff/") or f.short_path.startswith("core/quality/tiers/ty/"):
                    paths[f.short_path] = True
    return sorted(paths.keys())

def _tier_base_impl(ctx):
    env = analysistest.begin(ctx)
    profile = ctx.attr.expected_profile
    want = ["core/quality/tiers/ruff/%s.toml" % profile, "core/quality/tiers/ty/%s.toml" % profile]
    asserts.equals(env, want, _tier_base_inputs(env))
    return analysistest.end(env)

_TIER_BASE_ATTRS = {"expected_profile": attr.string(mandatory = True)}
_PYTHON_ASPECTS = [real_python_lint_aspect, real_python_typecheck_aspect]

_tier_base_default_test = analysistest.make(
    _tier_base_impl,
    attrs = _TIER_BASE_ATTRS,
    config_settings = {
        str(Label("//config:config_profile")): "standard",
        str(Label("//config:workspace")): str(Label("//config:default_workspace_policy")),
    },
    extra_target_under_test_aspects = _PYTHON_ASPECTS,
)

_tier_base_default_strict_test = analysistest.make(
    _tier_base_impl,
    attrs = _TIER_BASE_ATTRS,
    config_settings = {
        str(Label("//config:config_profile")): "strict",
        str(Label("//config:workspace")): str(Label("//config:default_workspace_policy")),
    },
    extra_target_under_test_aspects = _PYTHON_ASPECTS,
)

# The workspace profile stays standard, so strict bases can only come from the generated policy.
_tier_base_generated_strict_test = analysistest.make(
    _tier_base_impl,
    attrs = _TIER_BASE_ATTRS,
    config_settings = {
        str(Label("//config:config_profile")): "standard",
        str(Label("//config:workspace")): str(Label("//core/quality/tests/fixtures/consumer_policy:strict_generated_policy")),
    },
    extra_target_under_test_aspects = _PYTHON_ASPECTS,
)

def policy_config_tests(name):
    """Proves policy configs bind a target, aspect_hints override them, tier bases follow the profile and a rendered policy analyzes."""
    pkg = "core/quality/tests/fixtures/consumer_policy/"
    _policy_config_test(
        name = name + "_policy",
        size = "small",
        expected_config = pkg + "ruff.toml",
        target_under_test = "//core/quality/tests/fixtures/consumer_policy:policy_bound_py",
    )
    _policy_config_test(
        name = name + "_hint",
        size = "small",
        expected_config = pkg + "override/ruff.toml",
        target_under_test = "//core/quality/tests/fixtures/consumer_policy:hinted_py",
    )
    _policy_clang_tidy_test(
        name = name + "_clang_tidy",
        size = "small",
        target_under_test = "//core/quality/tests/fixtures/consumer_policy:policy_bound_c",
    )
    _policy_generated_test(
        name = name + "_generated",
        size = "small",
        target_under_test = "//core/quality/tests/fixtures/consumer_policy:policy_bound_py",
    )
    for suffix, test, profile in [
        ("_tier_base_default", _tier_base_default_test, "standard"),
        ("_tier_base_default_strict", _tier_base_default_strict_test, "strict"),
        ("_tier_base_generated_strict", _tier_base_generated_strict_test, "strict"),
    ]:
        test(
            name = name + suffix,
            size = "small",
            expected_profile = profile,
            target_under_test = "//core/quality/tests/fixtures/consumer_policy:policy_bound_py",
        )
    native.test_suite(name = name, tests = [
        name + "_policy",
        name + "_hint",
        name + "_clang_tidy",
        name + "_generated",
        name + "_tier_base_default",
        name + "_tier_base_default_strict",
        name + "_tier_base_generated_strict",
    ])

def consumer_policy_tests(name, cases, **kwargs):
    """Instantiates one proof per consumer policy selection."""
    kwargs.setdefault("size", "small")
    for case in cases:
        _consumer_policy_test(
            name = name + "_" + case["case"] + "_test",
            expected_dx_results = case["expected_dx_results"],
            policy_target = case["policy"],
            subject = case["subject"],
            **kwargs
        )
