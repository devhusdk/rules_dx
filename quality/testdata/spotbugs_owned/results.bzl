"""Exposes the real lint aspect results of one target as default files."""

load("//quality/testdata:real_aspect_subject.bzl", "real_aspect_subject")

def _results_impl(ctx):
    files = ctx.attr.subject[OutputGroupInfo].dx_results
    return [DefaultInfo(files = files, runfiles = ctx.runfiles(transitive_files = files))]

_results = rule(
    implementation = _results_impl,
    attrs = {"subject": attr.label(mandatory = True)},
)

def spotbugs_results(name, target):
    """Declares the lint results of target."""
    real_aspect_subject(name = name + "_subject", target = target, tags = ["manual"], testonly = True)
    _results(name = name, subject = name + "_subject", tags = ["manual"], testonly = True)
