"""Exposes the zanuda lint results of one target under the zanuda_owned policy."""

load("//quality/testdata:real_aspect_subject.bzl", "real_aspect_subject")

def _policy_transition_impl(_settings, _attr):
    """Selects the zanuda_owned workspace policy."""
    return {"//config:workspace": str(Label("//quality/testdata/zanuda_owned:policy"))}

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
    attrs = {"subject": attr.label(mandatory = True, cfg = _policy_transition)},
)

def zanuda_results(name, target):
    """Declares the zanuda lint results of target."""
    real_aspect_subject(name = name + "_subject", target = target, tags = ["manual"], testonly = True)
    _results(name = name, subject = name + "_subject", tags = ["manual"], testonly = True)
