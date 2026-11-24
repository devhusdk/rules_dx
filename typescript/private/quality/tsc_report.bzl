"""Test rule that exposes the tsc aspect output of one target."""

load("@bazel_skylib//rules:diff_test.bzl", "diff_test")
load("//core/private/testing:defs.bzl", "starlark_test")
load("//javascript:defs.bzl", "javascript_library")
load("//typescript/private/quality:tsc.bzl", "TscInfo", "tsc_aspect")

def _tsc_report_impl(ctx):
    info = ctx.attr.target[TscInfo]
    if ctx.attr.kind == "exit":
        return [DefaultInfo(files = depset([info.statuses[ctx.attr.index]]))]
    return [DefaultInfo(files = depset([info.logs[ctx.attr.index]]))]

tsc_report = rule(
    implementation = _tsc_report_impl,
    doc = "Exposes the tsc stdout or the exit status file of a target.",
    attrs = {
        "index": attr.int(doc = "Which checker of the target to expose.", default = 0),
        "kind": attr.string(doc = "Which tsc output to expose.", values = ["stdout", "exit"], mandatory = True),
        "target": attr.label(doc = "Target the tsc aspect runs on.", mandatory = True, aspects = [tsc_aspect]),
    },
)

def tsc_stdout_test(name, report, expected, diagnostics = "", size = "small"):
    """Declares the stdout check of one tsc report: a diff against the empty expected file, or one line per expected diagnostic without its path."""
    if diagnostics == "":
        diff_test(
            name = name,
            file1 = expected,
            file2 = report,
            size = size,
            testonly = True,
        )
        return
    starlark_test(
        name = name,
        file_checks = {report: diagnostics},
        mode = "execution",
        size = size,
        testonly = True,
    )

def tsc_framework_cases(library_rule, lib, srcs, exit_expected, stdout_expected = {}, deps = [], diagnostics = {}):
    """Declares a lib library and one library per case of a framework with their tsc reports and tests."""
    for case in srcs:
        if case not in exit_expected:
            fail("tsc_framework_cases case %r has no exit_expected file" % case)
        if (case in stdout_expected) == (case in diagnostics):
            fail("tsc_framework_cases case %r needs exactly one of stdout_expected or diagnostics" % case)
    javascript_library(
        name = "lib",
        srcs = [lib],
        testonly = True,
    )
    for case, src in srcs.items():
        library_rule(
            name = case,
            srcs = [src],
            deps = [":lib"] + deps,
            testonly = True,
        )
        for kind in ["stdout", "exit"]:
            tsc_report(
                name = "%s_%s_report" % (case, kind),
                kind = kind,
                target = ":" + case,
                testonly = True,
            )
        tsc_stdout_test(
            name = "%s_stdout_test" % case,
            report = ":%s_stdout_report" % case,
            expected = stdout_expected.get(case),
            diagnostics = diagnostics.get(case, ""),
            size = "small",
        )
        diff_test(
            name = "%s_exit_test" % case,
            file1 = exit_expected[case],
            file2 = ":%s_exit_report" % case,
            size = "small",
            testonly = True,
        )
