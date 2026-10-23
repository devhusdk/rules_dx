"""Analysis tests for the supported and unsupported target cells."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _unsupported_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, "target cell this platform is unsupported")
    return analysistest.end(env)

def _action_impl(ctx):
    env = analysistest.begin(ctx)
    action = [a for a in analysistest.target_actions(env) if a.mnemonic == "DxAppImage"][0]
    asserts.equals(env, None, action.env.get("PATH"))
    asserts.equals(env, "C", action.env.get("LC_ALL"))
    asserts.true(env, action.argv[0].endswith("/appimage_packer"), "action runs " + action.argv[0])
    asserts.true(env, ctx.attr.arch in action.argv, "arch " + ctx.attr.arch + " missing from " + str(action.argv))
    runtimes = [f.basename for f in action.inputs.to_list() if f.basename.startswith("runtime-")]
    asserts.equals(env, ["runtime-" + ctx.attr.arch], runtimes)
    return analysistest.end(env)

def _make_action_test(platform):
    return analysistest.make(
        _action_impl,
        attrs = {"arch": attr.string(mandatory = True)},
        config_settings = {"//command_line_option:platforms": str(Label(platform))},
    )

unsupported_cell_test = analysistest.make(
    _unsupported_impl,
    expect_failure = True,
    config_settings = {"//command_line_option:platforms": str(Label("//appimage/platforms:macos_arm64"))},
)

unsupported_cpu_test = analysistest.make(
    _unsupported_impl,
    expect_failure = True,
    config_settings = {"//command_line_option:platforms": str(Label("//appimage/platforms:linux_riscv64"))},
)

arm64_action_test = _make_action_test("//appimage/platforms:linux_arm64")

x86_64_action_test = _make_action_test("//appimage/platforms:linux_x86_64")
