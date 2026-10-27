"""Local-first OCI publisher for dx deploy."""

load("//rust/rules:defs.bzl", "rust_binary")
load(":defs.bzl", "dx_deployment")
load(":launcher.bzl", "rlocation_path")
load(":validate.bzl", "EMBED_UNSAFE", "LOWER_ALNUM", "NAME_CHARS", "has_any", "only_chars")

_VALID_TAG_CHARS = NAME_CHARS

_OCI_TAG_MAX = 128

_VALID_REGISTRY_CHARS = NAME_CHARS + "/:"

_VALID_REPOSITORY_CHARS = LOWER_ALNUM + "._-/"

OCI_DEFAULT_REGISTRY = "ghcr.io"

def oci_tag_error(tag):
    """Validates one image tag value."""
    if type(tag) != "string" or tag == "":
        return ("oci_deploy: invalid tag '" + str(tag) +
                "': want a non-empty tag (for example 'latest')")
    if not only_chars(tag, _VALID_TAG_CHARS):
        return ("oci_deploy: invalid tag '" + tag +
                "': want only [A-Za-z0-9._-] so the tag embeds " +
                "safely in the deploy launcher")
    if tag[0] in ".-":
        return ("oci_deploy: invalid tag '" + tag +
                "': want a tag starting with [A-Za-z0-9_]")
    if len(tag) > _OCI_TAG_MAX:
        return ("oci_deploy: invalid tag '" + tag +
                "': want at most " + str(_OCI_TAG_MAX) + " characters")
    return ""

def oci_registry_error(registry):
    """Validates one registry value."""
    if type(registry) != "string" or registry == "":
        return ("oci_deploy: invalid registry '" + str(registry) +
                "': want a non-empty registry (for example '" +
                OCI_DEFAULT_REGISTRY + "')")
    if not only_chars(registry, _VALID_REGISTRY_CHARS):
        return ("oci_deploy: invalid registry '" + registry +
                "': want only [A-Za-z0-9._-/:] so the registry embeds " +
                "safely in the deploy launcher")
    if registry.startswith("/") or registry.endswith("/"):
        return ("oci_deploy: invalid registry '" + registry +
                "': want no leading or trailing slash")
    if "://" in registry:
        return ("oci_deploy: invalid registry '" + registry +
                "': want a host plus optional path, never a URL scheme")
    if has_any(registry, EMBED_UNSAFE):
        return ("oci_deploy: invalid registry '" + registry +
                "': want no quotes, backslashes, spaces, or newlines " +
                "so the registry embeds safely in the deploy launcher")
    return ""

def _oci_repository_segment_error(segment):
    """Returns "" or a short reason a path segment breaks the OCI grammar."""
    if segment[0] not in LOWER_ALNUM or segment[-1] not in LOWER_ALNUM:
        return "want each path segment to start and end with [a-z0-9]"
    run = ""
    for c in segment.elems():
        if c in LOWER_ALNUM:
            if run not in ["", ".", "_", "__"] and run.strip("-") != "":
                return "want separators to be '.', '_', '__' or dashes only"
            run = ""
        else:
            run += c
    return ""

def oci_repository_error(repository, from_name = False):
    """Validates one repository value."""
    error = _oci_repository_error(repository)
    if error != "" and from_name:
        error += ("; repository defaults to the target name, " +
                  "set repository = '...' explicitly")
    return error

def _oci_repository_error(repository):
    if type(repository) != "string" or repository == "":
        return ("oci_deploy: invalid repository '" + str(repository) +
                "': want a non-empty repository (for example 'oci_demo')")
    if repository != repository.lower():
        return ("oci_deploy: invalid repository '" + repository +
                "': want lowercase (for example '" + repository.lower() + "')")
    if not only_chars(repository, _VALID_REPOSITORY_CHARS):
        return ("oci_deploy: invalid repository '" + repository +
                "': want only [a-z0-9._-/] so the repository embeds " +
                "safely in the deploy launcher")
    if repository.startswith("/") or repository.endswith("/"):
        return ("oci_deploy: invalid repository '" + repository +
                "': want no leading or trailing slash")
    if "//" in repository:
        return ("oci_deploy: invalid repository '" + repository +
                "': want no empty path segments")
    for segment in repository.split("/"):
        reason = _oci_repository_segment_error(segment)
        if reason != "":
            return ("oci_deploy: invalid repository '" + repository +
                    "': " + reason)
    return ""

OCI_PLATFORMS = ["linux/amd64", "linux/arm64"]

def oci_platform_error(platform):
    """Validates one image platform value."""
    if platform not in OCI_PLATFORMS:
        return ("oci_deploy: invalid platform '" + str(platform) +
                "': want one of " + ", ".join(OCI_PLATFORMS))
    return ""

def oci_tar_error(filename):
    """Validates one image-tar filename value."""
    if type(filename) != "string" or filename == "":
        return ("oci_deploy: invalid image_tar '" + str(filename) +
                "': want a non-empty .tar filename")
    if not (filename.endswith(".tar") or filename.endswith(".tar.gz")):
        return ("oci_deploy: invalid image_tar '" + filename +
                "': want a filename ending in .tar or .tar.gz")
    if has_any(filename, EMBED_UNSAFE):
        return ("oci_deploy: invalid image_tar '" + filename +
                "': want no quotes, backslashes, spaces, or newlines " +
                "so the filename embeds safely in the deploy launcher")
    return ""

def _oci_launcher_impl(ctx):
    """Expands the rust_binary launcher for one OCI deployment."""
    tar_files = ctx.attr.image_tar[DefaultInfo].files.to_list()
    if len(tar_files) != 1:
        fail("oci_deploy " + str(ctx.label) + ": image_tar " +
             str(ctx.attr.image_tar.label) + " provides " +
             str(len(tar_files)) + " files, want exactly one .tar")
    tar_file = tar_files[0]
    tar_error = oci_tar_error(tar_file.basename)
    if tar_error != "":
        fail(tar_error + " (in " + str(ctx.label) + ")")
    tar_rloc = rlocation_path(ctx, tar_file)

    launcher = ctx.actions.declare_file(ctx.label.name + ".rs")
    ctx.actions.write(
        output = launcher,
        content = "const IMAGE_RLOC: &str = \"" + tar_rloc + "\";\n" +
                  "const OCI_REGISTRY: &str = \"" + ctx.attr.registry + "\";\n" +
                  "const OCI_REPOSITORY: &str = \"" + ctx.attr.repository + "\";\n" +
                  "const OCI_TAG: &str = \"" + ctx.attr.tag + "\";\n" +
                  "const OCI_PLATFORM: &str = \"" + ctx.attr.platform + "\";\n" +
                  "fn main() { std::process::exit(dx_deploy_tools::oci_main(IMAGE_RLOC, OCI_REGISTRY, OCI_REPOSITORY, OCI_TAG, OCI_PLATFORM, &std::env::args().collect::<Vec<_>>())); }\n",
    )
    return [DefaultInfo(files = depset([launcher]))]

_oci_launcher = rule(
    implementation = _oci_launcher_impl,
    doc = "Writes the launcher Rust source for one OCI deployment.",
    attrs = {
        "image_tar": attr.label(
            doc = "docker save or OCI layout archive (.tar or .tar.gz) holding one image with uncompressed or gzip layers.",
            allow_single_file = True,
            mandatory = True,
        ),
        "registry": attr.string(
            doc = "Container registry host the image is pushed to.",
            mandatory = True,
        ),
        "repository": attr.string(
            doc = "Image repository path within the registry.",
            mandatory = True,
        ),
        "tag": attr.string(
            doc = "Image tag the deployment publishes.",
            mandatory = True,
        ),
        "platform": attr.string(
            doc = "Image platform, one of the supported OCI platforms.",
            mandatory = True,
        ),
    },
)

def oci_deploy(name, image_tar, platform, tag = "latest", registry = OCI_DEFAULT_REGISTRY, repository = "", profile = "release"):
    """Publishes one docker save or OCI layout archive (.tar or .tar.gz) holding one image as a local-first OCI deployment."""
    effective_repo = repository if repository != "" else name
    platform_error = oci_platform_error(platform)
    if platform_error != "":
        fail(platform_error + " (in " + native.package_name() + ":" + name + ")")
    tag_error = oci_tag_error(tag)
    if tag_error != "":
        fail(tag_error + " (in " + native.package_name() + ":" + name + ")")
    registry_error = oci_registry_error(registry)
    if registry_error != "":
        fail(registry_error + " (in " + native.package_name() + ":" + name + ")")
    repo_error = oci_repository_error(effective_repo, from_name = repository == "")
    if repo_error != "":
        fail(repo_error + " (in " + native.package_name() + ":" + name + ")")

    program_target = name + "_program"
    launcher_target = program_target + "_launcher"
    _oci_launcher(
        name = launcher_target,
        image_tar = image_tar,
        registry = registry,
        repository = effective_repo,
        tag = tag,
        platform = platform,
    )

    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        data = [image_tar],
        deps = [Label("//deploy/rules:dx_deploy_tools")],
    )

    dx_deployment(
        name = name,
        deploy = ":" + program_target,
        profile = profile,
    )
