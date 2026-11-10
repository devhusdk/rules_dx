"""Analysis tests for the inputs and environment of OCaml build actions."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

_LINUX = ["@platforms//os:linux", "@platforms//cpu:x86_64"]
_EMITTER = str(Label("//ocaml/private/toolchain:emitter"))

def _inputs_impl(ctx):
    env = analysistest.begin(ctx)
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic == "CompileOCamlModule"]
    asserts.equals(env, 1, len(actions), "expected one CompileOCamlModule action")
    inputs = [f.short_path for f in actions[0].inputs.to_list()]
    found = [p for p in inputs if p.endswith("/" + ctx.attr.file)]
    asserts.equals(env, ctx.attr.present, len(found) > 0, "inputs: " + str(inputs))
    return analysistest.end(env)

_inputs_test = analysistest.make(_inputs_impl, attrs = {
    "file": attr.string(),
    "present": attr.bool(),
})

def ocaml_compile_input_test(name, target, file, present):
    """Checks whether the module's compile action receives file as an input."""
    _inputs_test(
        name = name,
        size = "small",
        target_under_test = target,
        file = file,
        present = present,
        target_compatible_with = _LINUX,
    )

def _hermetic_impl(ctx):
    env = analysistest.begin(ctx)
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic.startswith("LinkOCaml")]
    asserts.true(env, len(actions) > 0, "no link action")
    for action in actions:
        for name in ["CONDA_OCAML_AR", "CONDA_OCAML_AS", "CONDA_OCAML_CC", "CONDA_OCAML_MKEXE"]:
            value = action.env.get(name, "")
            asserts.true(env, value.startswith("external/"), name + "=" + value)
        asserts.true(env, action.env.get("PATH", "").startswith("external/"), "PATH=" + action.env.get("PATH", ""))
    return analysistest.end(env)

_hermetic_test = analysistest.make(
    _hermetic_impl,
    config_settings = {_EMITTER: "sys"},
)

def ocaml_hermetic_link_test(name, target):
    """Checks the native link action names hermetic C tools and no host path."""
    _hermetic_test(
        name = name,
        size = "small",
        target_under_test = target,
        target_compatible_with = _LINUX,
    )

def _link_archive_impl(ctx):
    env = analysistest.begin(ctx)
    actions = [a for a in analysistest.target_actions(env) if a.mnemonic.startswith("LinkOCaml")]
    asserts.true(env, len(actions) > 0, "no link action")
    argv = [arg for action in actions for arg in action.argv]
    found = [arg for arg in argv if arg.endswith("/" + ctx.attr.archive)]
    asserts.true(env, len(found) > 0, "argv: " + str(argv))
    return analysistest.end(env)

_link_archive_test = analysistest.make(
    _link_archive_impl,
    attrs = {"archive": attr.string()},
    config_settings = {_EMITTER: "sys"},
)

def ocaml_link_archive_test(name, target, archive):
    """Checks the native link action passes the C stub archive on its command line."""
    _link_archive_test(
        name = name,
        size = "small",
        target_under_test = target,
        archive = archive,
        target_compatible_with = _LINUX,
    )
