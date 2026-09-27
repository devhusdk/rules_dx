"""Local-first OCI publisher for dx deploy."""

load("//rust/rules:defs.bzl", "rust_binary")
load(":defs.bzl", "dx_deployment")
load(":launcher.bzl", "rlocation_path")

OCI_SCHEMA_VERSION = 1

_VALID_TAG_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-"

_VALID_REGISTRY_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-/:"

_VALID_REPOSITORY_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-/"

OCI_DEFAULT_REGISTRY = "ghcr.io"

def oci_tag_charset():
    """Returns the launcher-safe image-tag charset via registry query."""
    return _VALID_TAG_CHARS

def oci_registry_charset():
    """Returns the launcher-safe registry charset via registry query."""
    return _VALID_REGISTRY_CHARS

def oci_repository_charset():
    """Returns the launcher-safe repository charset via registry query."""
    return _VALID_REPOSITORY_CHARS

def oci_schema_error():
    """Validates the versioned registry/tag charset schema."""
    if OCI_SCHEMA_VERSION != 1:
        return "oci tag: unsupported schema v" + str(OCI_SCHEMA_VERSION) + " (want v1)"
    if type(_VALID_TAG_CHARS) != "string" or _VALID_TAG_CHARS == "":
        return "oci tag: want a non-empty charset (schema v1)"
    seen = {}
    for c in _VALID_TAG_CHARS.elems():
        if c in seen:
            return "oci tag: duplicate charset char '" + c + "'"
        seen[c] = True
        if c in ["\"", "\\", "'", " ", "\n", "`", "$", "!", "#", "&", "|", ";", "<", ">", "(", ")", "[", "]", "{", "}", "*", "?", "~", "^", ":", ",", "/", "+"]:
            return "oci tag: unsafe charset char '" + c + "' (must stay launcher-safe)"
    if type(_VALID_REGISTRY_CHARS) != "string" or _VALID_REGISTRY_CHARS == "":
        return "oci registry: want a non-empty charset (schema v1)"
    seen_registry = {}
    for c in _VALID_REGISTRY_CHARS.elems():
        if c in seen_registry:
            return "oci registry: duplicate charset char '" + c + "'"
        seen_registry[c] = True
        if c in ["\"", "\\", "'", " ", "\n", "`", "$", "!", "#", "&", "|", ";", "<", ">", "(", ")", "[", "]", "{", "}", "*", "?", "~", "^", ",", "+", "=", "%", "@"]:
            return "oci registry: unsafe charset char '" + c + "' (must stay launcher-safe)"
    if type(_VALID_REPOSITORY_CHARS) != "string" or _VALID_REPOSITORY_CHARS == "":
        return "oci repository: want a non-empty charset (schema v1)"
    seen_repo = {}
    for c in _VALID_REPOSITORY_CHARS.elems():
        if c in seen_repo:
            return "oci repository: duplicate charset char '" + c + "'"
        seen_repo[c] = True
        if c in ["\"", "\\", "'", " ", "\n", "`", "$", "!", "#", "&", "|", ";", "<", ">", "(", ")", "[", "]", "{", "}", "*", "?", "~", "^", ":", ",", "+", "=", "%", "@"]:
            return "oci repository: unsafe charset char '" + c + "' (must stay launcher-safe)"
    return ""

def oci_tag_error(tag):
    """Validates one image tag value."""
    if type(tag) != "string" or tag == "":
        return ("oci_deploy: invalid tag '" + str(tag) +
                "': want a non-empty tag (for example 'latest')")
    for c in tag.elems():
        if c not in _VALID_TAG_CHARS:
            return ("oci_deploy: invalid tag '" + tag +
                    "': want only [A-Za-z0-9._-] so the tag embeds " +
                    "safely in the deploy launcher")
    return ""

def oci_registry_error(registry):
    """Validates one registry value."""
    if type(registry) != "string" or registry == "":
        return ("oci_deploy: invalid registry '" + str(registry) +
                "': want a non-empty registry (for example '" +
                OCI_DEFAULT_REGISTRY + "')")
    for c in registry.elems():
        if c not in _VALID_REGISTRY_CHARS:
            return ("oci_deploy: invalid registry '" + registry +
                    "': want only [A-Za-z0-9._-/:] so the registry embeds " +
                    "safely in the deploy launcher")
    if registry.startswith("/") or registry.endswith("/"):
        return ("oci_deploy: invalid registry '" + registry +
                "': want no leading or trailing slash")
    if "://" in registry:
        return ("oci_deploy: invalid registry '" + registry +
                "': want a host plus optional path, never a URL scheme")
    for c in registry.elems():
        if c in ["\"", "\\", "'", " ", "\n", "`", "$"]:
            return ("oci_deploy: invalid registry '" + registry +
                    "': want no quotes, backslashes, spaces, or newlines " +
                    "so the registry embeds safely in the deploy launcher")
    return ""

def oci_repository_error(repository):
    """Validates one repository value."""
    if type(repository) != "string" or repository == "":
        return ("oci_deploy: invalid repository '" + str(repository) +
                "': want a non-empty repository (for example 'oci_demo')")
    for c in repository.elems():
        if c not in _VALID_REPOSITORY_CHARS:
            return ("oci_deploy: invalid repository '" + repository +
                    "': want only [A-Za-z0-9._-/] so the repository embeds " +
                    "safely in the deploy launcher")
    if repository.startswith("/") or repository.endswith("/"):
        return ("oci_deploy: invalid repository '" + repository +
                "': want no leading or trailing slash")
    if "//" in repository:
        return ("oci_deploy: invalid repository '" + repository +
                "': want no empty path segments")
    return ""

def oci_tar_error(filename):
    """Validates one image-tar filename value."""
    if type(filename) != "string" or filename == "":
        return ("oci_deploy: invalid image_tar '" + str(filename) +
                "': want a non-empty .tar filename")
    if not (filename.endswith(".tar") or filename.endswith(".tar.gz")):
        return ("oci_deploy: invalid image_tar '" + filename +
                "': want a filename ending in .tar or .tar.gz")
    for c in filename.elems():
        if c in ["\"", "\\", "'", " ", "\n", "`", "$"]:
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
                  "fn main() { std::process::exit(dx_deploy_tools::oci_main(IMAGE_RLOC, OCI_REGISTRY, OCI_REPOSITORY, OCI_TAG, &std::env::args().collect::<Vec<_>>())); }\n",
    )
    return [DefaultInfo(files = depset([launcher]))]

_oci_launcher = rule(
    implementation = _oci_launcher_impl,
    attrs = {
        "image_tar": attr.label(
            allow_single_file = True,
            mandatory = True,
        ),
        "registry": attr.string(
            mandatory = True,
        ),
        "repository": attr.string(
            mandatory = True,
        ),
        "tag": attr.string(
            mandatory = True,
        ),
    },
)

def oci_deploy(name, image_tar, tag = "latest", registry = OCI_DEFAULT_REGISTRY, repository = "", profile = "release"):
    """Publishes one image tar as a local-first OCI deployment."""
    effective_repo = repository if repository != "" else name
    tag_error = oci_tag_error(tag)
    if tag_error != "":
        fail(tag_error + " (in " + native.package_name() + ":" + name + ")")
    registry_error = oci_registry_error(registry)
    if registry_error != "":
        fail(registry_error + " (in " + native.package_name() + ":" + name + ")")
    repo_error = oci_repository_error(effective_repo)
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
    )

    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        data = [image_tar],
        deps = ["//deploy/rules:dx_deploy_tools"],
    )

    dx_deployment(
        name = name,
        deploy = ":" + program_target,
        profile = profile,
    )
