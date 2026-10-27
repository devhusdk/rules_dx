"""Lists the third-party crates one Rust binary ships, with their license texts."""

load("@rules_rust//rust:defs.bzl", "rust_common")

ShippedCratesInfo = provider(
    doc = "Third-party crates reachable through runtime deps.",
    fields = {
        "crates": "Depset of structs with crate, version, label, manifest and texts.",
    },
)

INVENTORY_SCHEMA = "shipped-inventory/v1"

_TEXT_PREFIXES = ["LICENSE", "LICENCE", "COPYING", "NOTICE"]

_WALKED_ATTRS = ["deps", "upstream"]

def license_text_name(basename):
    """Returns whether a file name is a license or notice text."""
    upper = basename.upper()
    for prefix in _TEXT_PREFIXES:
        if upper.startswith(prefix):
            return True
    return False

def license_text_member(name):
    """Returns whether a crate root file path is a license text: LICENSE* at the root or a file in LICENSES/."""
    if "/" not in name:
        return license_text_name(name)
    return name.startswith("LICENSES/") and name.count("/") == 1

def crate_name_tag(tags):
    """Returns the crate-name= tag value, or empty."""
    for tag in tags:
        if tag.startswith("crate-name="):
            return tag[len("crate-name="):]
    return ""

def inventory_text_path(name, crate, version):
    """Returns the output path of one crate text, relative to the package."""
    return name + "_texts/cargo/" + crate + "-" + version + "/LICENSE"

def _children(ctx):
    """Returns the crate depsets of the walked dependency attrs."""
    found = []
    for attr in _WALKED_ATTRS:
        value = getattr(ctx.rule.attr, attr, None)
        if value == None:
            continue
        for dep in (value if type(value) == "list" else [value]):
            if ShippedCratesInfo in dep:
                found.append(dep[ShippedCratesInfo].crates)
    return found

def _shipped_crates_aspect_impl(target, ctx):
    """Records one third-party crate and merges its runtime deps."""
    if rust_common.crate_info not in target:
        return [ShippedCratesInfo(crates = depset())]
    if target[rust_common.crate_info].type == "proc-macro":
        return [ShippedCratesInfo(crates = depset())]
    direct = []
    crate = crate_name_tag(getattr(ctx.rule.attr, "tags", []))
    if target.label.repo_name != "" and crate != "":
        texts = [
            f
            for f in getattr(ctx.rule.files, "compile_data", [])
            if f.owner.package == "" and license_text_member(f.owner.name)
        ]
        manifests = [
            f
            for f in getattr(ctx.rule.files, "compile_data", [])
            if f.owner.package == "" and f.owner.name == "Cargo.toml"
        ]
        direct.append(struct(
            crate = crate,
            manifest = manifests[0] if manifests else None,
            version = ctx.rule.attr.version,
            label = str(target.label),
            texts = tuple(sorted(texts, key = lambda f: f.basename)),
        ))
    return [ShippedCratesInfo(crates = depset(direct, transitive = _children(ctx)))]

shipped_crates_aspect = aspect(
    implementation = _shipped_crates_aspect_impl,
    doc = "Collects third-party crates through runtime deps, skipping proc macros and build scripts.",
    attr_aspects = _WALKED_ATTRS,
)

def _shipped_inventory_impl(ctx):
    """Joins the shipped crates with lockfile licenses into a NOTICE manifest."""
    crates = sorted(
        ctx.attr.app[ShippedCratesInfo].crates.to_list(),
        key = lambda c: (c.crate, c.version, c.label),
    )
    records = []
    inputs = [ctx.file._lock, ctx.file._licenses]
    texts = []
    for c in crates:
        record = {
            "crate": c.crate,
            "label": c.label,
            "texts": [f.path for f in c.texts],
            "version": c.version,
        }
        if c.manifest:
            record["manifest"] = c.manifest.path
            inputs.append(c.manifest)
        records.append(record)
        inputs.extend(c.texts)
        texts.append(ctx.actions.declare_file(inventory_text_path(ctx.label.name, c.crate, c.version)))
    shipped = ctx.actions.declare_file(ctx.label.name + ".shipped.json")
    ctx.actions.write(shipped, json.encode({"crates": records, "schema": INVENTORY_SCHEMA}) + "\n")
    manifest = ctx.actions.declare_file(ctx.label.name + ".txt")
    package = ctx.label.package + "/" if ctx.label.package else ""
    texts_dir = ctx.bin_dir.path + "/" + package + ctx.label.name + "_texts"
    ctx.actions.run(
        executable = ctx.executable._tool,
        arguments = [
            "inventory",
            shipped.path,
            ctx.file._lock.path,
            ctx.file._licenses.path,
            manifest.path,
            texts_dir,
        ],
        inputs = inputs + [shipped],
        outputs = [manifest] + texts,
        mnemonic = "DxShippedInventory",
        progress_message = "Dx shipped inventory %{label}",
    )
    return [
        DefaultInfo(files = depset([manifest])),
        OutputGroupInfo(texts = depset(texts)),
    ]

_shipped_inventory = rule(
    implementation = _shipped_inventory_impl,
    doc = "Writes the NOTICE manifest and per-crate texts for one Rust binary.",
    attrs = {
        "app": attr.label(mandatory = True, aspects = [shipped_crates_aspect]),
        "_licenses": attr.label(default = Label("//:licenses.toml"), allow_single_file = True),
        "_lock": attr.label(default = Label("//:cargo-bazel-lock.json"), allow_single_file = True),
        "_tool": attr.label(
            cfg = "exec",
            default = Label("//deploy/release:release_tool"),
            executable = True,
        ),
    },
)

def shipped_inventory(name, app, **kwargs):
    """Declares `name` (the NOTICE manifest) and `name_texts` (the per-crate texts) for one Rust binary."""
    _shipped_inventory(name = name, app = app, **kwargs)
    native.filegroup(
        name = name + "_texts",
        srcs = [":" + name],
        output_group = "texts",
        **kwargs
    )
