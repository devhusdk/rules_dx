"""Credential-free release archives for dx deploy."""

load("//rust/rules:defs.bzl", "rust_binary")
load(":defs.bzl", "dx_deployment")
load(":launcher.bzl", "rlocation_path")

def _archive_stage_impl(ctx):
    """Stages one executable as a single file preserving its basename."""
    exe = ctx.attr.app[DefaultInfo].files_to_run.executable
    if exe == None:
        fail("archive_deploy " + str(ctx.label) + ": app " +
             str(ctx.attr.app.label) + " has no executable")
    staged = ctx.actions.declare_file(ctx.label.name + "/" + exe.basename)
    ctx.actions.symlink(output = staged, target_file = exe)
    return [DefaultInfo(files = depset([staged]))]

_archive_stage = rule(
    implementation = _archive_stage_impl,
    attrs = {
        "app": attr.label(
            mandatory = True,
        ),
    },
)

def _archive_launcher_impl(ctx):
    """Expands the rust_binary launcher for one release."""
    app_files = ctx.attr.app[DefaultInfo].files.to_list()
    if len(app_files) != 1:
        fail("archive_deploy " + str(ctx.label) + ": stage must provide exactly one file")
    archive_files = ctx.attr.archive[DefaultInfo].files.to_list()
    if len(archive_files) != 1:
        fail("archive_deploy " + str(ctx.label) + ": archive must provide exactly one file")
    checksum_files = ctx.attr.checksum[DefaultInfo].files.to_list()
    if len(checksum_files) != 1:
        fail("archive_deploy " + str(ctx.label) + ": checksum must provide exactly one file")
    app_file = app_files[0]
    archive_file = archive_files[0]
    checksum_file = checksum_files[0]

    app_rloc = rlocation_path(ctx, app_file)
    archive_rloc = rlocation_path(ctx, archive_file)
    checksum_rloc = rlocation_path(ctx, checksum_file)

    launcher = ctx.actions.declare_file(ctx.label.name + ".rs")
    ctx.actions.write(
        output = launcher,
        content = "const APP_RLOC: &str = \"" + app_rloc + "\";\n" +
                  "const TARBALL_RLOC: &str = \"" + archive_rloc + "\";\n" +
                  "const CHECKSUM_RLOC: &str = \"" + checksum_rloc + "\";\n" +
                  "fn main() { std::process::exit(dx_deploy_tools::archive_main(APP_RLOC, TARBALL_RLOC, CHECKSUM_RLOC, &std::env::args().collect::<Vec<_>>())); }\n",
    )
    return [DefaultInfo(files = depset([launcher]))]

_archive_launcher = rule(
    implementation = _archive_launcher_impl,
    attrs = {
        "app": attr.label(mandatory = True),
        "archive": attr.label(mandatory = True),
        "checksum": attr.label(mandatory = True),
    },
)

def archive_filenames(name):
    """Returns the deterministic (tarball, checksum) output names."""
    return (name + ".tar.gz", name + ".tar.gz.sha256")

def archive_deploy(name, app, profile = "release"):
    """Packages one executable as a tarball + sha256 deployable target."""
    (tarball, checksum) = archive_filenames(name)
    archive_target = name + "_archive"
    checksum_target = name + "_checksum"
    program_target = name + "_program"
    stage_target = name + "_stage"

    _archive_stage(
        name = stage_target,
        app = app,
    )

    native.genrule(
        name = archive_target,
        srcs = [":" + stage_target],
        outs = [tarball],
        tools = ["//deploy/rules:archiver"],
        cmd = "$(location //deploy/rules:archiver) $(location :" + stage_target + ") $(OUTS)",
    )

    native.genrule(
        name = checksum_target,
        srcs = [":" + archive_target],
        outs = [checksum],
        tools = ["//deploy/rules:hasher"],
        cmd = "$(location //deploy/rules:hasher) $(location :" + archive_target + ") $(OUTS)",
    )

    launcher_target = program_target + "_launcher"
    _archive_launcher(
        name = launcher_target,
        app = ":" + stage_target,
        archive = ":" + archive_target,
        checksum = ":" + checksum_target,
    )

    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        data = [
            ":" + stage_target,
            ":" + archive_target,
            ":" + checksum_target,
        ],
        deps = ["//deploy/rules:dx_deploy_tools"],
    )

    dx_deployment(
        name = name,
        app = app,
        deploy = ":" + program_target,
        profile = profile,
    )
