"""Rust API reference shard actions built from stable rustdoc JSON."""

load("@rules_rust//rust:defs.bzl", "rust_common")
load("@rules_rust//rust/private:providers.bzl", "LintsInfo")
load("@rules_rust//rust/private:rustdoc.bzl", "rustdoc_compile_action")
load("@rules_rust//rust/private:utils.bzl", "find_toolchain")
load("//tools/docs/ir:shard.bzl", _repo_relative = "repo_relative", _srcs_manifest_line = "srcs_manifest_line")

_RUSTDOC_FLAGS = ["-Zunstable-options", "--output-format=json"]

RustDocInputsInfo = provider(
    doc = "Inputs of one crate that rustdoc needs but an aspect context cannot read.",
    fields = {
        "crate_features": "Features enabled on the crate.",
        "generated": "Repo-relative paths of the generated srcs.",
    },
)

repo_relative = _repo_relative

srcs_manifest_line = _srcs_manifest_line

def _rust_doc_inputs_aspect_impl(_target, ctx):
    upstream = getattr(ctx.rule.attr, "upstream", None)
    if type(upstream) == "list":
        upstream = upstream[0] if upstream else None
    if upstream and RustDocInputsInfo in upstream:
        return [upstream[RustDocInputsInfo]]
    srcs = getattr(ctx.rule.files, "srcs", [])
    return [RustDocInputsInfo(
        generated = [repo_relative(f.short_path) for f in srcs if not f.is_source],
        crate_features = list(getattr(ctx.rule.attr, "crate_features", [])),
    )]

rust_doc_inputs_aspect = aspect(
    implementation = _rust_doc_inputs_aspect_impl,
    attr_aspects = ["upstream"],
    doc = "Collects the crate features and generated srcs of a Rust library or its upstream.",
)

def _rule_like(ctx):
    """Returns a context whose attr has the `name` that rules_rust reads from a rule."""
    if hasattr(ctx.attr, "name"):
        return ctx
    attrs = {key: getattr(ctx.attr, key) for key in dir(ctx.attr)}
    attrs["name"] = ctx.label.name
    return struct(
        actions = ctx.actions,
        attr = struct(**attrs),
        bin_dir = ctx.bin_dir,
        configuration = ctx.configuration,
        coverage_instrumented = ctx.coverage_instrumented,
        disabled_features = ctx.disabled_features,
        executable = ctx.executable,
        features = ctx.features,
        genfiles_dir = ctx.genfiles_dir,
        file = ctx.file,
        files = ctx.files,
        fragments = ctx.fragments,
        label = ctx.label,
        toolchains = ctx.toolchains,
        var = ctx.var,
        workspace_name = ctx.workspace_name,
    )

def _rustdoc_json(ctx, crate_info, lints_info, output_dir, features):
    flags = ctx.actions.args()
    flags.add("--extern", "{}={}".format(crate_info.name, crate_info.output.path))
    flags.add_all(_RUSTDOC_FLAGS)
    flags.add_all(['--cfg=feature="{}"'.format(f) for f in features])
    action = rustdoc_compile_action(
        ctx = _rule_like(ctx),
        toolchain = find_toolchain(ctx),
        crate_info = crate_info,
        lints_info = lints_info,
        output = output_dir,
        rustdoc_flags = flags,
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

def rust_shard_action(ctx, target, package, stem = None):
    """Runs stable rustdoc JSON and the extractor for one Rust crate target."""
    crate_info = target[rust_common.crate_info]
    package = package or crate_info.name
    stem = stem or ctx.label.name
    inputs = target[RustDocInputsInfo]
    generated = {path: True for path in inputs.generated}
    features = [] if hasattr(ctx.attr, "crate_features") else inputs.crate_features
    lines = []
    for src in crate_info.srcs.to_list():
        rel = repo_relative(src.short_path)
        lines.append(srcs_manifest_line(src.path, rel, rel in generated))
    for dep in crate_info.deps.to_list():
        build_info = getattr(dep, "build_info", None)
        out_dir = getattr(build_info, "out_dir", None) if build_info else None
        if out_dir:
            lines.append(srcs_manifest_line(out_dir.path + "/", repo_relative(out_dir.short_path) + "/", True))
    manifest = ctx.actions.declare_file(stem + ".srcs.tsv")
    ctx.actions.write(manifest, "".join(sorted(lines)))
    rustdoc_dir = ctx.actions.declare_directory(stem + ".rustdoc")
    _rustdoc_json(ctx, crate_info, target[LintsInfo] if LintsInfo in target else None, rustdoc_dir, features)
    shard = ctx.actions.declare_file(stem + ".ir.pb")
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
