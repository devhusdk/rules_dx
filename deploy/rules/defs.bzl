"""Deploy boundary for dx deploy."""

load("//libs/starlark:defs.bzl", "DxSubjectInfo", "display_label")
load("//libs/starlark:wrapper.bzl", "dx_symlink_executable")

DxDeployInfo = provider(
    doc = "Deploy entrypoint identity and default profile for dx deploy dispatch.",
    fields = {
        "app": "Label or None: the deployed app target when distinct from the deploy target.",
        "artifacts": "List of Labels: declared app artifact targets staged for the deploy tool.",
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
    run = deploy_default.files_to_run
    exe = run.executable if run != None else None
    if exe == None:
        fail("dx_deployment " + str(ctx.label) + ": deploy target " +
             str(ctx.attr.deploy.label) + " has no executable")
    link = dx_symlink_executable(ctx, exe)
    runfiles = ctx.runfiles(files = [link]).merge(deploy_default.default_runfiles)
    artifact_labels = []
    for target in ctx.attr.artifacts:
        info = target[DefaultInfo]
        files = info.files.to_list()
        if len(files) == 0:
            fail("dx_deployment " + str(ctx.label) + ": artifact " +
                 str(target.label) + " provides no files")
        artifact_labels.append(display_label(target.label))
        runfiles = runfiles.merge(ctx.runfiles(files = files))
        runfiles = runfiles.merge(info.default_runfiles)
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
        DxDeployInfo(
            app = app_label,
            artifacts = [target.label for target in ctx.attr.artifacts],
            profile = ctx.attr.profile,
        ),
        DxSubjectInfo(fields = {
            "app": app_text,
            "artifacts": ",".join(artifact_labels),
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
        "artifacts": attr.label_list(
            allow_files = True,
            cfg = "target",
            mandatory = False,
        ),
        "deploy": attr.label(
            cfg = "exec",
            executable = True,
            mandatory = True,
        ),
        "profile": attr.string(
            default = "release",
            values = VALID_DEPLOY_PROFILES,
        ),
    },
)
