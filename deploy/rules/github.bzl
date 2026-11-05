"""Draft-only GitHub Release publisher for dx deploy."""

load(":config.bzl", "DEPLOY_TOOL", "deploy_config", "rlocation_path")
load(":defs.bzl", "dx_deployment")
load(":validate.bzl", "NAME_CHARS", "only_chars")

_VALID_TAG_CHARS = NAME_CHARS

def github_notes_error(notes):
    """Validates the notes value: commits, none or a label."""
    if type(notes) != "string" or notes == "":
        return "github_deploy: invalid notes '" + str(notes) + "': want 'commits', 'none' or a file label"
    return ""

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
            "notes": "file" if ctx.file.notes_file else ctx.attr.notes,
            "notes_rloc": rlocation_path(ctx, ctx.file.notes_file) if ctx.file.notes_file else "",
        },
        ctx.runfiles(files = asset_files + ([ctx.file.notes_file] if ctx.file.notes_file else [])).merge(runfiles),
    )

github_config = rule(
    doc = "Writes the deploy config for one draft release.",
    implementation = _github_config_impl,
    attrs = {
        "artifacts": attr.label_list(mandatory = True, doc = "Files to attach as release assets."),
        "deploy_name": attr.string(mandatory = True, doc = "Deploy target name."),
        "tag": attr.string(mandatory = True, doc = "Release tag."),
        "notes": attr.string(default = "commits", doc = "'commits' or 'none'."),
        "notes_file": attr.label(allow_single_file = True, doc = "Markdown file used as the release body."),
    },
)

def github_deploy(name, artifacts, tag = "v0.0.0-dryrun", profile = "release", tags = [], sbom = None, notes = "commits"):
    """Publishes pinned files as a draft-only GitHub Release.

    `sbom` is a dx_sbom target uploaded as one more release asset.
    `tag = "current"` uses the tag that points at HEAD.
    `notes` is "commits" (the grouped commit list since the previous tag), "none"
    or a label of a markdown file.
    """
    tag_error = github_tag_error(tag)
    if tag_error != "":
        fail(tag_error + " (in " + native.package_name() + ":" + name + ")")
    notes_error = github_notes_error(notes)
    if notes_error != "":
        fail(notes_error + " (in " + native.package_name() + ":" + name + ")")
    notes_mode = notes if notes in ["commits", "none"] else "none"
    notes_file = None if notes in ["commits", "none"] else notes

    config_target = name + "_config"
    github_config(
        name = config_target,
        artifacts = artifacts + ([sbom] if sbom else []),
        deploy_name = name,
        notes = notes_mode,
        notes_file = notes_file,
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
