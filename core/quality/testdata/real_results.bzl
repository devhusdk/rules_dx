"""Runs the real aspect on a target and checks the result file for strings."""

load("@rules_shell//shell:sh_test.bzl", "sh_test")
load("//core/quality/testdata:config_subject.bzl", "config_results")
load("//core/quality/testdata:real_aspect_subject.bzl", "REAL_SUBJECT_BY_CLASS")
load("//core/quality/testdata:real_typecheck_subject.bzl", "real_typecheck_subject")
load("//core/quality/testdata:runner_matrix_tests.bzl", "compatible")

def _quote(s):
    return "'" + s.replace("'", "'\\''") + "'"

def check_args(contains, absent):
    """Returns real_result_check.sh args; quoted because Bazel shell-tokenizes args."""
    return [_quote("+" + s) for s in contains] + [_quote("-" + s) for s in absent]

def real_result_test(name, target, file_class, config, contains = [], absent = [], typecheck_subject = False, target_compatible_with = None, **flags):
    """Declares a test that greps the real aspect result of target under config."""
    if file_class not in REAL_SUBJECT_BY_CLASS:
        fail("%s: unknown file class %s" % (name, file_class))
    subject_rule = real_typecheck_subject if typecheck_subject else REAL_SUBJECT_BY_CLASS[file_class]
    subject_rule(name = name + "_subject", target = target, tags = ["manual"], testonly = True)
    config_results(
        name = name + "_results",
        config = config,
        subject = name + "_subject",
        tags = ["manual"],
        testonly = True,
    )
    sh_test(
        name = name,
        srcs = ["//core/quality/testdata:real_result_check.sh"],
        args = ["$(rootpaths :" + name + "_results)"] + check_args(contains, absent),
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
