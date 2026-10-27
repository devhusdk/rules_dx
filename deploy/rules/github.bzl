"""Draft-only GitHub Release publisher for dx deploy."""

load(":config.bzl", "DEPLOY_TOOL", "deploy_config", "rlocation_path")
load(":defs.bzl", "dx_deployment")
load(":validate.bzl", "NAME_CHARS", "only_chars")

_VALID_TAG_CHARS = NAME_CHARS

def github_tag_error(tag):
    """Validates one release tag value."""
    if type(tag) != "string" or tag == "":
        return ("github_deploy: invalid tag '" + str(tag) +
                "': want a non-empty tag (for example 'v0.0.0-dryrun')")
    if not only_chars(tag, _VALID_TAG_CHARS):
        return ("github_deploy: invalid tag '" + tag +
                "': want only [A-Za-z0-9._-]")
    return ""

def github_asset_collision_error(assets):
    """Reports the first pair of artifacts that share one asset name."""
    seen = {}
    for label, base in assets:
        if base in seen:
            return ("github_deploy: artifacts " + seen[base] + " and " + label +
                    " share the asset name '" + base + "'")
        seen[base] = label
    return ""

def _github_config_impl(ctx):
    """Writes the deploy config for one draft release."""
    asset_rlocs = []
    asset_files = []
    runfiles = ctx.runfiles()
    asset_names = []
    for target in ctx.attr.artifacts:
        info = target[DefaultInfo]
        f = info.files_to_run.executable
        if f == None:
            files = info.files.to_list()
            if len(files) != 1:
                fail("github_deploy " + str(ctx.label) + ": artifact " +
                     str(target.label) + " provides " +
                     str(len(files)) + " files, want exactly one " +
                     "(executables resolve to their binary)")
            f = files[0]
        asset_rlocs.append(rlocation_path(ctx, f))
        asset_files.append(f)
        runfiles = runfiles.merge(info.default_runfiles)
        asset_names.append((str(target.label), f.basename))

    collision_error = github_asset_collision_error(asset_names)
    if collision_error != "":
        fail("github_deploy " + str(ctx.label) + ": " + collision_error)

    return deploy_config(
        ctx,
        "github",
        {
            "asset_rlocs": asset_rlocs,
            "deploy_name": ctx.attr.deploy_name,
            "tag": ctx.attr.tag,
        },
        ctx.runfiles(files = asset_files).merge(runfiles),
    )

github_config = rule(
    doc = "Writes the deploy config for one draft release.",
    implementation = _github_config_impl,
    attrs = {
        "artifacts": attr.label_list(mandatory = True, doc = "Files to attach as release assets."),
        "deploy_name": attr.string(mandatory = True, doc = "Deploy target name."),
        "tag": attr.string(mandatory = True, doc = "Release tag."),
    },
)

def github_deploy(name, artifacts, tag = "v0.0.0-dryrun", profile = "release", tags = []):
    """Publishes pinned files as a draft-only GitHub Release."""
    tag_error = github_tag_error(tag)
    if tag_error != "":
        fail(tag_error + " (in " + native.package_name() + ":" + name + ")")
    if len(artifacts) == 0:
        fail("github_deploy " + native.package_name() + ":" + name +
             ": need at least one artifact")

    config_target = name + "_config"
    github_config(
        name = config_target,
        artifacts = artifacts,
        deploy_name = name,
        tag = tag,
        tags = tags,
    )

    dx_deployment(
        name = name,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        profile = profile,
        tags = tags,
    )
