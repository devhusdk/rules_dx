"""Draft-only GitHub Release publisher for dx deploy."""

load(":api_compat.bzl", "API_COMPAT_ATTRS", "api_compat_error")
load(":config.bzl", "DEPLOY_TOOL", "deploy_config", "rlocation_path")
load(":defs.bzl", "dx_deployment")
load(":validate.bzl", "NAME_CHARS", "only_chars")

_VALID_TAG_CHARS = NAME_CHARS

def github_notes_error(notes, notes_file = None):
    """Validates the notes value: commits or none, with an optional notes_file."""
    if notes not in ["commits", "none"]:
        return "github_deploy: invalid notes '" + str(notes) + "': want 'commits' or 'none'; use notes_file for a file"
    if notes_file and notes != "none":
        return "github_deploy: notes_file needs notes = 'none'"
    return ""

def github_tag_error(tag):
    """Validates one release tag value."""
    if type(tag) != "string" or tag == "":
        return ("github_deploy: invalid release_tag '" + str(tag) +
                "': want a non-empty tag (for example 'v0.0.0-dryrun')")
    if tag == "current":
        return "github_deploy: invalid release_tag 'current': use tag_from_head = True"
    if not only_chars(tag, _VALID_TAG_CHARS):
        return ("github_deploy: invalid release_tag '" + tag +
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

    apidiff_rloc = ""
    go_rloc = ""
    apidiff_runfiles = ctx.runfiles()
    if ctx.attr.api_compat != "off" and ctx.attr.apidiff and ctx.attr.go:
        apidiff_rloc = rlocation_path(ctx, ctx.executable.apidiff)
        go_rloc = rlocation_path(ctx, ctx.file.go)
        apidiff_runfiles = ctx.runfiles(files = [ctx.executable.apidiff]).merge(ctx.attr.apidiff[DefaultInfo].default_runfiles).merge(ctx.attr.go[DefaultInfo].default_runfiles)

    return deploy_config(
        ctx,
        "github",
        {
            "api_baseline": ctx.attr.api_baseline,
            "api_compat": ctx.attr.api_compat,
            "apidiff_rloc": apidiff_rloc,
            "go_rloc": go_rloc,
            "go_modules": ctx.attr.go_modules,
            "asset_rlocs": asset_rlocs,
            "deploy_name": ctx.attr.deploy_name,
            "tag": "current" if ctx.attr.tag_from_head else ctx.attr.release_tag,
            "notes": "file" if ctx.file.notes_file else ctx.attr.notes,
            "notes_rloc": rlocation_path(ctx, ctx.file.notes_file) if ctx.file.notes_file else "",
        },
        ctx.runfiles(files = asset_files + ([ctx.file.notes_file] if ctx.file.notes_file else [])).merge(runfiles).merge(apidiff_runfiles),
    )

github_config = rule(
    doc = "Writes the deploy config for one draft release.",
    implementation = _github_config_impl,
    attrs = API_COMPAT_ATTRS | {
        "apidiff": attr.label(
            doc = "The apidiff binary; set when api_compat is on.",
            executable = True,
            cfg = "exec",
        ),
        "go": attr.label(
            doc = "The go binary apidiff runs; set when api_compat is on.",
            allow_single_file = True,
            cfg = "exec",
        ),
        "go_modules": attr.string_list(doc = "Workspace-relative directories of the Go modules to check."),
        "artifacts": attr.label_list(mandatory = True, doc = "Files to attach as release assets."),
        "deploy_name": attr.string(mandatory = True, doc = "Deploy target name."),
        "release_tag": attr.string(doc = "Release tag. Empty when tag_from_head is set."),
        "tag_from_head": attr.bool(doc = "Use the tag that points at HEAD."),
        "notes": attr.string(default = "commits", doc = "'commits' or 'none'."),
        "notes_file": attr.label(allow_single_file = True, doc = "Markdown file used as the release body."),
    },
)

def github_deploy(name, artifacts, release_tag = "", tag_from_head = False, build_mode = "release", tags = [], sbom = None, notes = "commits", notes_file = None, api_compat = "fail", api_baseline = "registry", go_modules = [], visibility = None, testonly = None):
    """Publishes pinned files as a draft-only GitHub Release.

    `sbom` is a dx_sbom target uploaded as one more release asset.
    `release_tag` defaults to "v0.0.0-dryrun"; `tag_from_head = True` uses the
    tag that points at HEAD instead.
    `notes` is "commits" (the grouped commit list since the previous tag) or
    "none"; `notes_file` is a markdown file used as the release body instead.
    `api_compat` and `api_baseline` work as described under "Public API
    compatibility" in the README; the tool is apidiff on each of `go_modules`
    (workspace-relative directories; empty means the go.mod at the root).
    """
    if tag_from_head:
        if release_tag != "":
            fail("github_deploy: release_tag and tag_from_head are exclusive (in " + native.package_name() + ":" + name + ")")
    else:
        release_tag = release_tag or "v0.0.0-dryrun"
        tag_error = github_tag_error(release_tag)
        if tag_error != "":
            fail(tag_error + " (in " + native.package_name() + ":" + name + ")")
    notes_error = github_notes_error(notes, notes_file)
    if notes_error != "":
        fail(notes_error + " (in " + native.package_name() + ":" + name + ")")
    if notes_file:
        notes = "none"

    api_error = api_compat_error("github_deploy", api_compat, api_baseline)
    if api_error != "":
        fail(api_error + " (in " + native.package_name() + ":" + name + ")")
    checked = api_compat != "off"
    config_target = name + "_config"
    github_config(
        name = config_target,
        api_baseline = api_baseline,
        api_compat = api_compat,
        apidiff = Label("//deploy/rules/pins:apidiff") if checked else None,
        go = Label("//deploy/rules/pins:go") if checked else None,
        artifacts = artifacts + ([sbom] if sbom else []),
        go_modules = go_modules,
        deploy_name = name,
        notes = notes,
        notes_file = notes_file,
        release_tag = release_tag,
        tag_from_head = tag_from_head,
        tags = tags,
        testonly = testonly,
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        tags = tags,
        testonly = testonly,
        visibility = visibility,
    )
