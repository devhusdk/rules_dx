"""SBOM + provenance generation for releases."""

load("//deploy/rules:validate.bzl", "genrule_arg")

def sbom_filenames(name):
    """Returns the deterministic (spdx, provenance) output names."""
    return (name + ".spdx.json", name + ".provenance.json")

SBOM_BUILDER_DRY_RUN = "https://github.com/devhusdk/rules_dx/.github/workflows/publish-dry-run.yml"
SBOM_BUILDER_RELEASE = "https://github.com/devhusdk/rules_dx/.github/workflows/release.yml"

def sbom_builder_error(builder_id):
    """Validates one SLSA builder id against the allowlist."""
    if builder_id == SBOM_BUILDER_DRY_RUN or builder_id == SBOM_BUILDER_RELEASE:
        return ""
    return ("sbom_release: invalid builder '" + str(builder_id) +
            "': want '" + SBOM_BUILDER_DRY_RUN + "' (dry-run demo only) or '" +
            SBOM_BUILDER_RELEASE + "' (owner-approved release)")

def sbom_release(name, artifact, package_name = "dx", supplier = "rules_dx", builder_id = "https://github.com/devhusdk/rules_dx/.github/workflows/publish-dry-run.yml"):
    """Generates SPDX 2.3 JSON + SLSA provenance for one release artifact."""
    builder_err = sbom_builder_error(builder_id)
    if builder_err != "":
        fail(builder_err + " (in " + native.package_name() + ":" + name + ")")
    (spdx, provenance) = sbom_filenames(name)
    src_target = artifact

    native.genrule(
        name = name + "_spdx",
        srcs = [src_target],
        outs = [spdx],
        tools = ["//deploy/release:release_tool"],
        cmd = "$(location //deploy/release:release_tool) spdx $(location " + src_target + ") $(OUTS) " + genrule_arg(package_name) + " " + genrule_arg(supplier),
    )

    native.genrule(
        name = name + "_provenance",
        srcs = [src_target],
        outs = [provenance],
        tools = ["//deploy/release:release_tool"],
        cmd = "$(location //deploy/release:release_tool) provenance $(location " + src_target + ") $(OUTS) " + genrule_arg(builder_id),
    )

    native.filegroup(
        name = name,
        srcs = [":" + name + "_spdx", ":" + name + "_provenance"],
    )
