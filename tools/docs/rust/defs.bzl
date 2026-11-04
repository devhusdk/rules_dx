"""Rust API reference shards built from stable rustdoc JSON."""

load("@rules_rust//rust:defs.bzl", "rust_common")
load("@rules_rust//rust/private:providers.bzl", "LintsInfo")
load("@rules_rust//rust/private:rustdoc.bzl", "rustdoc_compile_action")
load("@rules_rust//rust/private:utils.bzl", "find_toolchain")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", _repo_relative = "repo_relative", _srcs_manifest_line = "srcs_manifest_line")

_RUSTDOC_FLAGS = ["-Zunstable-options", "--output-format=json"]

_GeneratedSrcsInfo = provider(
    doc = "Repo-relative paths of one crate's generated srcs.",
    fields = {"paths": "Repo-relative paths of the generated srcs."},
)

repo_relative = _repo_relative

srcs_manifest_line = _srcs_manifest_line

def _generated_srcs_aspect_impl(_target, ctx):
    srcs = getattr(ctx.rule.files, "srcs", [])
    return [_GeneratedSrcsInfo(paths = [repo_relative(f.short_path) for f in srcs if not f.is_source])]

_generated_srcs_aspect = aspect(
    implementation = _generated_srcs_aspect_impl,
    doc = "Collects the paths of generated crate srcs.",
)

def _rustdoc_json(ctx, crate_info, lints_info, output_dir):
    action = rustdoc_compile_action(
        ctx = ctx,
        toolchain = find_toolchain(ctx),
        crate_info = crate_info,
        lints_info = lints_info,
        output = output_dir,
        rustdoc_flags = ["--extern", "{}={}".format(crate_info.name, crate_info.output.path)] + _RUSTDOC_FLAGS,
    )
    ctx.actions.run(
        mnemonic = "DxDocsRustdoc",
        progress_message = "Generating rustdoc JSON for %{label}",
        outputs = [output_dir],
        executable = action.executable,
        inputs = action.inputs,
        env = dict(action.env, RUSTC_BOOTSTRAP = "1"),
        arguments = action.arguments,
        tools = action.tools,
    )

def rust_shard_action(ctx, target, package):
    """Runs stable rustdoc JSON and the extractor for one Rust crate target.

    Args:
        ctx: The rule or aspect context; it needs the rustdoc_compile_action attributes and `_extract`.
        target: The documented target, which carries a CrateInfo.
        package: Package segment of the symbol ids; empty means the crate name.

    Returns:
        A struct with the DocIr `shard` file and the `outputs` OutputGroupInfo fields.
    """
    crate_info = target[rust_common.crate_info]
    package = package or crate_info.name
    generated = {path: True for path in target[_GeneratedSrcsInfo].paths}
    lines = []
    for src in crate_info.srcs.to_list():
        rel = repo_relative(src.short_path)
        lines.append(srcs_manifest_line(src.path, rel, rel in generated))
    for dep in crate_info.deps.to_list():
        build_info = getattr(dep, "build_info", None)
        out_dir = getattr(build_info, "out_dir", None) if build_info else None
        if out_dir:
            lines.append(srcs_manifest_line(out_dir.path + "/", repo_relative(out_dir.short_path) + "/", True))
    manifest = ctx.actions.declare_file(ctx.label.name + ".srcs.tsv")
    ctx.actions.write(manifest, "".join(sorted(lines)))
    rustdoc_dir = ctx.actions.declare_directory(ctx.label.name + ".rustdoc")
    _rustdoc_json(ctx, crate_info, target[LintsInfo] if LintsInfo in target else None, rustdoc_dir)
    shard = ctx.actions.declare_file(ctx.label.name + ".ir.pb")
    args = ctx.actions.args()
    args.add("--json", rustdoc_dir.path + "/" + crate_info.name + ".json")
    args.add("--package", package)
    args.add("--crate", crate_info.name)
    args.add("--srcs-file", manifest)
    args.add("--out", shard)
    ctx.actions.run(
        executable = ctx.executable._extract,
        arguments = [args],
        inputs = [rustdoc_dir, manifest],
        outputs = [shard],
        mnemonic = "DxDocsRustExtract",
        progress_message = "Extracting Rust API docs for %{label}",
    )
    return struct(
        shard = shard,
        package = package,
        outputs = {"rustdoc_json": depset([rustdoc_dir]), "srcs_manifest": depset([manifest])},
    )

def _docs_rust_shard_impl(ctx):
    result = rust_shard_action(ctx, ctx.attr.crate, ctx.attr.package)
    return [
        DefaultInfo(files = depset([result.shard])),
        OutputGroupInfo(**result.outputs),
        DocsShardInfo(language = "rust", package = result.package, shard = result.shard),
    ]

docs_rust_shard = rule(
    doc = "Converts one crate's rustdoc JSON into a DocIr shard.",
    implementation = _docs_rust_shard_impl,
    attrs = {
        "crate": attr.label(
            doc = "The documented Rust library.",
            aspects = [_generated_srcs_aspect],
            mandatory = True,
            providers = [rust_common.crate_info],
        ),
        "crate_features": attr.string_list(doc = "Features enabled when documenting the crate."),
        "package": attr.string(doc = "Package segment of the symbol ids; defaults to the crate name."),
        "_error_format": attr.label(default = Label("@rules_rust//rust/settings:error_format")),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/rust/extract:rust_extract"),
            executable = True,
        ),
        "_process_wrapper": attr.label(
            default = Label("@rules_rust//util/process_wrapper"),
            cfg = "exec",
            executable = True,
        ),
    },
    fragments = ["cpp"],
    toolchains = [
        str(Label("@rules_rust//rust:toolchain_type")),
        "@bazel_tools//tools/cpp:toolchain_type",
    ],
    provides = [DocsShardInfo],
)

def _local_features(crate):
    label = crate
    if label.startswith("//") and ":" in label:
        pkg, target = label.split(":", 1)
        if pkg != "//" + native.package_name():
            fail("rust_doc_shard: crate {} is outside this package, so its crate_features cannot be read; pass crate_features explicitly (use [] for none)".format(crate))
        label = ":" + target
    elif not label.startswith(":"):
        fail("rust_doc_shard: crate {} is outside this package, so its crate_features cannot be read; pass crate_features explicitly (use [] for none)".format(crate))
    for name in [label[1:], label[1:] + "_upstream"]:
        rule = native.existing_rule(name)
        if rule and rule.get("crate_features"):
            return list(rule["crate_features"])
    return []

def rust_doc_shard(name, crate, package = "", crate_features = None, **kwargs):
    """Declares the DocIr shard of one crate built from stable rustdoc JSON."""
    docs_rust_shard(
        name = name,
        crate = crate,
        crate_features = _local_features(crate) if crate_features == None else crate_features,
        package = package,
        **kwargs
    )
