"""Draft-only GitHub Release publisher for dx deploy."""

load("//rust/rules:defs.bzl", "rust_binary")
load(":defs.bzl", "dx_deployment")
load(":launcher.bzl", "rlocation_path")
load(":validate.bzl", "NAME_CHARS", "only_chars")

_VALID_TAG_CHARS = NAME_CHARS

def github_tag_error(tag):
    """Validates one release tag value."""
    if type(tag) != "string" or tag == "":
        return ("github_deploy: invalid tag '" + str(tag) +
                "': want a non-empty tag (for example 'v0.0.0-dryrun')")
    if not only_chars(tag, _VALID_TAG_CHARS):
        return ("github_deploy: invalid tag '" + tag +
                "': want only [A-Za-z0-9._-] so the tag embeds " +
                "safely in the deploy launcher")
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

def _github_launcher_impl(ctx):
    """Expands the rust_binary launcher for one draft release."""
    asset_rlocs = []
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
        asset_names.append((str(target.label), f.basename))

    collision_error = github_asset_collision_error(asset_names)
    if collision_error != "":
        fail("github_deploy " + str(ctx.label) + ": " + collision_error)

    launcher = ctx.actions.declare_file(ctx.label.name + ".rs")
    ctx.actions.write(
        output = launcher,
        content = "const ASSET_RLOCS: &str = \"" + ";".join(asset_rlocs) + "\";\n" +
                  "const DEPLOY_NAME: &str = \"" + ctx.attr.deploy_name + "\";\n" +
                  "const TAG: &str = \"" + ctx.attr.tag + "\";\n" +
                  "fn main() { std::process::exit(dx_deploy_tools::github_main(ASSET_RLOCS, DEPLOY_NAME, TAG, &std::env::args().collect::<Vec<_>>())); }\n",
    )
    return [DefaultInfo(files = depset([launcher]))]

github_launcher = rule(
    doc = "Expands the Rust launcher source for one draft release.",
    implementation = _github_launcher_impl,
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

    program_target = name + "_program"
    launcher_target = program_target + "_launcher"
    github_launcher(
        name = launcher_target,
        artifacts = artifacts,
        deploy_name = name,
        tag = tag,
        tags = tags,
    )

    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        data = artifacts,
        deps = [Label("//deploy/rules:dx_deploy_tools")],
        tags = tags,
    )

    dx_deployment(
        name = name,
        deploy = ":" + program_target,
        profile = profile,
        tags = tags,
    )
