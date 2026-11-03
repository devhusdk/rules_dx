"""Runs the real aspect on a target and checks the result file for strings."""

load("@rules_shell//shell:sh_test.bzl", "sh_test")
load("//quality/testdata:real_aspect_subject.bzl", "real_aspect_subject")
load("//quality/testdata:runner_matrix_tests.bzl", "compatible")

def _policy_transition_impl(_settings, attr):
    """Selects the workspace policy named by the policy attr."""
    return {"//config:workspace": attr.policy}

_policy_transition = transition(
    implementation = _policy_transition_impl,
    inputs = [],
    outputs = ["//config:workspace"],
)

def _results_impl(ctx):
    files = depset(transitive = [subject[OutputGroupInfo].dx_results for subject in ctx.attr.subject])
    return [DefaultInfo(files = files, runfiles = ctx.runfiles(transitive_files = files))]

_results = rule(
    implementation = _results_impl,
    attrs = {
        "policy": attr.string(mandatory = True),
        "subject": attr.label(mandatory = True, cfg = _policy_transition),
    },
)

def real_result_test(name, target, policy, contains = [], absent = [], **flags):
    """Declares a test that greps the real aspect result of target under policy."""
    real_aspect_subject(name = name + "_subject", target = target, tags = ["manual"], testonly = True)
    _results(
        name = name + "_results",
        policy = str(native.package_relative_label(policy)),
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
        target_compatible_with = compatible(
            flags.get("macos_arm64", False),
            flags.get("windows_x86_64", False),
            flags.get("linux_arm64", False),
            flags.get("windows_arm64", False),
        ),
    )
