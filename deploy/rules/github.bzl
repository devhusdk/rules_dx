"""Draft-only GitHub Release publisher for dx deploy."""

load("//rust/rules:defs.bzl", "rust_binary")
load(":defs.bzl", "dx_deployment")
load(":launcher.bzl", "rlocation_path")

TAG_SCHEMA_VERSION = 1

_VALID_TAG_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-"

def tag_charset():
    """Returns the launcher-safe tag charset via registry query."""
    return _VALID_TAG_CHARS

def tag_schema_error():
    """Validates the versioned tag-charset schema."""
    if TAG_SCHEMA_VERSION != 1:
        return "github tag: unsupported schema v" + str(TAG_SCHEMA_VERSION) + " (want v1)"
    if type(_VALID_TAG_CHARS) != "string" or _VALID_TAG_CHARS == "":
        return "github tag: want a non-empty charset (schema v1)"
    seen = {}
    for c in _VALID_TAG_CHARS.elems():
        if c in seen:
            return "github tag: duplicate charset char '" + c + "'"
        seen[c] = True
        if c in ["\"", "\\", "'", " ", "\n", "`", "$", "!", "#", "&", "|", ";", "<", ">", "(", ")", "[", "]", "{", "}", "*", "?", "~", "^", ":", ",", "/", "+"]:
            return "github tag: unsafe charset char '" + c + "' (must stay launcher-safe)"
    return ""

def github_tag_error(tag):
    """Validates one release tag value."""
    if type(tag) != "string" or tag == "":
        return ("github_deploy: invalid tag '" + str(tag) +
                "': want a non-empty tag (for example 'v0.0.0-dryrun')")
    for c in tag.elems():
        if c not in _VALID_TAG_CHARS:
            return ("github_deploy: invalid tag '" + tag +
                    "': want only [A-Za-z0-9._-] so the tag embeds " +
                    "safely in the deploy launcher")
    return ""

def github_draft_error(draft):
    """Validates the draft gate."""
    if draft != True:
        return ("github_deploy: draft=False requires explicit owner " +
                "approval; keep the draft gate and publish " +
                "the release on GitHub after approval")
    return ""

def _github_launcher_impl(ctx):
    """Expands the rust_binary launcher for one draft release."""
    asset_rlocs = []
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

    launcher = ctx.actions.declare_file(ctx.label.name + ".rs")
    ctx.actions.write(
        output = launcher,
        content = "const ASSET_RLOCS: &str = \"" + ";".join(asset_rlocs) + "\";\n" +
                  "const DEPLOY_NAME: &str = \"" + ctx.attr.deploy_name + "\";\n" +
                  "const TAG: &str = \"" + ctx.attr.tag + "\";\n" +
                  "fn main() { std::process::exit(dx_deploy_tools::github_main(ASSET_RLOCS, DEPLOY_NAME, TAG, &std::env::args().collect::<Vec<_>>())); }\n",
    )
    return [DefaultInfo(files = depset([launcher]))]

_github_launcher = rule(
    implementation = _github_launcher_impl,
    attrs = {
        "artifacts": attr.label_list(
            mandatory = True,
        ),
        "deploy_name": attr.string(
            mandatory = True,
        ),
        "tag": attr.string(
            mandatory = True,
        ),
    },
)

def github_deploy(name, artifacts, tag = "v0.0.0-dryrun", draft = True, profile = "release"):
    """Publishes pinned files as a draft-only GitHub Release."""
    tag_error = github_tag_error(tag)
    if tag_error != "":
        fail(tag_error + " (in " + native.package_name() + ":" + name + ")")
    draft_error = github_draft_error(draft)
    if draft_error != "":
        fail(draft_error + " (in " + native.package_name() + ":" + name + ")")
    if len(artifacts) == 0:
        fail("github_deploy " + native.package_name() + ":" + name +
             ": need at least one artifact")

    program_target = name + "_program"
    launcher_target = program_target + "_launcher"
    _github_launcher(
        name = launcher_target,
        artifacts = artifacts,
        deploy_name = name,
        tag = tag,
    )

    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        data = artifacts,
        deps = ["//deploy/rules:dx_deploy_tools"],
    )

    dx_deployment(
        name = name,
        deploy = ":" + program_target,
        profile = profile,
    )

def github_release(name, artifacts, tag = "v0.0.0-dryrun", draft = True, profile = "release"):
    """Compat alias for github_deploy."""
    github_deploy(
        name = name,
        artifacts = artifacts,
        tag = tag,
        draft = draft,
        profile = profile,
    )
