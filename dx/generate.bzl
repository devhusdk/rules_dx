"""The gazelle runners of `//dx:generate` and `//dx:generate_check`."""

load("@gazelle//:def.bzl", "gazelle")

GENERATE_EXCLUDES = [
    "-exclude=.dx",
    "-exclude=**/node_modules",
]

def _rlocation_path(ctx, file):
    if file.short_path.startswith("../"):
        return file.short_path[3:]
    return ctx.workspace_name + "/" + file.short_path

def _dx_gazelle_run_impl(ctx):
    runner = ctx.attr.runner[DefaultInfo]
    launcher = ctx.attr.launcher[DefaultInfo].files_to_run.executable
    out = ctx.actions.declare_file(ctx.label.name + (".exe" if launcher.extension == "exe" else ""))
    ctx.actions.symlink(output = out, target_file = launcher, is_executable = True)
    configs = [
        file
        for file in runner.default_runfiles.files.to_list()
        if file.basename == "WORKSPACE" and file.owner.repo_name.endswith("bazel_gazelle_go_repository_config")
    ]
    go_tool = ctx.toolchains["@rules_go//go:toolchain"].sdk.go
    args = ["update"]
    if ctx.attr.mode:
        args.extend(["-mode", ctx.attr.mode])
    args.extend(ctx.attr.gazelle_args)
    environment = {
        "DX_GAZELLE": _rlocation_path(ctx, ctx.executable.gazelle),
        "DX_GAZELLE_ARGS": json.encode(args),
        "DX_GAZELLE_GO": _rlocation_path(ctx, go_tool),
    }
    if configs:
        environment["DX_GAZELLE_REPO_CONFIG"] = _rlocation_path(ctx, configs[0])
    runfiles = ctx.runfiles(files = [out, ctx.executable.gazelle, go_tool] + configs)
    runfiles = runfiles.merge(ctx.attr.gazelle[DefaultInfo].default_runfiles)
    runfiles = runfiles.merge(ctx.attr.launcher[DefaultInfo].default_runfiles)
    return [
        DefaultInfo(executable = out, runfiles = runfiles),
        RunEnvironmentInfo(environment = environment),
    ]

_dx_gazelle_run = rule(
    implementation = _dx_gazelle_run_impl,
    attrs = {
        "gazelle": attr.label(mandatory = True, executable = True, cfg = "target", doc = "The gazelle binary."),
        "gazelle_args": attr.string_list(doc = "Arguments after the gazelle command."),
        "launcher": attr.label(default = Label("//dx:gazelle_launcher"), executable = True, cfg = "target", doc = "The launcher that starts gazelle."),
        "mode": attr.string(doc = "The gazelle -mode value."),
        "runner": attr.label(mandatory = True, doc = "The upstream gazelle runner that carries the repo config."),
    },
    executable = True,
    toolchains = ["@rules_go//go:toolchain"],
    doc = "Runs gazelle through a native launcher, so no shell tools are needed.",
)

def dx_generate(name, mode = None, visibility = None):
    """A gazelle run target over the dx languages."""
    gazelle(
        name = name + "_gazelle",
        gazelle = "@dx_gazelle//:gazelle",
        tags = ["manual"],
    )
    _dx_gazelle_run(
        name = name,
        gazelle = "@dx_gazelle//:gazelle",
        gazelle_args = GENERATE_EXCLUDES,
        mode = mode,
        runner = ":%s_gazelle-runner" % name,
        visibility = visibility,
    )
