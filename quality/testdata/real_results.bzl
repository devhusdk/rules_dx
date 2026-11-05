"""Runs the real aspect on a target and checks the result file for strings."""

load("@rules_shell//shell:sh_test.bzl", "sh_test")
load("//quality/testdata:real_aspect_subject.bzl", "real_aspect_subject")
load("//quality/testdata:real_typecheck_subject.bzl", "real_typecheck_subject")
load("//quality/testdata:runner_matrix_tests.bzl", "compatible")

_COMPILER = str(Label("//cc/toolchains:compiler"))

def _policy_transition_impl(settings, attr):
    """Selects the workspace policy and, when set, the C++ compiler."""
    return {
        _COMPILER: attr.compiler or settings[_COMPILER],
        "//config:tool_dialyzer": str(attr.tool_dialyzer),
        "//config:tool_govet": str(attr.tool_govet),
        "//config:workspace": attr.policy,
        "@rules_rust//rust/settings:clippy_output_diagnostics": True,
        "@rules_rust//rust/settings:extra_rustc_flags": [],
        "@rules_rust//rust/settings:extra_rustc_flag": attr.rustc_flags,
        "@rules_rust//rust/settings:rustc_output_diagnostics": True,
    }

_policy_transition = transition(
    implementation = _policy_transition_impl,
    inputs = [_COMPILER],
    outputs = [
        _COMPILER,
        "//config:tool_dialyzer",
        "//config:tool_govet",
        "//config:workspace",
        "@rules_rust//rust/settings:clippy_output_diagnostics",
        "@rules_rust//rust/settings:extra_rustc_flags",
        "@rules_rust//rust/settings:extra_rustc_flag",
        "@rules_rust//rust/settings:rustc_output_diagnostics",
    ],
)

def _results_impl(ctx):
    files = depset(transitive = [subject[OutputGroupInfo].dx_results for subject in ctx.attr.subject])
    return [DefaultInfo(files = files, runfiles = ctx.runfiles(transitive_files = files))]

_results = rule(
    implementation = _results_impl,
    attrs = {
        "compiler": attr.string(),
        "policy": attr.string(mandatory = True),
        "rustc_flags": attr.string_list(),
        "tool_dialyzer": attr.label(default = "//erlang/tools:dialyzer"),
        "tool_govet": attr.label(default = "//go/tools/analysis/cmd/dx_govet"),
        "subject": attr.label(mandatory = True, cfg = _policy_transition),
    },
)

def real_result_test(name, target, policy, contains = [], absent = [], tool_dialyzer = None, tool_govet = None, compiler = "", typecheck_subject = False, rustc_flags = [], target_compatible_with = None, **flags):
    """Declares a test that greps the real aspect result of target under policy."""
    subject_rule = real_typecheck_subject if typecheck_subject else real_aspect_subject
    subject_rule(name = name + "_subject", target = target, tags = ["manual"], testonly = True)
    _results(
        name = name + "_results",
        compiler = compiler,
        rustc_flags = rustc_flags,
        policy = str(native.package_relative_label(policy)),
        tool_dialyzer = tool_dialyzer or "//erlang/tools:dialyzer",
        tool_govet = tool_govet or "//go/tools/analysis/cmd/dx_govet",
        subject = name + "_subject",
        tags = ["manual"],
        testonly = True,
    )
    sh_test(
        name = name,
        srcs = ["//quality/testdata:real_result_check.sh"],
        args = ["$(rootpaths :" + name + "_results)"] + ["+" + s for s in contains] + ["-" + s for s in absent],
        data = [":" + name + "_results"],
        size = "small",
        tags = ["no-coverage"],
        target_compatible_with = target_compatible_with or compatible(
            flags.get("macos_arm64", False),
            flags.get("windows_x86_64", False),
            flags.get("linux_arm64", False),
            flags.get("windows_arm64", False),
        ),
    )
