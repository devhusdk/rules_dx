"""The gazelle runners of `//dx:generate` and `//dx:generate_check`."""

load("@gazelle//:def.bzl", "gazelle")
load("//cli/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS", "write_native_launcher")

visibility(["//dx/...", "//tools/..."])

GENERATE_EXCLUDES = [
    "-exclude=.dx",
    "-exclude=**/node_modules",
]

def _rlocation_path(ctx, file):
    if file.short_path.startswith("../"):
        return file.short_path[3:]
    return ctx.workspace_name + "/" + file.short_path

def gazelle_launch_descriptor(gazelle, repo_config, goroot, mode, gazelle_args):
    """Returns the launcher descriptor that runs gazelle.

    Args:
      gazelle: runfiles key of the gazelle binary.
      repo_config: runfiles key of the repo config, or None.
      goroot: runfiles key of a file at the root of the Go SDK.
      mode: the gazelle -mode value, or None.
      gazelle_args: arguments after the command when the user names none.
    """
    lines = ["program=" + gazelle, "verb=update", "verb=fix"]
    if repo_config:
        lines.extend(["arg=-bzlmod", "arg=-repo_config", "path=" + repo_config])
    defaults = ["-mode", mode] if mode else []
    lines.extend(["default_arg=" + arg for arg in defaults + gazelle_args])
    lines.extend(["env_dir=GOROOT\t" + goroot, "env=GOTOOLCHAIN\tlocal"])
    return "\n".join(lines) + "\n"

def _dx_gazelle_run_impl(ctx):
    runner = ctx.attr.runner[DefaultInfo]
    configs = [
        file
        for file in runner.default_runfiles.files.to_list()
        if file.basename == "WORKSPACE" and file.owner.repo_name.endswith("bazel_gazelle_go_repository_config")
    ]
    sdk = ctx.toolchains["@rules_go//go:toolchain"].sdk
    descriptor = gazelle_launch_descriptor(
        gazelle = _rlocation_path(ctx, ctx.executable.gazelle),
        repo_config = _rlocation_path(ctx, configs[0]) if configs else None,
        goroot = _rlocation_path(ctx, sdk.root_file),
        mode = ctx.attr.mode,
        gazelle_args = ctx.attr.gazelle_args,
    )
    launcher, _, launcher_runfiles = write_native_launcher(ctx, descriptor)
    runfiles = ctx.runfiles(files = [ctx.executable.gazelle, sdk.go, sdk.root_file] + configs)
    runfiles = runfiles.merge_all([launcher_runfiles, ctx.attr.gazelle[DefaultInfo].default_runfiles])
    return [DefaultInfo(executable = launcher, runfiles = runfiles)]

_dx_gazelle_run = rule(
    implementation = _dx_gazelle_run_impl,
    attrs = NATIVE_LAUNCHER_ATTRS | {
        "gazelle": attr.label(mandatory = True, executable = True, cfg = "target", doc = "The gazelle binary."),
        "gazelle_args": attr.string_list(doc = "Arguments after the gazelle command."),
        "mode": attr.string(doc = "The gazelle -mode value."),
        "runner": attr.label(mandatory = True, doc = "The upstream gazelle runner that carries the repo config."),
    },
    executable = True,
    toolchains = ["@rules_go//go:toolchain"],
    doc = "Runs gazelle through the native launcher, so no shell tools are needed.",
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
