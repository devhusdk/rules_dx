"""Deploy boundary for dx deploy."""

load("//libs/starlark:defs.bzl", "DxSubjectInfo", "display_label")
load("//libs/starlark:wrapper.bzl", "dx_symlink_executable")
load(":config.bzl", "DxDeployConfigInfo", "rlocation_path")

DxDeployInfo = provider(
    doc = "Deploy entrypoint identity and default profile for dx deploy dispatch.",
    fields = {
        "app": "Label or None: the deployed app target when distinct from the deploy target.",
        "artifacts": "List of Labels: declared app artifact targets staged for the deploy tool.",
        "profile": "String or None: default profile ('debug', 'dev', 'release'); None means the command default applies.",
    },
)

VALID_DEPLOY_PROFILES = ["debug", "dev", "release"]

def _dx_deployment_impl(ctx):
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
    for target in ctx.attr.data:
        runfiles = runfiles.merge(ctx.runfiles(files = target[DefaultInfo].files.to_list()))
    extra = []
    if ctx.attr.config:
        config = ctx.attr.config[DxDeployConfigInfo]
        runfiles = runfiles.merge(ctx.attr.config[DefaultInfo].default_runfiles)
        runfiles = runfiles.merge(ctx.runfiles(files = [config.file]))
        extra.append(RunEnvironmentInfo(
            environment = {"DX_DEPLOY_CONFIG": rlocation_path(ctx, config.file)},
        ))
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
    ] + extra

dx_deployment = rule(
    implementation = _dx_deployment_impl,
    doc = "Deployment that dx deploy runs with a declared profile.",
    executable = True,
    attrs = {
        "app": attr.label(
            doc = "App target described by the deployment when distinct from the deploy target.",
            cfg = "target",
            executable = True,
            mandatory = False,
            providers = [DefaultInfo],
        ),
        "artifacts": attr.label_list(
            doc = "Artifact targets staged into the deployment runfiles.",
            allow_files = True,
            cfg = "target",
            mandatory = False,
        ),
        "config": attr.label(
            cfg = "target",
            mandatory = False,
            providers = [DxDeployConfigInfo],
            doc = "Deploy config the shared deploy_tool reads through DX_DEPLOY_CONFIG.",
        ),
        "data": attr.label_list(
            allow_files = True,
            cfg = "target",
            doc = "Extra target-config files built and shipped with the deployment.",
        ),
        "deploy": attr.label(
            doc = "Executable target that performs the deployment when run.",
            cfg = "exec",
            executable = True,
            mandatory = True,
        ),
        "profile": attr.string(
            doc = "Default deploy profile, one of debug, dev, or release.",
            default = "release",
            values = VALID_DEPLOY_PROFILES,
        ),
    },
)
