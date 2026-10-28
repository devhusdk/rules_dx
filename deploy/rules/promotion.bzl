"""Staging-to-production promotion publisher for dx deploy."""

load(":config.bzl", "DEPLOY_TOOL", "deploy_config", "rlocation_path")
load(":defs.bzl", "dx_deployment")
load(":validate.bzl", "NAME_CHARS", "VERSION_CHARS", "only_chars")

_VALID_ENVIRONMENT_CHARS = NAME_CHARS

_VALID_VERSION_CHARS = VERSION_CHARS

_VALID_ARTIFACT_SUFFIXES = [".tar.gz", ".tar", ".tgz", ".whl", ".jar", ".nupkg", ".zip"]

def promotion_environment_error(environment):
    """Validates one promotion environment name value."""
    if type(environment) != "string" or environment == "":
        return ("promotion_deploy: invalid environment '" + str(environment) +
                "': want a non-empty environment (for example 'staging')")
    if not only_chars(environment, _VALID_ENVIRONMENT_CHARS):
        return ("promotion_deploy: invalid environment '" + environment +
                "': want only [A-Za-z0-9._-]")
    return ""

def promotion_version_error(version):
    """Validates one promotion version value."""
    if type(version) != "string" or version == "":
        return ("promotion_deploy: invalid version '" + str(version) +
                "': want a non-empty version (for example '1.2.3')")
    if not only_chars(version, _VALID_VERSION_CHARS):
        return ("promotion_deploy: invalid version '" + version +
                "': want only [A-Za-z0-9._-+]")
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
    return ""

def promotion_edge_error(from_environment, to_environment):
    """Validates one staging-to-production promotion edge."""
    if from_environment == to_environment:
        return ("promotion_deploy: invalid edge '" + from_environment + " -> " +
                to_environment + "': source and target environments must differ")
    return ""

def _promotion_config_impl(ctx):
    """Writes the deploy config for one promotion deployment."""
    artifact_files = ctx.attr.artifact[DefaultInfo].files.to_list()
    if len(artifact_files) != 1:
        fail("promotion_deploy " + str(ctx.label) + ": artifact " +
             str(ctx.attr.artifact.label) + " provides " +
             str(len(artifact_files)) + " files, want exactly one deploy artifact")
    artifact_file = artifact_files[0]
    artifact_error = promotion_artifact_error(artifact_file.basename)
    if artifact_error != "":
        fail(artifact_error + " (in " + str(ctx.label) + ")")

    return deploy_config(
        ctx,
        "promotion",
        {
            "artifact_rloc": rlocation_path(ctx, artifact_file),
            "deploy_name": ctx.attr.deploy_name,
            "from_environment": ctx.attr.from_environment,
            "to_environment": ctx.attr.to_environment,
            "version": ctx.attr.version,
        },
        ctx.runfiles(files = [artifact_file]),
    )

_promotion_config = rule(
    implementation = _promotion_config_impl,
    doc = "Writes the deploy config for one promotion deployment.",
    attrs = {
        "artifact": attr.label(
            doc = "Single deploy artifact promoted by the deploy tool.",
            allow_single_file = True,
            mandatory = True,
        ),
        "deploy_name": attr.string(
            doc = "Deployment name recorded for the promotion.",
            mandatory = True,
        ),
        "from_environment": attr.string(
            doc = "Environment the artifact is promoted from.",
            mandatory = True,
        ),
        "to_environment": attr.string(
            doc = "Environment the artifact is promoted to.",
            mandatory = True,
        ),
        "version": attr.string(
            doc = "Release version recorded for the promotion.",
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

    config_target = name + "_config"
    _promotion_config(
        name = config_target,
        artifact = artifact,
        deploy_name = name,
        from_environment = from_environment,
        to_environment = to_environment,
        version = version,
    )

    dx_deployment(
        name = name,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        profile = profile,
    )
