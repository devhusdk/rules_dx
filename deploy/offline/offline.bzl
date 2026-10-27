"""Vendored offline/airgap bundle trees."""

load("@bazel_skylib//rules:copy_file.bzl", "copy_file")

# Keep in step with host_launcher in deploy/offline/bootstrap/src/lib.rs.
_LAUNCHERS = [
    "bazelisk-darwin-amd64",
    "bazelisk-darwin-arm64",
    "bazelisk-linux-amd64",
    "bazelisk-linux-arm64",
    "bazelisk-windows-amd64.exe",
]

def offline_set_error(set):
    """Validates one vendored advisory set name."""
    if set in ["cargo", "npm", "maven", "nuget", "go", "rubygems"]:
        return ""
    return ("offline_bundle: invalid advisory set '" + str(set) +
            "': want one of cargo, npm, maven, nuget, go, rubygems")

def offline_launcher_error(basename):
    """Validates one launcher file name the bootstrap can pick for a host."""
    if basename in _LAUNCHERS:
        return ""
    return ("offline_bundle: invalid launcher '" + str(basename) +
            "': want one of " + ", ".join(_LAUNCHERS))

def offline_srcs_error(srcs):
    """Validates one manifest input list."""
    if len(srcs) == 0:
        return "offline_bundle: need at least one bundle file"
    return ""

def _basename(label):
    """Returns the file basename for one label string."""
    return str(label).rsplit(":", 1)[-1].rsplit("/", 1)[-1]

def _stem_key(basename):
    """Returns the target stem fragment for one basename."""
    return basename.replace(".", "_")

def offline_basenames_error(srcs):
    """Rejects bundle files that share a basename or target stem."""
    seen = {}
    for src in srcs:
        base = _basename(src)
        key = _stem_key(base)
        if key in seen:
            (prev_base, prev_src) = seen[key]
            if prev_base == base:
                return ("offline_bundle: duplicate bundle basename '" + base + "': " +
                        prev_src + " and " + str(src))
            return ("offline_bundle: bundle basenames '" + prev_base + "' and '" + base +
                    "' both make target stem '" + key + "': " + prev_src + " and " + str(src))
        seen[key] = (base, str(src))
    return ""

def offline_meta_error(advisory, advisory_meta):
    """Requires one recorded-date identity file for each advisory snapshot."""
    for set in advisory:
        if set not in advisory_meta:
            return ("offline_bundle: advisory set '" + str(set) +
                    "' has no advisory_meta identity file: add advisory_meta = {'" +
                    str(set) + "': <" + str(set) + ".meta.json from advisory_prep>}")
    for set in advisory_meta:
        if set not in advisory:
            return ("offline_bundle: advisory_meta set '" + str(set) +
                    "' has no snapshot in advisory")
    return ""

def offline_bundle_layout(name, advisory, launchers, advisory_meta = {}):
    """Returns the sorted (dir, dest, src) entries of one bundle tree."""
    entries = [("advisory", set + ".json", src) for set, src in advisory.items()]
    entries += [("advisory", set + ".meta.json", src) for set, src in advisory_meta.items()]
    entries += [("bazelisk", _basename(src), src) for src in launchers]
    return sorted(entries)

def offline_bundle(name, advisory, launchers, advisory_meta):
    """Assembles one vendored offline bundle tree.

    The tree is `<name>/advisory/<set>.json`, `<name>/advisory/<set>.meta.json`,
    `<name>/advisory/SHA256SUMS`, `<name>/bazelisk/<launcher>` and
    `<name>/bazelisk/SHA256SUMS`: the layout bootstrap_offline reads. The
    manifest pins every file, so the recorded date cannot change unseen.

    Args:
      name: the bundle target name, also the tree directory.
      advisory: dict of advisory set name to snapshot label.
      launchers: bazelisk launcher labels, named like the bootstrap expects.
      advisory_meta: dict of advisory set name to the identity file
        `advisory_prep` wrote next to that snapshot (`<set>.meta.json`). It
        carries the `retrieved_at` date and the snapshot `sha256`. Every set in
        `advisory` needs one, and no other set may appear.
    """
    where = " (in " + native.package_name() + ":" + name + ")"
    for set in advisory:
        set_err = offline_set_error(set)
        if set_err != "":
            fail(set_err + where)
    meta_err = offline_meta_error(advisory, advisory_meta)
    if meta_err != "":
        fail(meta_err + where)
    advisory_err = offline_srcs_error(advisory.values())
    if advisory_err != "":
        fail(advisory_err + where)
    launchers_err = offline_srcs_error(launchers)
    if launchers_err != "":
        fail(launchers_err + where)
    for src in launchers:
        launcher_err = offline_launcher_error(_basename(src))
        if launcher_err != "":
            fail(launcher_err + where)
    basenames_err = offline_basenames_error(launchers)
    if basenames_err != "":
        fail(basenames_err + where)
    tree = []
    sidecars = {}
    for (dir, dest, src) in offline_bundle_layout(name, advisory, launchers, advisory_meta):
        stem = name + "_" + dir + "_" + _stem_key(dest)
        out = name + "/" + dir + "/" + dest
        copy_file(name = stem + "_copy", src = src, out = out)
        sidecar = stem + ".sha256"
        native.genrule(
            name = stem + "_line",
            srcs = [":" + out],
            outs = [sidecar],
            tools = ["//deploy/rules:hasher"],
            cmd = "$(location //deploy/rules:hasher) $(location :" + out + ") $(OUTS)",
        )
        tree.append(":" + out)
        sidecars.setdefault(dir, []).append(":" + sidecar)
    for dir in sorted(sidecars):
        manifest = name + "/" + dir + "/SHA256SUMS"
        native.genrule(
            name = name + "_" + dir + "_manifest",
            srcs = sidecars[dir],
            outs = [manifest],
            cmd = "cat $(SRCS) > $(OUTS)",
        )
        tree.append(":" + manifest)
    native.filegroup(name = name, srcs = tree)
