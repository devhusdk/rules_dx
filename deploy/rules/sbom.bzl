"""SPDX SBOM for one deployable artifact."""

load("@rules_rust//rust:defs.bzl", "rust_common")

DxSbomInfo = provider(
    doc = "Inventory lines for the crates in one target's dependency closure.",
    fields = {
        "lines": "depset of strings: package|cargo|version|NOASSERTION||",
        "own": "String: the target's own crate line, or empty.",
    },
)

_LOCAL_VERSION = "0.0.0"

_EDGES = ["deps", "proc_macro_deps", "srcs", "crate", "upstream"]

def sbom_crate_line(name, repo):
    """Returns the inventory line for one crate; `repo` is the label's repository name."""
    version = _LOCAL_VERSION
    if repo:
        tail = repo.rsplit("-", 1)
        if len(tail) == 2 and tail[1][:1].isdigit():
            version = tail[1]
    return "|".join([name, "cargo", version, "NOASSERTION", "", ""])

def _sbom_aspect_impl(target, ctx):
    own = ""
    if rust_common.crate_info in target:
        info = target[rust_common.crate_info]
        if not getattr(info, "is_test", False):
            own = sbom_crate_line(info.name, target.label.repo_name)
    transitive = []
    for attr in _EDGES:
        deps = getattr(ctx.rule.attr, attr, [])
        for dep in deps if type(deps) == "list" else [deps]:
            if dep != None and DxSbomInfo in dep:
                transitive.append(dep[DxSbomInfo].lines)
                if attr == "upstream" and own == "":
                    own = dep[DxSbomInfo].own
    return [DxSbomInfo(own = own, lines = depset([own] if own else [], transitive = transitive))]

dx_sbom_aspect = aspect(
    implementation = _sbom_aspect_impl,
    attr_aspects = _EDGES,
)

def _dx_sbom_impl(ctx):
    artifact = ctx.file.artifact
    info = ctx.attr.artifact[DxSbomInfo]
    lines = [line for line in info.lines.to_list() if line != info.own]
    inventory = ctx.actions.declare_file(ctx.label.name + ".inventory")
    ctx.actions.write(
        output = inventory,
        content = "".join([line + "\n" for line in sorted(lines)]),
    )
    out = ctx.actions.declare_file(ctx.label.name + ".spdx.json")
    ctx.actions.run(
        executable = ctx.executable._release_tool,
        arguments = ["spdx", artifact.path, out.path, ctx.attr.package_name or ctx.label.name, ctx.attr.supplier, inventory.path],
        inputs = [artifact, inventory],
        outputs = [out],
        mnemonic = "DxSbom",
        progress_message = "Writing SPDX SBOM " + out.short_path,
    )
    return [DefaultInfo(files = depset([out]))]

dx_sbom = rule(
    implementation = _dx_sbom_impl,
    doc = "Writes <name>.spdx.json listing the crates in the dependency closure of `artifact`.",
    attrs = {
        "artifact": attr.label(
            mandatory = True,
            allow_single_file = True,
            aspects = [dx_sbom_aspect],
            doc = "The deployable the SBOM describes.",
        ),
        "package_name": attr.string(doc = "Name of the described package; defaults to the rule name."),
        "supplier": attr.string(default = "rules_dx", doc = "Supplier recorded on the described package."),
        "_release_tool": attr.label(
            default = Label("//deploy/release:release_tool"),
            executable = True,
            cfg = "exec",
        ),
    },
)
