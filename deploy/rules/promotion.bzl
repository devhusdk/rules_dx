"""Staging-to-production promotion publisher for dx deploy."""

load("//rust/rules:defs.bzl", "rust_binary")
load(":defs.bzl", "dx_deployment")
load(":launcher.bzl", "rlocation_path")

PROMOTION_SCHEMA_VERSION = 1

_VALID_ENVIRONMENT_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-"

_VALID_VERSION_CHARS = "ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789._-+"

_VALID_ARTIFACT_SUFFIXES = [".tar.gz", ".tar", ".tgz", ".whl", ".jar", ".nupkg", ".zip"]

def promotion_schema_error():
    """Validates the versioned environment charset schema."""
    if PROMOTION_SCHEMA_VERSION != 1:
        return "promotion: unsupported schema v" + str(PROMOTION_SCHEMA_VERSION) + " (want v1)"
    if type(_VALID_ENVIRONMENT_CHARS) != "string" or _VALID_ENVIRONMENT_CHARS == "":
        return "promotion: want a non-empty charset (schema v1)"
    seen = {}
    for c in _VALID_ENVIRONMENT_CHARS.elems():
        if c in seen:
            return "promotion: duplicate charset char '" + c + "'"
        seen[c] = True
        if c in ["\"", "\\", "'", " ", "\n", "`", "$", "!", "#", "&", "|", ";", "<", ">", "(", ")", "[", "]", "{", "}", "*", "?", "~", "^", ":", ",", "/", "+"]:
            return "promotion: unsafe charset char '" + c + "' (must stay launcher-safe)"
    if type(_VALID_VERSION_CHARS) != "string" or _VALID_VERSION_CHARS == "":
        return "promotion: want a non-empty version charset (schema v1)"
    seen_version = {}
    for c in _VALID_VERSION_CHARS.elems():
        if c in seen_version:
            return "promotion: duplicate version charset char '" + c + "'"
        seen_version[c] = True
        if c in ["\"", "\\", "'", " ", "\n", "`", "$", "!", "#", "&", "|", ";", "<", ">", "(", ")", "[", "]", "{", "}", "*", "?", "~", "^", ":", ",", "/"]:
            return "promotion: unsafe version charset char '" + c + "' (must stay launcher-safe)"
    if type(_VALID_ARTIFACT_SUFFIXES) != "list" or len(_VALID_ARTIFACT_SUFFIXES) == 0:
        return "promotion: want a non-empty artifact suffix list (schema v1)"
    return ""

def promotion_environment_error(environment):
    """Validates one promotion environment name value."""
    if type(environment) != "string" or environment == "":
        return ("promotion_deploy: invalid environment '" + str(environment) +
                "': want a non-empty environment (for example 'staging')")
    for c in environment.elems():
        if c not in _VALID_ENVIRONMENT_CHARS:
            return ("promotion_deploy: invalid environment '" + environment +
                    "': want only [A-Za-z0-9._-] so the environment embeds " +
                    "safely in the deploy launcher")
    return ""

def promotion_version_error(version):
    """Validates one promotion version value."""
    if type(version) != "string" or version == "":
        return ("promotion_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '1.2.3')")
    for c in version.elems():
        if c not in _VALID_VERSION_CHARS:
            return ("promotion_deploy: invalid version '" + version +
                    "': want only [A-Za-z0-9._-+] so the version embeds " +
                    "safely in the deploy launcher")
    return ""

def promotion_artifact_error(filename):
    """Validates one promotion artifact filename value."""
    if type(filename) != "string" or filename == "":
        return ("promotion_deploy: invalid artifact '" + str(filename) +
                "': want a non-empty deploy artifact filename")
    matched = False
    for suffix in _VALID_ARTIFACT_SUFFIXES:
        if filename.endswith(suffix):
            matched = True
    if not matched:
        return ("promotion_deploy: invalid artifact '" + filename +
                "': want one of .tar.gz, .tar, .tgz, .whl, .jar, .nupkg, .zip")
    for c in filename.elems():
        if c in ["\"", "\\", "'", " ", "\n", "`", "$"]:
            return ("promotion_deploy: invalid artifact '" + filename +
                    "': want no quotes, backslashes, spaces, or newlines " +
                    "so the filename embeds safely in the deploy launcher")
    return ""

def promotion_edge_error(from_environment, to_environment):
    """Validates one staging-to-production promotion edge."""
    if from_environment == to_environment:
        return ("promotion_deploy: invalid edge '" + from_environment + " -> " +
                to_environment + "': source and target environments must differ")
    return ""

def _promotion_launcher_impl(ctx):
    """Expands the rust_binary launcher for one promotion deployment."""
    artifact_files = ctx.attr.artifact[DefaultInfo].files.to_list()
    if len(artifact_files) != 1:
        fail("promotion_deploy " + str(ctx.label) + ": artifact " +
             str(ctx.attr.artifact.label) + " provides " +
             str(len(artifact_files)) + " files, want exactly one deploy artifact")
    artifact_file = artifact_files[0]
    artifact_error = promotion_artifact_error(artifact_file.basename)
    if artifact_error != "":
        fail(artifact_error + " (in " + str(ctx.label) + ")")
    artifact_rloc = rlocation_path(ctx, artifact_file)

    launcher = ctx.actions.declare_file(ctx.label.name + ".rs")
    ctx.actions.write(
        output = launcher,
        content = "const ARTIFACT_RLOC: &str = \"" + artifact_rloc + "\";\n" +
                  "const DEPLOY_NAME: &str = \"" + ctx.attr.deploy_name + "\";\n" +
                  "const FROM_ENV: &str = \"" + ctx.attr.from_environment + "\";\n" +
                  "const TO_ENV: &str = \"" + ctx.attr.to_environment + "\";\n" +
                  "const VERSION: &str = \"" + ctx.attr.version + "\";\n" +
                  "fn main() { std::process::exit(dx_deploy_tools::promotion_main(ARTIFACT_RLOC, DEPLOY_NAME, FROM_ENV, TO_ENV, VERSION, &std::env::args().collect::<Vec<_>>())); }\n",
    )
    return [DefaultInfo(files = depset([launcher]))]

_promotion_launcher = rule(
    implementation = _promotion_launcher_impl,
    attrs = {
        "artifact": attr.label(
            allow_single_file = True,
            mandatory = True,
        ),
        "deploy_name": attr.string(
            mandatory = True,
        ),
        "from_environment": attr.string(
            mandatory = True,
        ),
        "to_environment": attr.string(
            mandatory = True,
        ),
        "version": attr.string(
            mandatory = True,
        ),
    },
)

def promotion_deploy(name, artifact, from_environment = "staging", to_environment = "production", version = "0.0.0", profile = "release"):
    """Promotes one pinned artifact across environments with gated health and rollback."""
    from_error = promotion_environment_error(from_environment)
    if from_error != "":
        fail(from_error + " (in " + native.package_name() + ":" + name + ")")
    to_error = promotion_environment_error(to_environment)
    if to_error != "":
        fail(to_error + " (in " + native.package_name() + ":" + name + ")")
    edge_error = promotion_edge_error(from_environment, to_environment)
    if edge_error != "":
        fail(edge_error + " (in " + native.package_name() + ":" + name + ")")
    version_error = promotion_version_error(version)
    if version_error != "":
        fail(version_error + " (in " + native.package_name() + ":" + name + ")")

    program_target = name + "_program"
    launcher_target = program_target + "_launcher"
    _promotion_launcher(
        name = launcher_target,
        artifact = artifact,
        deploy_name = name,
        from_environment = from_environment,
        to_environment = to_environment,
        version = version,
    )

    rust_binary(
        name = program_target,
        srcs = [":" + launcher_target],
        data = [artifact],
        deps = ["//deploy/rules:dx_deploy_tools"],
    )

    dx_deployment(
        name = name,
        deploy = ":" + program_target,
        profile = profile,
    )
