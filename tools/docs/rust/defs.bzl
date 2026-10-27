"""Rust API reference shards built from pinned nightly rustdoc JSON."""

load("@rules_rust//rust:defs.bzl", "rust_common", "rust_doc")
load("//tools/docs/ir:shard.bzl", "DocsShardInfo", _repo_relative = "repo_relative", _srcs_manifest_line = "srcs_manifest_line")

_CHANNEL = str(Label("@rules_rust//rust/toolchain/channel"))

_RUSTDOC_FLAGS = ["-Zunstable-options", "--output-format=json"]

def _nightly_transition_impl(_settings, _attr):
    return {_CHANNEL: "nightly"}

_nightly_transition = transition(
    implementation = _nightly_transition_impl,
    inputs = [],
    outputs = [_CHANNEL],
)

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

def _single(value):
    return value[0] if type(value) == "list" else value

def _docs_rust_shard_impl(ctx):
    doc = _single(ctx.attr.doc)
    crate = _single(ctx.attr.crate)
    crate_info = crate[rust_common.crate_info]
    package = ctx.attr.package or crate_info.name
    generated = {path: True for path in crate[_GeneratedSrcsInfo].paths}
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
    rustdoc_dir = doc[OutputGroupInfo].rustdoc_dir.to_list()[0]
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
    return [
        DefaultInfo(files = depset([shard])),
        OutputGroupInfo(rustdoc_json = depset([rustdoc_dir]), srcs_manifest = depset([manifest])),
        DocsShardInfo(language = "rust", package = package, shard = shard),
    ]

docs_rust_shard = rule(
    doc = "Converts one crate's nightly rustdoc JSON into a DocIr shard.",
    implementation = _docs_rust_shard_impl,
    attrs = {
        "crate": attr.label(
            doc = "The documented Rust library.",
            aspects = [_generated_srcs_aspect],
            cfg = _nightly_transition,
            mandatory = True,
            providers = [rust_common.crate_info],
        ),
        "doc": attr.label(
            doc = "The crate's JSON rust_doc target.",
            cfg = _nightly_transition,
            mandatory = True,
        ),
        "package": attr.string(doc = "Package segment of the symbol ids; defaults to the crate name."),
        "_allowlist_function_transition": attr.label(
            default = Label("@bazel_tools//tools/allowlists/function_transition_allowlist"),
        ),
        "_extract": attr.label(
            cfg = "exec",
            default = Label("//tools/docs/rust/extract:rust_extract"),
            executable = True,
        ),
    },
    provides = [DocsShardInfo],
)

def _local_features(crate):
    if not crate.startswith(":"):
        return []
    for name in [crate[1:], crate[1:] + "_upstream"]:
        rule = native.existing_rule(name)
        if rule and rule.get("crate_features"):
            return list(rule["crate_features"])
    return []

def rust_doc_shard(name, crate, package = "", crate_features = None, **kwargs):
    """Declares a manual JSON rust_doc of one crate and its DocIr shard."""
    rust_doc(
        name = name + "_rustdoc",
        crate = crate,
        crate_features = _local_features(crate) if crate_features == None else crate_features,
        rustdoc_flags = _RUSTDOC_FLAGS,
        tags = ["manual"],
        visibility = ["//visibility:private"],
    )
    docs_rust_shard(
        name = name,
        crate = crate,
        doc = ":" + name + "_rustdoc",
        package = package,
        **kwargs
    )
