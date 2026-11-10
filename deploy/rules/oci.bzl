"""Local-first OCI publisher for dx deploy."""

load("//core:platform.bzl", "PLATFORM_ATTRS", "platform_identity")
load(":config.bzl", "DEPLOY_TOOL", "deploy_attrs", "deploy_config", "deployment_budget", "rlocation_path")
load(":deployment.bzl", "dx_deployment")
load(":validate_oci.bzl", "OCI_DEFAULT_REGISTRY", "oci_image_error", "oci_platform", "oci_platform_error", "oci_registry_error", "oci_repository_error", "oci_tag_error")

def _oci_config_impl(ctx):
    """Writes the deploy config for one OCI deployment."""
    tar_files = ctx.attr.image_tar[DefaultInfo].files.to_list()
    if len(tar_files) != 1:
        fail("oci_deploy " + str(ctx.label) + ": image_tar " +
             str(ctx.attr.image_tar.label) + " provides " +
             str(len(tar_files)) + " files, want exactly one .tar/.tar.gz archive or OCI layout directory")
    tar_file = tar_files[0]
    tar_error = oci_image_error(tar_file.basename, tar_file.is_directory)
    if tar_error != "":
        fail(tar_error + " (in " + str(ctx.label) + ")")

    platform = oci_platform(platform_identity(ctx))
    platform_error = oci_platform_error(platform)
    if platform_error != "":
        fail(platform_error + " (in " + str(ctx.label) + ")")

    files = [tar_file]
    repo = ("@" + ctx.label.repo_name) if ctx.label.repo_name else ""
    deploy_name = ctx.label.name[:-len("_config")] if ctx.label.name.endswith("_config") else ctx.label.name
    fields = {
        "image_rloc": rlocation_path(ctx, tar_file),
        "label": repo + "//" + ctx.label.package + ":" + deploy_name,
        "platform": platform,
        "registry": ctx.attr.registry,
        "repository": ctx.attr.repository,
        "scan": ctx.attr.scan,
        "scan_severity": ctx.attr.scan_severity,
        "tag": ctx.attr.image_tag,
    }
    if ctx.file.sbom:
        files.append(ctx.file.sbom)
        fields["sbom_rloc"] = rlocation_path(ctx, ctx.file.sbom)
    return deploy_config(ctx, "oci", fields, ctx.runfiles(files = files)) + [
        OutputGroupInfo(dx_oci_image = depset([tar_file])),
    ]

_oci_config = rule(
    implementation = _oci_config_impl,
    doc = "Writes the deploy config for one OCI deployment.",
    attrs = {
        "image_tar": attr.label(
            doc = "docker save or OCI layout archive (.tar or .tar.gz), or OCI layout directory, holding one image with uncompressed, gzip, or zstd layers.",
            allow_single_file = True,
            mandatory = True,
        ),
        "scan": attr.string(
            doc = "Image vulnerability scan with trivy: off, warn or fail.",
            default = "fail",
        ),
        "scan_severity": attr.string(
            doc = "Lowest severity the scan reports: LOW, MEDIUM, HIGH or CRITICAL.",
            default = "HIGH",
        ),
        "registry": attr.string(
            doc = "Container registry host the image is pushed to.",
            mandatory = True,
        ),
        "repository": attr.string(
            doc = "Image repository path within the registry.",
            mandatory = True,
        ),
        "image_tag": attr.string(
            doc = "Image tag the deployment publishes.",
            mandatory = True,
        ),
        "sbom": attr.label(
            doc = "Optional dx_sbom target kept with the deployment for the cosign attestation.",
            allow_single_file = True,
        ),
    } | PLATFORM_ATTRS,
)

def oci_deploy(name, image_tar, platform, image_tag = "latest", registry = OCI_DEFAULT_REGISTRY, repository = "", build_mode = "release", size_budget = None, sbom = None, scan = "fail", scan_severity = "HIGH", tags = None, visibility = None, testonly = False, target_compatible_with = None):
    """Publishes one docker save or OCI layout archive or OCI layout directory holding one image for `platform`, a linux amd64 or arm64 platform label, as a local-first OCI deployment, optionally failing above size_budget such as '500MB', carrying a dx_sbom target as `sbom`. The staged image is scanned with trivy (DX_TRIVY, DX_TRIVY_CACHE) for findings at or above scan_severity not covered by security.toml exceptions; scan is off, warn or fail, and fail also stops the push. `dx security` scans the same image (output group dx_oci_image of the `<name>_config` target) and prints one row per image."""
    attrs = deploy_attrs(tags, visibility, testonly, target_compatible_with)
    effective_repo = repository if repository != "" else name
    if scan not in ["off", "warn", "fail"]:
        fail("oci_deploy: invalid scan '" + str(scan) + "': want off, warn or fail (in " + native.package_name() + ":" + name + ")")
    if scan_severity not in ["LOW", "MEDIUM", "HIGH", "CRITICAL"]:
        fail("oci_deploy: invalid scan_severity '" + str(scan_severity) + "': want LOW, MEDIUM, HIGH or CRITICAL (in " + native.package_name() + ":" + name + ")")
    tag_error = oci_tag_error(image_tag)
    if tag_error != "":
        fail(tag_error + " (in " + native.package_name() + ":" + name + ")")
    registry_error = oci_registry_error(registry)
    if registry_error != "":
        fail(registry_error + " (in " + native.package_name() + ":" + name + ")")
    repo_error = oci_repository_error(effective_repo, from_name = repository == "")
    if repo_error != "":
        fail(repo_error + " (in " + native.package_name() + ":" + name + ")")

    config_target = name + "_config"
    _oci_config(
        name = config_target,
        image_tar = image_tar,
        registry = registry,
        repository = effective_repo,
        scan = scan,
        scan_severity = scan_severity,
        image_tag = image_tag,
        platform = platform,
        sbom = sbom,
        **attrs
    )

    dx_deployment(
        name = name,
        sbom = sbom,
        config = ":" + config_target,
        deploy = DEPLOY_TOOL,
        build_mode = build_mode,
        **(deployment_budget(name, size_budget, [image_tar]) | attrs)
    )
