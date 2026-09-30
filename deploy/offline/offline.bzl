"""Vendored offline/airgap bundle manifests."""

def offline_manifest_name(name):
    """Returns the deterministic manifest output name."""
    return name + ".SHA256SUMS"

def offline_set_error(set):
    """Validates one vendored advisory set name."""
    if set in ["cargo", "npm", "maven", "nuget", "go", "rubygems"]:
        return ""
    return ("offline_bundle: invalid advisory set '" + str(set) +
            "': want one of cargo, npm, maven, nuget, go, rubygems")

def offline_srcs_error(srcs):
    """Validates one manifest input list."""
    if len(srcs) == 0:
        return "offline_bundle: need at least one bundle file"
    return ""

def _basename(label):
    """Returns the file basename for one label string."""
    parts = str(label).split("/")
    return parts[len(parts) - 1]

def offline_bundle(name, advisory, launchers):
    """Assembles one vendored offline bundle manifest pair."""
    for set in advisory:
        set_err = offline_set_error(set)
        if set_err != "":
            fail(set_err + " (in " + native.package_name() + ":" + name + ")")
    advisory_err = offline_srcs_error(advisory.values())
    if advisory_err != "":
        fail(advisory_err + " (in " + native.package_name() + ":" + name + ")")
    launchers_err = offline_srcs_error(launchers)
    if launchers_err != "":
        fail(launchers_err + " (in " + native.package_name() + ":" + name + ")")
    srcs = advisory.values() + launchers
    basenames = [_basename(src) for src in srcs]
    if len(basenames) != len(dict([(basename, True) for basename in basenames])):
        fail("offline_bundle " + native.package_name() + ":" + name +
             ": bundle basenames must be unique")
    sidecars = []
    for basename in sorted(basenames):
        sidecar = name + "_" + basename + ".sha256"
        src = [src for src in srcs if _basename(src) == basename][0]
        native.genrule(
            name = name + "_line_" + basename.replace(".", "_"),
            srcs = [src],
            outs = [sidecar],
            tools = ["//deploy/rules:hasher"],
            cmd = "$(location //deploy/rules:hasher) $(location " + src + ") $(OUTS)",
        )
        sidecars.append(":" + sidecar)
    (manifest, checksum) = (offline_manifest_name(name), offline_manifest_name(name) + ".sha256")
    native.genrule(
        name = name + "_manifest",
        srcs = sidecars,
        outs = [manifest],
        cmd = "cat $(SRCS) > $(OUTS)",
    )
    native.genrule(
        name = name + "_checksum",
        srcs = [":" + name + "_manifest"],
        outs = [checksum],
        tools = ["//deploy/rules:hasher"],
        cmd = "$(location //deploy/rules:hasher) $(location :" + name + "_manifest) $(OUTS)",
    )
    native.filegroup(
        name = name,
        srcs = [":" + name + "_manifest", ":" + name + "_checksum"],
    )
