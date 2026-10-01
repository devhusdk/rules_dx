"""Deploy boundary for dx deploy."""

load("//libs/starlark:defs.bzl", "DxSubjectInfo", "display_label")
load("//libs/starlark:wrapper.bzl", "dx_symlink_executable")

DxDeployInfo = provider(
    doc = "Deploy entrypoint identity and default profile for dx deploy dispatch.",
    fields = {
        "app": "Label or None: the deployed app target when distinct from the deploy target.",
        "profile": "String or None: default profile ('debug', 'dev', 'release'); None means the command default applies.",
    },
)

VALID_DEPLOY_PROFILES = ["debug", "dev", "release"]

def deploy_profile_error(profile):
    """Validates one deploy profile value."""
    if profile == None:
        return ""
    if type(profile) != "string" or profile not in VALID_DEPLOY_PROFILES:
        return ("dx_deployment: invalid profile '" + str(profile) +
                "': want one of debug, dev, release")
    return ""

def _dx_deployment_impl(ctx):
    error = deploy_profile_error(ctx.attr.profile)
    if error != "":
        fail(error + " (in " + str(ctx.label) + ")")
    deploy_default = ctx.attr.deploy[DefaultInfo]
    exe = deploy_default.files_to_run.executable
    if exe == None:
        fail("dx_deployment " + str(ctx.label) + ": deploy target " +
             str(ctx.attr.deploy.label) + " has no executable")
    link = dx_symlink_executable(ctx, exe)
    runfiles = ctx.runfiles(files = [link]).merge(deploy_default.default_runfiles)
    app_label = None
    app_text = ""
    if ctx.attr.app:
        app_label = ctx.attr.app.label
        app_text = display_label(app_label)
    return [
        DefaultInfo(
            executable = link,
            files = depset([link]),
            runfiles = runfiles,
        ),
        DxDeployInfo(app = app_label, profile = ctx.attr.profile),
        DxSubjectInfo(fields = {
            "app": app_text,
            "profile": ctx.attr.profile,
        }),
    ]

dx_deployment = rule(
    implementation = _dx_deployment_impl,
    executable = True,
    attrs = {
        "app": attr.label(
            cfg = "target",
            executable = True,
            mandatory = False,
            providers = [DefaultInfo],
        ),
        "deploy": attr.label(
            cfg = "target",
            executable = True,
            mandatory = True,
        ),
        "profile": attr.string(
            default = "release",
            values = VALID_DEPLOY_PROFILES,
        ),
    },
)
