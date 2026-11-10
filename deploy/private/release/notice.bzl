"""NOTICE bundling for rules_dx's own release archives. Not a consumer API."""

load("//deploy/private:validate.bzl", "genrule_arg")

def notice_filenames(name):
    """Returns the deterministic NOTICE output name."""
    return (name + ".NOTICE", name + ".NOTICE.sha256")

def notice_root_error(root):
    """Validates one NOTICE root label value."""
    if type(root) != "string" or root == "":
        return ("notice_bundle: invalid root '" + str(root) +
                "': want a non-empty distributed-tier root label")
    return ""

def notice_manifest_error(manifest):
    """Validates one NOTICE manifest label value."""
    if manifest == None:
        return "notice_bundle: invalid manifest 'None': want the audited inventory manifest label"
    return ""

def notice_bundle(name, root, inventory, texts, runtimes = None, target_compatible_with = None):
    """Bundles one aggregated NOTICE; texts are matched by manifest path suffix."""
    root_err = notice_root_error(root)
    if root_err != "":
        fail(root_err + " (in " + native.package_name() + ":" + name + ")")
    manifest_err = notice_manifest_error(inventory)
    if manifest_err != "":
        fail(manifest_err + " (in " + native.package_name() + ":" + name + ")")
    if len(texts) == 0:
        fail("notice_bundle " + native.package_name() + ":" + name +
             ": need at least one license-words file")
    (notice, checksum) = notice_filenames(name)

    native.genrule(
        name = name + "_notice",
        srcs = [inventory] + texts + ([runtimes] if runtimes else []),
        outs = [notice],
        target_compatible_with = target_compatible_with,
        tools = ["//deploy/private/release:release_tool"],
        cmd = "$(location //deploy/private/release:release_tool) notice $(location " + inventory + ") $(OUTS) " + genrule_arg(root) + " " +
              ("--runtimes=$(location " + runtimes + ") " if runtimes else "") +
              " ".join(["$(locations " + text + ")" for text in texts]),
    )

    native.genrule(
        name = name + "_checksum",
        srcs = [":" + name + "_notice"],
        outs = [checksum],
        target_compatible_with = target_compatible_with,
        tools = ["//deploy/private:hasher"],
        cmd = "$(location //deploy/private:hasher) $(location :" + name + "_notice) $(OUTS)",
    )

    native.filegroup(
        name = name,
        srcs = [":" + name + "_notice", ":" + name + "_checksum"],
        target_compatible_with = target_compatible_with,
    )
