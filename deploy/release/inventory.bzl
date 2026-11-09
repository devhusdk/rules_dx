"""Lists the third-party crates one Rust binary ships, with their license texts.

For rules_dx's own release archives. Not a consumer API.
"""

load("@rules_rust//rust:defs.bzl", "rust_common")
load("//deploy/rules:native_runtime.bzl", "NativeRuntimeInfo")

ShippedCratesInfo = provider(
    doc = "Third-party crates reachable through runtime deps.",
    fields = {
        "crates": "Depset of structs with crate, version, label, manifest and texts.",
    },
)

INVENTORY_SCHEMA = "shipped-inventory/v1"

_TEXT_PREFIXES = ["LICENSE", "LICENCE", "COPYING", "NOTICE"]

_BUNDLED_TEXTS = {"libz-sys": ["src/zlib/LICENSE"]}

_WALKED_ATTRS = ["deps", "upstream"]

def license_text_name(basename):
    """Returns whether a file name is a license or notice text."""
    upper = basename.upper()
    for prefix in _TEXT_PREFIXES:
        if upper.startswith(prefix):
            return True
    return False

def license_text_member(name, crate = ""):
    """Returns whether a crate file path is a license text: LICENSE* at the root, a file in LICENSES/, or a listed bundled text."""
    if name in _BUNDLED_TEXTS.get(crate, []):
        return True
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
            if f.owner.package == "" and license_text_member(f.owner.name, crate)
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
    document = {"crates": records, "schema": INVENTORY_SCHEMA}
    runtime_records = []
    runtime_inputs = []
    for target in ctx.attr.runtimes:
        info = target[NativeRuntimeInfo]
        if info.redistribution == "build_only":
            fail("shipped_inventory " + str(ctx.label) + ": runtime " + info.name + " is build_only and cannot ship")
        if info.redistribution == "app_local" and info.notice == None:
            fail("shipped_inventory " + str(ctx.label) + ": app_local runtime " + info.name + " has no notice")
        runtime_records.append({
            "files": [f.path for f in info.files],
            "name": info.name,
            "notice": info.notice.path if info.notice else None,
            "redistribution": info.redistribution,
            "source": info.source,
            "version": info.version,
        })
        runtime_inputs.extend(info.files)
        if info.notice:
            runtime_inputs.append(info.notice)
    outputs_extra = []
    arguments_extra = []
    if runtime_records:
        document["runtimes"] = runtime_records
        runtimes_json = ctx.actions.declare_file(ctx.label.name + ".runtimes.json")
        outputs_extra.append(runtimes_json)
        arguments_extra.append(runtimes_json.path)
    shipped = ctx.actions.declare_file(ctx.label.name + ".shipped.json")
    ctx.actions.write(shipped, json.encode(document) + "\n")
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
        ] + arguments_extra,
        inputs = inputs + runtime_inputs + [shipped],
        outputs = [manifest] + texts + outputs_extra,
        mnemonic = "DxShippedInventory",
        progress_message = "Dx shipped inventory %{label}",
    )
    return [
        DefaultInfo(files = depset([manifest])),
        OutputGroupInfo(texts = depset(texts), runtimes = depset(outputs_extra)),
    ]

_shipped_inventory = rule(
    implementation = _shipped_inventory_impl,
    doc = "Writes the NOTICE manifest and per-crate texts for one Rust binary.",
    attrs = {
        "app": attr.label(doc = "Rust binary to inventory.", mandatory = True, aspects = [shipped_crates_aspect]),
        "runtimes": attr.label_list(
            doc = "native_runtime targets the archive ships next to the app.",
            providers = [NativeRuntimeInfo],
        ),
        "_licenses": attr.label(default = Label("//:license.toml"), allow_single_file = True),
        "_lock": attr.label(default = Label("//:cargo-bazel-lock.json"), allow_single_file = True),
        "_tool": attr.label(
            cfg = "exec",
            default = Label("//deploy/release:release_tool"),
            executable = True,
        ),
    },
)

def shipped_inventory(name, app, runtimes = [], **kwargs):
    """Declares `name` (the NOTICE manifest) and `name_texts` (the per-crate texts) for one Rust binary."""
    _shipped_inventory(name = name, app = app, runtimes = runtimes, **kwargs)
    if runtimes:
        native.filegroup(
            name = name + "_runtimes",
            srcs = [":" + name],
            output_group = "runtimes",
            **kwargs
        )
    native.filegroup(
        name = name + "_texts",
        srcs = [":" + name],
        output_group = "texts",
        **kwargs
    )
