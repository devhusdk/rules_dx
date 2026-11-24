"""Shared pieces for rules that run the native launcher."""

load("//core:no_coverage.bzl", "no_coverage_transition")
load("//core:rlocation.bzl", "rlocation_path")

NATIVE_LAUNCHER_ATTRS = {
    "_exec_launcher": attr.label(default = Label("@dx_prebuilt//:launcher"), executable = True, cfg = no_coverage_transition, doc = "The native launcher."),
    "_windows_os": attr.label(default = Label("@platforms//os:windows")),
}

def target_is_windows(ctx):
    """Returns whether the rule builds for a Windows target platform."""
    return ctx.target_platform_has_constraint(ctx.attr._windows_os[platform_common.ConstraintValueInfo])

def native_launch_descriptor_path(exe_path):
    """Returns the launcher descriptor path for `exe_path`."""
    if exe_path.endswith(".exe"):
        return exe_path[:-len(".exe")] + ".launch"
    return exe_path + ".launch"

def write_native_launcher(ctx, descriptor_text, launcher_name = None):
    """Writes the native launcher and its descriptor."""
    launcher = ctx.actions.declare_file((launcher_name or ctx.label.name) + (".exe" if target_is_windows(ctx) else ""))
    descriptor = ctx.actions.declare_file(ctx.label.name + ".launch")
    ctx.actions.write(descriptor, descriptor_text)
    ctx.actions.symlink(output = launcher, target_file = ctx.executable._exec_launcher, is_executable = True)
    runfiles = ctx.runfiles(files = [launcher, descriptor]).merge(ctx.attr._exec_launcher[0][DefaultInfo].default_runfiles)
    return launcher, descriptor, runfiles

def subcommand_descriptor(program_key, subcommand, path_args = [], args = []):
    """Returns the launch descriptor text that runs `program_key subcommand [ARG]... [FLAG=PATH]...`."""
    lines = ["program=" + program_key, "arg=" + subcommand]
    lines.extend(["arg=" + arg for arg in args])
    lines.extend(["prefixed=" + flag + "=\t" + key for flag, key in path_args])
    return "\n".join(lines) + "\n"

def _dx_subcommand_impl(ctx):
    dx = ctx.executable._dx
    path_args = []
    for flag in sorted(ctx.attr.path_args.keys()):
        files = ctx.attr.path_args[flag].files.to_list()
        if len(files) != 1:
            fail("path_args[{}] must be exactly one file, got {}".format(flag, len(files)))
        path_args.append((flag, rlocation_path(ctx, files[0])))
    launcher, _, runfiles = write_native_launcher(ctx, subcommand_descriptor(rlocation_path(ctx, dx), ctx.attr.subcommand, path_args, ctx.attr.subcommand_args))
    runfiles = runfiles.merge(ctx.runfiles(files = [dx])).merge(ctx.attr._dx[DefaultInfo].default_runfiles)
    for target in ctx.attr.data + ctx.attr.path_args.values():
        runfiles = runfiles.merge(ctx.runfiles(transitive_files = target.files)).merge(target[DefaultInfo].default_runfiles)
    return [DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = runfiles)]

dx_subcommand = rule(
    implementation = _dx_subcommand_impl,
    doc = "Runs `dx <subcommand> [ARG]...` from the prebuilt dx through the native launcher.",
    executable = True,
    attrs = dict({
        "data": attr.label_list(allow_files = True, doc = "Files the subcommand reads at run time."),
        "path_args": attr.string_keyed_label_dict(allow_files = True, cfg = "target", doc = "Flag to a one-file target; dx gets `FLAG=<absolute path>` after the subcommand."),
        "subcommand": attr.string(mandatory = True, doc = "The dx multicall subcommand."),
        "subcommand_args": attr.string_list(doc = "Literal arguments after the subcommand, before the `path_args` flags."),
        "_dx": attr.label(default = Label("@dx_prebuilt//:dx"), executable = True, cfg = "target", doc = "The prebuilt dx."),
    }, **NATIVE_LAUNCHER_ATTRS),
)
