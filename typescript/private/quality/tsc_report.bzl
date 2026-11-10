"""Test rule that exposes the tsc aspect output of one target."""

load("@bazel_skylib//rules:diff_test.bzl", "diff_test")
load("//core:defs.bzl", "starlark_test")
load("//javascript:defs.bzl", "javascript_library")
load("//typescript/private/quality:tsc.bzl", "TscInfo", "tsc_aspect")

def _tsc_report_impl(ctx):
    info = ctx.attr.target[TscInfo]
    if ctx.attr.kind == "exit":
        return [DefaultInfo(files = depset([info.status]))]
    return [DefaultInfo(files = depset([info.stdout]))]

tsc_report = rule(
    implementation = _tsc_report_impl,
    doc = "Exposes the tsc stdout or the exit status file of a target.",
    attrs = {
        "kind": attr.string(doc = "Which tsc output to expose.", values = ["stdout", "exit"], mandatory = True),
        "target": attr.label(doc = "Target the tsc aspect runs on.", mandatory = True, aspects = [tsc_aspect]),
    },
)

def tsc_stdout_test(name, report, expected, diagnostics = ""):
    """Declares the stdout check of one tsc report: a diff against the empty expected file, or one line per expected diagnostic without its path."""
    if diagnostics == "":
        diff_test(
            name = name,
            file1 = expected,
            file2 = report,
            size = "small",
            testonly = True,
        )
        return
    starlark_test(
        name = name,
        file_checks = {report: diagnostics},
        mode = "execution",
        size = "small",
        testonly = True,
    )

def tsc_framework_cases(library_rule, ext, deps = [], diagnostics = {}):
    """Declares the lib, bad, broken and clean libraries of one framework with their tsc reports and tests; diagnostics maps a case to its expected diagnostic lines."""
    javascript_library(
        name = "lib",
        srcs = ["lib.js"],
        testonly = True,
    )
    cases = ["bad", "broken", "clean"]
    for case in cases:
        library_rule(
            name = case,
            srcs = ["%s.%s" % (case, ext)],
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
            if kind == "stdout":
                tsc_stdout_test(
                    name = "%s_stdout_test" % case,
                    report = ":%s_stdout_report" % case,
                    expected = "%s.stdout.expected" % case,
                    diagnostics = diagnostics.get(case, ""),
                )
            else:
                diff_test(
                    name = "%s_exit_test" % case,
                    file1 = "%s.exit.expected" % case,
                    file2 = ":%s_exit_report" % case,
                    size = "small",
                    testonly = True,
                )
