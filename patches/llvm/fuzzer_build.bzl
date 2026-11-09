"""Builds its `deps` with the pinned LLVM's libFuzzer runtime enabled."""

def _fuzzer_transition_impl(_settings, _attr):
    return {"@llvm//config:fuzzer": True, "//cc/toolchains:compiler": "llvm"}

_fuzzer_transition = transition(
    implementation = _fuzzer_transition_impl,
    inputs = [],
    outputs = ["@llvm//config:fuzzer", "//cc/toolchains:compiler"],
)

def _fuzzer_build_impl(ctx):
    return [DefaultInfo(files = depset(transitive = [dep[DefaultInfo].files for dep in ctx.attr.deps]))]

fuzzer_build = rule(
    implementation = _fuzzer_build_impl,
    attrs = {"deps": attr.label_list(cfg = _fuzzer_transition)},
)
