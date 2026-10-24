"""Analysis tests for the MSIX cells, toolchain states and attribute checks."""

load("@bazel_skylib//lib:unittest.bzl", "analysistest", "asserts")

def _fake_exe_impl(ctx):
    exe = ctx.actions.declare_file(ctx.attr.out)
    ctx.actions.write(exe, "MZ", is_executable = True)
    return [DefaultInfo(executable = exe, files = depset([exe]))]

fake_exe = rule(
    implementation = _fake_exe_impl,
    doc = "Declares a placeholder executable for analysis-only fixtures.",
    attrs = {"out": attr.string(doc = "Output file name.", mandatory = True)},
    executable = True,
)

def _failure_impl(ctx):
    env = analysistest.begin(ctx)
    asserts.expect_failure(env, ctx.attr.expected)
    return analysistest.end(env)

def _failure_test(platform, extra_toolchains = []):
    settings = {"//command_line_option:platforms": str(Label(platform))}
    if extra_toolchains:
        settings["//command_line_option:extra_toolchains"] = [str(Label(t)) for t in extra_toolchains]
    return analysistest.make(
        _failure_impl,
        expect_failure = True,
        attrs = {"expected": attr.string(mandatory = True)},
        config_settings = settings,
    )

def _action_impl(ctx):
    env = analysistest.begin(ctx)
    actions = analysistest.target_actions(env)
    pack = [a for a in actions if a.mnemonic == "DxMsix"]
    asserts.equals(env, 1, len(pack))
    action = pack[0]
    asserts.true(env, action.argv[0].replace("\\", "/").split("/")[-1].startswith("pwsh"), "action runs " + action.argv[0])
    asserts.true(env, action.argv[-2].endswith("msix/pack.ps1"), "packer missing from " + str(action.argv))
    asserts.equals(env, None, action.env.get("PATH"))
    asserts.equals(env, "1", action.env.get("POWERSHELL_TELEMETRY_OPTOUT"))
    request = [a for a in actions if a.outputs.to_list()[0].path == action.argv[-1]][0]
    spec = json.decode(request.content)
    asserts.equals(env, ctx.attr.arch, spec["arch"])
    asserts.equals(env, "Dx.Cell", spec["name"])
    asserts.equals(env, "Dx Cell", spec["publisher_display_name"])
    asserts.equals(env, "fake_makeappx.exe", spec["makeappx"].split("/")[-1])
    asserts.equals(
        env,
        [["exe", "cell.exe"], ["logo", "Assets/logo.png"], ["lib", "cell.dll"], ["data", "share/fixtures/greeting.txt"], ["notice", "licenses/NOTICE.txt"]],
        [[f["kind"], f["dest"]] for f in spec["files"]],
    )
    inputs = [f.basename for f in action.inputs.to_list()]
    for name in ["cell.exe", "cell.dll", "greeting.txt", "NOTICE.txt", "logo.png", "fake_makeappx.exe", "pack.ps1"]:
        asserts.true(env, name in inputs, name + " missing from the packaging inputs")
    outputs = sorted([f.basename for f in action.outputs.to_list()])
    asserts.equals(env, ["cell.msix", "cell.msix.identity.json"], outputs)
    return analysistest.end(env)

def _action_test(platform):
    return analysistest.make(
        _action_impl,
        attrs = {"arch": attr.string(mandatory = True)},
        config_settings = {
            "//command_line_option:extra_toolchains": [str(Label("//msix/tests:fake_toolchain"))],
            "//command_line_option:platforms": str(Label(platform)),
        },
    )

unsupported_cell_test = _failure_test("//msix/platforms:windows_x86_32")

no_windows_exec_test = _failure_test("//msix/platforms:windows_x86_64")

unaccepted_license_test = _failure_test("//msix/platforms:windows_x86_64", ["//msix/tests:unaccepted_toolchain"])

attribute_failure_test = _failure_test("//msix/platforms:windows_x86_64", ["//msix/tests:fake_toolchain"])

x64_action_test = _action_test("//msix/platforms:windows_x86_64")

arm64_action_test = _action_test("//msix/platforms:windows_arm64")

def _sign_env_impl(ctx):
    env = analysistest.begin(ctx)
    environment = analysistest.target_under_test(env)[RunEnvironmentInfo].environment
    asserts.equals(env, ["app.msix", "app.msix.identity.json"], sorted([p.split("/")[-1] for p in environment["DX_MSIX_PACKAGE"].split(" ")]))
    tools = [p.split("/")[-1] for p in environment["DX_MSIX_SIGNTOOL"].split(" ")]
    asserts.true(env, "signtool.exe" in tools, "signtool.exe missing from " + str(tools))
    return analysistest.end(env)

sign_env_test = analysistest.make(_sign_env_impl)

def _sign_build_impl(ctx):
    env = analysistest.begin(ctx)
    target = analysistest.target_under_test(env)
    for action in analysistest.target_actions(env):
        for arg in action.argv or []:
            asserts.false(env, "signtool" in arg or "fake_makeappx" in arg or arg.endswith("sign.ps1"), action.mnemonic + " runs " + arg)
        for output in action.outputs.to_list():
            asserts.false(env, output.basename.endswith(".msix"), action.mnemonic + " writes " + output.short_path)
    built = [f.basename for f in target[DefaultInfo].files.to_list()]
    asserts.false(env, [b for b in built if b.endswith(".msix")], "building signs " + str(built))
    runfiles = [f.basename for f in target[DefaultInfo].default_runfiles.files.to_list()]
    for name in ["cell.msix", "cell.msix.identity.json", "fake_makeappx.exe", "sign.ps1"]:
        asserts.true(env, name in runfiles, name + " is not a run input of the signer")
    environment = target[RunEnvironmentInfo].environment
    asserts.true(env, "DX_MSIX_PFX_PASSWORD" not in environment, "the password is fixed at build time")
    return analysistest.end(env)

sign_build_test = analysistest.make(
    _sign_build_impl,
    config_settings = {
        "//command_line_option:extra_toolchains": [str(Label("//msix/tests:fake_toolchain"))],
        "//command_line_option:platforms": str(Label("//msix/platforms:windows_x86_64")),
    },
)
