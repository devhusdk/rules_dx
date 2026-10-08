"""Deploy boundary for dx deploy."""

load("//libs/starlark:defs.bzl", "DxConfigInfo", "DxSubjectInfo", "display_label")
load("//libs/starlark:wrapper.bzl", "dx_symlink_executable")

DxDeployInfo = provider(
    doc = "Deploy entrypoint identity and default profile for dx deploy dispatch.",
    fields = {
        "app": "Label or None: the deployed app target when distinct from the deploy target.",
        "artifacts": "List of labels: declared nonexecutable artifact targets consumed from the target configuration.",
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

def deploy_empty_artifact_error(label, file_count):
    """Validates one deploy artifact contributes files."""
    if file_count == 0:
        return ("dx_deployment: artifact " + label + " provides no files: " +
                "want declared output files or directories")
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
    artifact_files = []
    artifact_labels = []
    for artifact in ctx.attr.artifacts:
        files = artifact[DefaultInfo].files.to_list()
        artifact_error = deploy_empty_artifact_error(display_label(artifact.label), len(files))
        if artifact_error != "":
            fail(artifact_error + " (in " + str(ctx.label) + ")")
        artifact_files.extend(files)
        artifact_labels.append(artifact.label)
        runfiles = runfiles.merge(artifact[DefaultInfo].default_runfiles)
    app_label = None
    app_text = ""
    if ctx.attr.app:
        app_label = ctx.attr.app.label
        app_text = display_label(app_label)
    config_fields = {
        "artifact_config": "target",
        "launcher_config": "exec",
    }
    if DxConfigInfo in ctx.attr.deploy:
        launcher_side = ctx.attr.deploy[DxConfigInfo].fields.get("side", "")
        if launcher_side != "":
            config_fields["launcher_side"] = launcher_side
    for artifact in ctx.attr.artifacts:
        if DxConfigInfo in artifact:
            artifact_side = artifact[DxConfigInfo].fields.get("side", "")
            if artifact_side != "":
                config_fields["artifact_side"] = artifact_side
    return [
        DefaultInfo(
            executable = link,
            files = depset([link] + artifact_files),
            runfiles = runfiles,
        ),
        DxDeployInfo(app = app_label, artifacts = artifact_labels, profile = ctx.attr.profile),
        DxSubjectInfo(fields = {
            "app": app_text,
            "artifacts": ", ".join([display_label(label) for label in artifact_labels]),
            "profile": ctx.attr.profile,
        }),
        DxConfigInfo(fields = config_fields),
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
