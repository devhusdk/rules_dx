"""SBOM + provenance generation for releases."""

load("//deploy/rules:validate.bzl", "genrule_arg")

def sbom_filenames(name):
    """Returns the deterministic (spdx, provenance) output names."""
    return (name + ".spdx.json", name + ".provenance.json")

SBOM_BUILDER_RELEASE = "https://github.com/devhusdk/rules_dx/.github/workflows/release.yml"

def sbom_builder_error(builder_id):
    """Validates one SLSA builder id against the allowlist."""
    if builder_id == SBOM_BUILDER_RELEASE:
        return ""
    return ("sbom_release: invalid builder '" + str(builder_id) +
            "': want '" + SBOM_BUILDER_RELEASE + "'")

def sbom_release(name, artifact, package_name = "dx", supplier = "rules_dx", builder_id = SBOM_BUILDER_RELEASE, inventory = None, runtimes = None, **kwargs):
    """Generates SPDX 2.3 JSON + SLSA provenance for one release artifact."""
    builder_err = sbom_builder_error(builder_id)
    if builder_err != "":
        fail(builder_err + " (in " + native.package_name() + ":" + name + ")")
    (spdx, provenance) = sbom_filenames(name)
    src_target = artifact
    inventory_srcs = [inventory] if inventory else []
    inventory_arg = " $(location " + inventory + ")" if inventory else ""
    runtimes_srcs = [runtimes] if runtimes else []
    runtimes_arg = " --runtimes=$(location " + runtimes + ")" if runtimes else ""

    native.genrule(
        name = name + "_spdx",
        srcs = [src_target] + inventory_srcs + runtimes_srcs,
        outs = [spdx],
        tools = ["//deploy/release:release_tool"],
        cmd = "$(location //deploy/release:release_tool) spdx $(location " + src_target + ") $(OUTS) " + genrule_arg(package_name) + " " + genrule_arg(supplier) + inventory_arg + runtimes_arg,
        **kwargs
    )

    native.genrule(
        name = name + "_provenance",
        srcs = [src_target],
        outs = [provenance],
        tools = ["//deploy/release:release_tool"],
        cmd = "$(location //deploy/release:release_tool) provenance $(location " + src_target + ") $(OUTS) " + genrule_arg(builder_id),
        **kwargs
    )

    native.filegroup(
        name = name,
        srcs = [":" + name + "_spdx", ":" + name + "_provenance"],
        **kwargs
    )
