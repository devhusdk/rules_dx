"""Deploy boundary for dx deploy."""

load("//libs/starlark:defs.bzl", "DxSubjectInfo", "display_label")
load("//libs/starlark:wrapper.bzl", "dx_symlink_executable")
load(":sbom.bzl", "DxSbomFileInfo")
load(":config.bzl", "DxDeployConfigInfo", "rlocation_path")
load(":validate.bzl", "size_budget_bytes", "size_budget_error")

DxDeployInfo = provider(
    doc = "Deploy entrypoint identity and default build mode for dx deploy dispatch.",
    fields = {
        "app": "Label or None: the deployed app target when distinct from the deploy target.",
        "artifacts": "List of Labels: declared app artifact targets staged for the deploy tool.",
        "sbom": "String: exec path of the SPDX document, empty without an sbom.",
        "size_budget": "String: the size_budget attribute, empty when unset.",
        "size_stamp": "String: exec path of the file holding the checked size, empty without a budget.",
        "build_mode": "String or None: default build mode ('debug', 'dev', 'release'); None means the command default applies.",
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
    budget_error = size_budget_error(ctx.attr.size_budget)
    if budget_error:
        fail("dx_deployment " + str(ctx.label) + ": " + budget_error)
    artifact_labels = []
    artifact_files = []
    for target in ctx.attr.artifacts:
        info = target[DefaultInfo]
        files = info.files.to_list()
        if len(files) == 0:
            fail("dx_deployment " + str(ctx.label) + ": artifact " +
                 str(target.label) + " provides no files")
        artifact_labels.append(display_label(target.label))
        artifact_files.extend(files)
        runfiles = runfiles.merge(ctx.runfiles(files = files))
        runfiles = runfiles.merge(info.default_runfiles)
    for target in ctx.attr.data:
        runfiles = runfiles.merge(ctx.runfiles(files = target[DefaultInfo].files.to_list()))
    extra = []
    environment = {}
    built = [link]
    if ctx.attr.config:
        config = ctx.attr.config[DxDeployConfigInfo]
        runfiles = runfiles.merge(ctx.attr.config[DefaultInfo].default_runfiles)
        runfiles = runfiles.merge(ctx.runfiles(files = [config.file]))
        extra.append(config)
        environment["DX_DEPLOY_CONFIG"] = rlocation_path(ctx, config.file)
    sbom_path = ""
    if ctx.attr.sbom:
        sbom_file = ctx.attr.sbom[DxSbomFileInfo].file
        runfiles = runfiles.merge(ctx.runfiles(files = [sbom_file]))
        environment["DX_DEPLOY_SBOM"] = rlocation_path(ctx, sbom_file)
        sbom_path = sbom_file.path
        built.append(sbom_file)
    app_label = None
    app_text = ""
    if ctx.attr.app:
        app_label = ctx.attr.app.label
        app_text = display_label(app_label)
    validation = []
    size_stamp = ""
    if ctx.attr.size_budget and artifact_files:
        stamp = ctx.actions.declare_file(ctx.label.name + ".size_budget")
        args = ctx.actions.args()
        args.add(size_budget_bytes(ctx.attr.size_budget))
        args.add(stamp)
        args.add_all(artifact_files)
        ctx.actions.run(
            executable = ctx.executable._size_checker,
            arguments = [args],
            inputs = artifact_files,
            outputs = [stamp],
            mnemonic = "DxSizeBudget",
            progress_message = "Checking size_budget of " + str(ctx.label),
        )
        validation.append(stamp)
        size_stamp = stamp.path
    if environment:
        extra.append(RunEnvironmentInfo(environment = environment))
    return [
        OutputGroupInfo(_validation = depset(validation)),
        DefaultInfo(
            executable = link,
            files = depset(built),
            runfiles = runfiles,
        ),
        DxDeployInfo(
            app = app_label,
            artifacts = [target.label for target in ctx.attr.artifacts],
            build_mode = ctx.attr.build_mode,
            sbom = sbom_path,
            size_budget = ctx.attr.size_budget,
            size_stamp = size_stamp,
        ),
        DxSubjectInfo(fields = {
            "app": app_text,
            "artifacts": ",".join(artifact_labels),
            "build_mode": ctx.attr.build_mode,
        }),
    ] + extra

dx_deployment = rule(
    implementation = _dx_deployment_impl,
    doc = "Deployment that dx deploy runs with a declared build mode.",
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
        "sbom": attr.label(
            cfg = "target",
            mandatory = False,
            providers = [DxSbomFileInfo],
            doc = "dx_sbom target whose SPDX document is staged with the deployment and counted by a dx deploy check.",
        ),
        "size_budget": attr.string(
            doc = "Largest total size of artifacts, such as 25MB (decimal B, KB, MB or GB). Building fails above it and prints both sizes in bytes. Empty means no budget.",
        ),
        "_size_checker": attr.label(
            cfg = "exec",
            default = Label("//deploy/rules:size_checker"),
            executable = True,
        ),
        "build_mode": attr.string(
            doc = "Default build mode, one of debug, dev, or release.",
            default = "release",
            values = VALID_DEPLOY_PROFILES,
        ),
    },
)
