"""Test rule that exposes the tsc aspect output of one target."""

load("@bazel_skylib//rules:diff_test.bzl", "diff_test")
load("//javascript:defs.bzl", "javascript_library")
load("//typescript/private/quality:tsc.bzl", "TscInfo", "tsc_aspect")

def _tsc_report_impl(ctx):
    info = ctx.attr.target[TscInfo]
    if ctx.attr.kind == "exit":
        return [DefaultInfo(files = depset([info.status]))]
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    # run_shell: A pipe and a redirect normalize the output.
    ctx.actions.run_shell(
        inputs = [info.stdout],
        outputs = [out],
        command = "tr -d '\\r' < \"$1\" | tr '\\\\' / > \"$2\"",
        arguments = [info.stdout.path, out.path],
        mnemonic = "TscReportNormalize",
    )
    return [DefaultInfo(files = depset([out]))]

tsc_report = rule(
    implementation = _tsc_report_impl,
    doc = "Exposes the tsc stdout, with CRs removed and backslashes turned into slashes, or the exit status file of a target.",
    attrs = {
        "kind": attr.string(doc = "Which tsc output to expose.", values = ["stdout", "exit"], mandatory = True),
        "target": attr.label(doc = "Target the tsc aspect runs on.", mandatory = True, aspects = [tsc_aspect]),
    },
)

def tsc_framework_cases(library_rule, ext, deps = []):
    """Declares the lib, bad, broken and clean libraries of one framework with their tsc reports and diff tests."""
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
            diff_test(
                name = "%s_%s_test" % (case, kind),
                file1 = "%s.%s.expected" % (case, kind),
                file2 = ":%s_%s_report" % (case, kind),
                testonly = True,
            )
