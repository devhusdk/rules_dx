"""Tests that a typescript_binary or typescript_test typechecks through the dx tsc gate."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")
load("//core:defs.bzl", "expect_equal", "starlark_test")

def _reads_exit_impl(ctx):
    env = analysistest.begin(ctx)
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic == "TscGate"]
    asserts.equals(env, 1, len(actions))
    inputs = [f.path for f in actions[0].inputs.to_list()]
    asserts.true(env, any([p.endswith("dx_tsc/" + ctx.attr.project + ".exit") for p in inputs]), "no dx_tsc/%s.exit input in %s" % (ctx.attr.project, inputs))
    return analysistest.end(env)

_reads_exit_test = analysistest.make(
    _reads_exit_impl,
    attrs = {"project": attr.string(mandatory = True)},
)

def typecheck_gate_test(name, target):
    """Asserts the gate of `target`, declared earlier in this package, reads its tsc exit status and the stock typecheck test is manual."""
    _reads_exit_test(
        name = name + "_exit_test",
        size = "small",
        target_under_test = ":" + target + "_typecheck",
        project = target + "_ts",
    )
    rule = native.existing_rule(target + "_ts_typecheck_test")
    if rule == None:
        fail("typecheck_gate_test: %s_ts_typecheck_test is not declared" % target)
    starlark_test(
        name = name + "_manual_test",
        mode = "unit",
        checks = [expect_equal("stock typecheck test is manual", "manual" in rule.get("tags", []), True)],
    )
