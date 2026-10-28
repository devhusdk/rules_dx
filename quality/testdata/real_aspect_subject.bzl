"""Analysis subject observing real-aspect dx_results."""

load("//libs/starlark:canonical.bzl", "strip_canonical")
load("//libs/starlark:defs.bzl", "DxSubjectInfo")
load(
    "//quality:real_aspects.bzl",
    "RealShardProbeInfo",
    "real_ansible_lint_aspect",
    "real_cc_format_aspect",
    "real_cc_lint_aspect",
    "real_cc_typecheck_aspect",
    "real_cloudformation_lint_aspect",
    "real_cmake_format_aspect",
    "real_make_format_aspect",
    "real_cmake_lint_aspect",
    "real_cue_format_aspect",
    "real_cue_typecheck_aspect",
    "real_dockerfile_lint_aspect",
    "real_dotnet_format_aspect",
    "real_dotnet_lint_aspect",
    "real_dotnet_typecheck_aspect",
    "real_flatbuffers_lint_aspect",
    "real_flatbuffers_typecheck_aspect",
    "real_format_aspect",
    "real_format_probe_aspect",
    "real_fortran_lint_aspect",
    "real_go_format_aspect",
    "real_go_typecheck_aspect",
    "real_haskell_format_aspect",
    "real_haskell_lint_aspect",
    "real_html_template_format_aspect",
    "real_html_template_lint_aspect",
    "real_js_format_aspect",
    "real_js_lint_aspect",
    "real_js_typecheck_aspect",
    "real_jsonnet_format_aspect",
    "real_jsonnet_lint_aspect",
    "real_jvm_format_aspect",
    "real_jvm_lint_aspect",
    "real_jvm_typecheck_aspect",
    "real_kubernetes_lint_aspect",
    "real_latex_format_aspect",
    "real_lint_aspect",
    "real_lint_probe_aspect",
    "real_lua_format_aspect",
    "real_make_lint_aspect",
    "real_nix_format_aspect",
    "real_nix_lint_aspect",
    "real_pkl_format_aspect",
    "real_powershell_format_aspect",
    "real_powershell_lint_aspect",
    "real_protobuf_format_aspect",
    "real_protobuf_lint_aspect",
    "real_python_lint_aspect",
    "real_rego_format_aspect",
    "real_rego_lint_aspect",
    "real_rego_typecheck_aspect",
    "real_ruby_format_aspect",
    "real_ruby_lint_aspect",
    "real_rust_format_aspect",
    "real_rust_lint_aspect",
    "real_scala_format_aspect",
    "real_scala_lint_aspect",
    "real_shard_result_stems",
    "real_shell_format_aspect",
    "real_shell_lint_aspect",
    "real_sql_lint_aspect",
    "real_terraform_format_aspect",
    "real_terraform_lint_aspect",
    "real_text_lint_aspect",
    "real_typecheck_probe_aspect",
    "real_typst_format_aspect",
    "real_yaml_format_aspect",
    "real_yaml_lint_aspect",
)
load("//quality:sources.bzl", "QualitySourcesInfo")

def _label_text(label):
    return strip_canonical(str(label))

def _real_aspect_subject_impl(ctx):
    target = ctx.attr.target
    dx_files = []
    if OutputGroupInfo in target:
        groups = target[OutputGroupInfo]
        if "dx_results" in groups:
            dx_files = sorted(
                groups["dx_results"].to_list(),
                key = lambda f: f.basename,
            )
    basenames = [f.basename for f in dx_files]
    return [
        DefaultInfo(files = depset([])),
        OutputGroupInfo(dx_results = depset(dx_files)),
        DxSubjectInfo(fields = {
            "dx_count": str(len(basenames)),
            "dx_results": ",".join(basenames) if len(basenames) > 0 else "(none)",
            "has_quality_sources": str(QualitySourcesInfo in target),
            "label": _label_text(ctx.attr.target.label),
        }),
    ]

_REAL_ASPECTS = [
    real_cc_format_aspect,
    real_cue_format_aspect,
    real_jsonnet_format_aspect,
    real_fortran_lint_aspect,
    real_jsonnet_lint_aspect,
    real_typst_format_aspect,
    real_latex_format_aspect,
    real_lua_format_aspect,
    real_cmake_format_aspect,
    real_make_format_aspect,
    real_cmake_lint_aspect,
    real_haskell_format_aspect,
    real_haskell_lint_aspect,
    real_pkl_format_aspect,
    real_cc_lint_aspect,
    real_cc_typecheck_aspect,
    real_lint_aspect,
    real_nix_format_aspect,
    real_nix_lint_aspect,
    real_kubernetes_lint_aspect,
    real_ansible_lint_aspect,
    real_cloudformation_lint_aspect,
    real_sql_lint_aspect,
    real_format_aspect,
    real_go_format_aspect,
    real_go_typecheck_aspect,
    real_dotnet_format_aspect,
    real_dotnet_lint_aspect,
    real_dotnet_typecheck_aspect,
    real_dockerfile_lint_aspect,
    real_make_lint_aspect,
    real_flatbuffers_lint_aspect,
    real_js_lint_aspect,
    real_js_format_aspect,
    real_protobuf_format_aspect,
    real_powershell_format_aspect,
    real_powershell_lint_aspect,
    real_protobuf_lint_aspect,
    real_python_lint_aspect,
    real_rego_format_aspect,
    real_rego_lint_aspect,
    real_js_typecheck_aspect,
    real_jvm_typecheck_aspect,
    real_rego_typecheck_aspect,
    real_flatbuffers_typecheck_aspect,
    real_cue_typecheck_aspect,
    real_jvm_lint_aspect,
    real_jvm_format_aspect,
    real_ruby_lint_aspect,
    real_ruby_format_aspect,
    real_rust_lint_aspect,
    real_rust_format_aspect,
    real_scala_format_aspect,
    real_scala_lint_aspect,
    real_shell_format_aspect,
    real_shell_lint_aspect,
    real_terraform_format_aspect,
    real_terraform_lint_aspect,
    real_text_lint_aspect,
    real_html_template_format_aspect,
    real_html_template_lint_aspect,
    real_yaml_format_aspect,
    real_yaml_lint_aspect,
]

real_aspect_subject = rule(
    implementation = _real_aspect_subject_impl,
    attrs = {
        "target": attr.label(
            aspects = _REAL_ASPECTS,
            mandatory = True,
        ),
    },
)

def _probe_shards_of(target):
    if RealShardProbeInfo in target:
        return target[RealShardProbeInfo].shards
    return []

def _joined(names):
    return ",".join(names) if len(names) > 0 else "(none)"

def _real_shard_probe_subject_impl(ctx):
    probed = {}
    fields = {"label": _label_text(ctx.attr.target.label)}
    for capability, dep in [("lint", ctx.attr.lint_probe), ("format", ctx.attr.format_probe), ("typecheck", ctx.attr.typecheck_probe)]:
        shards = _probe_shards_of(dep)
        fields[capability + "_shards"] = _joined(shards)
        for shard in shards:
            probed[shard] = True
    stems = real_shard_result_stems()
    needed = []
    target = ctx.attr.target
    if OutputGroupInfo in target and "dx_results" in target[OutputGroupInfo]:
        for f in target[OutputGroupInfo]["dx_results"].to_list():
            stem = f.basename.removesuffix(".pb")
            stem = stem[stem.rfind("-real-") + 1:]
            needed.append(stems.get(stem, "unknown:" + stem))
    fields["missing"] = _joined(sorted([shard for shard in needed if shard not in probed]))
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = fields),
    ]

real_shard_probe_subject = rule(
    implementation = _real_shard_probe_subject_impl,
    doc = "Observes the shard probe per capability and the real shards it would have missed.",
    attrs = {
        "format_probe": attr.label(aspects = [real_format_probe_aspect], mandatory = True),
        "lint_probe": attr.label(aspects = [real_lint_probe_aspect], mandatory = True),
        "target": attr.label(aspects = _REAL_ASPECTS, mandatory = True),
        "typecheck_probe": attr.label(aspects = [real_typecheck_probe_aspect], mandatory = True),
    },
)
