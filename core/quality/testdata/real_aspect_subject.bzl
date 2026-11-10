"""Analysis subject observing real-aspect dx_results."""

load("//core:canonical.bzl", "strip_canonical")
load("//core:defs.bzl", "DxSubjectInfo")
load(
    "//core/quality/testdata:all_languages.bzl",
    "RealShardProbeInfo",
    "real_ansible_lint_aspect",
    "real_avro_lint_aspect",
    "real_avro_typecheck_aspect",
    "real_bicep_format_aspect",
    "real_bicep_lint_aspect",
    "real_capnp_lint_aspect",
    "real_capnp_typecheck_aspect",
    "real_cc_format_aspect",
    "real_cc_lint_aspect",
    "real_cc_typecheck_aspect",
    "real_cc_format_probe_aspect",
    "real_cc_lint_probe_aspect",
    "real_cc_typecheck_probe_aspect",
    "real_cloudformation_lint_aspect",
    "real_cmake_format_aspect",
    "real_cmake_lint_aspect",
    "real_cue_format_aspect",
    "real_cue_typecheck_aspect",
    "real_core_format_probe_aspect",
    "real_core_lint_probe_aspect",
    "real_core_typecheck_probe_aspect",
    "real_d_typecheck_aspect",
    "real_docs_aspects",
    "real_dockerfile_lint_aspect",
    "real_oci_image_lint_aspect",
    "real_oci_image_typecheck_aspect",
    "real_dotnet_format_aspect",
    "real_dotnet_lint_aspect",
    "real_dotnet_typecheck_aspect",
    "real_flatbuffers_lint_aspect",
    "real_flatbuffers_typecheck_aspect",
    "real_format_aspect",
    "real_format_probe_aspect",
    "real_fortran_format_aspect",
    "real_fortran_lint_aspect",
    "real_fortran_typecheck_aspect",
    "real_go_format_aspect",
    "real_go_lint_aspect",
    "real_go_typecheck_aspect",
    "real_haskell_format_aspect",
    "real_haskell_lint_aspect",
    "real_haskell_typecheck_aspect",
    "real_latex_lint_aspect",
    "real_html_template_format_aspect",
    "real_html_template_lint_aspect",
    "real_js_format_aspect",
    "real_js_format_probe_aspect",
    "real_js_lint_probe_aspect",
    "real_js_typecheck_probe_aspect",
    "real_js_lint_aspect",
    "real_js_typecheck_aspect",
    "real_jsonnet_format_aspect",
    "real_jsonnet_lint_aspect",
    "real_jvm_format_aspect",
    "real_jvm_lint_aspect",
    "real_jvm_typecheck_aspect",
    "real_kubernetes_format_aspect",
    "real_kubernetes_lint_aspect",
    "real_latex_format_aspect",
    "real_lint_aspect",
    "real_lint_probe_aspect",
    "real_python_format_probe_aspect",
    "real_python_lint_probe_aspect",
    "real_python_typecheck_probe_aspect",
    "real_rust_format_probe_aspect",
    "real_rust_lint_probe_aspect",
    "real_rust_typecheck_probe_aspect",
    "real_lua_format_aspect",
    "real_lua_lint_aspect",
    "real_make_format_aspect",
    "real_make_lint_aspect",
    "real_nix_format_aspect",
    "real_nix_lint_aspect",
    "real_ocaml_format_aspect",
    "real_ocaml_lint_aspect",
    "real_ocaml_typecheck_aspect",
    "real_pkl_format_aspect",
    "real_pkl_lint_aspect",
    "real_pkl_typecheck_aspect",
    "real_powershell_format_aspect",
    "real_powershell_lint_aspect",
    "real_protobuf_format_aspect",
    "real_protobuf_lint_aspect",
    "real_protobuf_typecheck_aspect",
    "real_python_lint_aspect",
    "real_dart_format_aspect",
    "real_dart_lint_aspect",
    "real_dart_typecheck_aspect",
    "real_d_format_aspect",
    "real_d_lint_aspect",
    "real_perl_format_aspect",
    "real_perl_lint_aspect",
    "real_python_format_aspect",
    "real_python_typecheck_aspect",
    "real_julia_format_aspect",
    "real_swift_format_aspect",
    "real_swift_lint_aspect",
    "real_swift_typecheck_aspect",
    "real_zig_format_aspect",
    "real_zig_lint_aspect",
    "real_julia_lint_aspect",
    "real_rego_format_aspect",
    "real_rego_lint_aspect",
    "real_rego_typecheck_aspect",
    "real_erlang_format_aspect",
    "real_erlang_lint_aspect",
    "real_erlang_typecheck_aspect",
    "real_elixir_format_aspect",
    "real_elixir_lint_aspect",
    "real_php_lint_aspect",
    "real_php_typecheck_aspect",
    "real_julia_typecheck_aspect",
    "real_lua_typecheck_aspect",
    "real_ruby_typecheck_aspect",
    "real_ruby_format_aspect",
    "real_ruby_lint_aspect",
    "real_rust_format_aspect",
    "real_rust_lint_aspect",
    "real_scala_format_aspect",
    "real_scala_lint_aspect",
    "real_shard_result_stems",
    "real_shell_format_aspect",
    "real_shell_lint_aspect",
    "real_sql_format_aspect",
    "real_sql_lint_aspect",
    "real_terraform_format_aspect",
    "real_terraform_lint_aspect",
    "real_terraform_typecheck_aspect",
    "real_text_lint_aspect",
    "real_thrift_lint_aspect",
    "real_thrift_typecheck_aspect",
    "real_typecheck_aspect",
    "real_typecheck_probe_aspect",
    "real_typst_format_aspect",
    "real_typst_lint_aspect",
    "real_xml_lint_aspect",
    "real_yaml_format_aspect",
    "real_yaml_lint_aspect",
    "real_zig_typecheck_aspect",
)
load("//core/quality:families/cc.bzl", "CcShardProbeInfo")
load("//core/quality:families/core_probe.bzl", "CoreShardProbeInfo")
load("//javascript/private/quality:js.bzl", "JsShardProbeInfo")
load("//python/private/quality:python.bzl", "PythonShardProbeInfo")
load("//rust/private/quality:rust.bzl", "RustShardProbeInfo")
load("//core/quality:sources.bzl", "QualitySourcesInfo")

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
    real_fortran_format_aspect,
    real_fortran_lint_aspect,
    real_jsonnet_lint_aspect,
    real_typst_format_aspect,
    real_typst_lint_aspect,
    real_latex_format_aspect,
    real_lua_format_aspect,
    real_lua_lint_aspect,
    real_ocaml_lint_aspect,
    real_cmake_format_aspect,
    real_make_format_aspect,
    real_cmake_lint_aspect,
    real_haskell_format_aspect,
    real_haskell_lint_aspect,
    real_haskell_typecheck_aspect,
    real_latex_lint_aspect,
    real_pkl_format_aspect,
    real_pkl_lint_aspect,
    real_pkl_typecheck_aspect,
    real_terraform_typecheck_aspect,
    real_cc_lint_aspect,
    real_cc_typecheck_aspect,
    real_lint_aspect,
    real_nix_format_aspect,
    real_nix_lint_aspect,
    real_ocaml_format_aspect,
    real_ocaml_typecheck_aspect,
    real_kubernetes_format_aspect,
    real_kubernetes_lint_aspect,
    real_ansible_lint_aspect,
    real_bicep_format_aspect,
    real_bicep_lint_aspect,
    real_cloudformation_lint_aspect,
    real_sql_format_aspect,
    real_sql_lint_aspect,
    real_format_aspect,
    real_go_format_aspect,
    real_go_lint_aspect,
    real_go_typecheck_aspect,
    real_dotnet_format_aspect,
    real_dotnet_lint_aspect,
    real_dotnet_typecheck_aspect,
    real_dockerfile_lint_aspect,
    real_oci_image_lint_aspect,
    real_oci_image_typecheck_aspect,
    real_make_lint_aspect,
    real_capnp_lint_aspect,
    real_flatbuffers_lint_aspect,
    real_thrift_lint_aspect,
    real_avro_lint_aspect,
    real_js_lint_aspect,
    real_js_format_aspect,
    real_protobuf_format_aspect,
    real_powershell_format_aspect,
    real_powershell_lint_aspect,
    real_protobuf_lint_aspect,
    real_protobuf_typecheck_aspect,
    real_python_lint_aspect,
    real_dart_format_aspect,
    real_dart_lint_aspect,
    real_dart_typecheck_aspect,
    real_d_format_aspect,
    real_d_lint_aspect,
    real_perl_format_aspect,
    real_perl_lint_aspect,
    real_python_format_aspect,
    real_python_typecheck_aspect,
    real_julia_format_aspect,
    real_swift_format_aspect,
    real_swift_lint_aspect,
    real_swift_typecheck_aspect,
    real_zig_format_aspect,
    real_zig_lint_aspect,
    real_julia_lint_aspect,
    real_rego_format_aspect,
    real_rego_lint_aspect,
    real_js_typecheck_aspect,
    real_jvm_typecheck_aspect,
    real_d_typecheck_aspect,
    real_fortran_typecheck_aspect,
    real_zig_typecheck_aspect,
    real_rego_typecheck_aspect,
    real_capnp_typecheck_aspect,
    real_flatbuffers_typecheck_aspect,
    real_thrift_typecheck_aspect,
    real_avro_typecheck_aspect,
    real_typecheck_aspect,
    real_cue_typecheck_aspect,
    real_jvm_lint_aspect,
    real_jvm_format_aspect,
    real_erlang_format_aspect,
    real_erlang_lint_aspect,
    real_erlang_typecheck_aspect,
    real_elixir_format_aspect,
    real_elixir_lint_aspect,
    real_php_lint_aspect,
    real_php_typecheck_aspect,
    real_julia_typecheck_aspect,
    real_lua_typecheck_aspect,
    real_ruby_typecheck_aspect,
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
    real_xml_lint_aspect,
    real_yaml_format_aspect,
    real_yaml_lint_aspect,
]

def _subject_rule(aspects):
    return rule(
        implementation = _real_aspect_subject_impl,
        attrs = {
            "target": attr.label(
                aspects = aspects,
                mandatory = True,
            ),
        },
    )

real_aspect_subject = _subject_rule(_REAL_ASPECTS)

_CORE_ASPECTS = [real_lint_aspect, real_format_aspect, real_typecheck_aspect]

real_rust_aspect_subject = _subject_rule(_CORE_ASPECTS + [real_rust_lint_aspect, real_rust_format_aspect])

real_go_aspect_subject = _subject_rule(_CORE_ASPECTS + [real_go_lint_aspect, real_go_format_aspect, real_go_typecheck_aspect])

real_haskell_aspect_subject = _subject_rule(_CORE_ASPECTS + [real_haskell_lint_aspect, real_haskell_format_aspect, real_haskell_typecheck_aspect])

real_ocaml_aspect_subject = _subject_rule(_CORE_ASPECTS + [real_ocaml_lint_aspect, real_ocaml_format_aspect, real_ocaml_typecheck_aspect])

real_jvm_aspect_subject = _subject_rule(_CORE_ASPECTS + [real_jvm_lint_aspect, real_jvm_format_aspect, real_jvm_typecheck_aspect])

real_scala_aspect_subject = _subject_rule(_CORE_ASPECTS + [real_jvm_lint_aspect, real_jvm_format_aspect, real_jvm_typecheck_aspect, real_scala_format_aspect, real_scala_lint_aspect])

real_beam_aspect_subject = _subject_rule(_CORE_ASPECTS + [real_erlang_lint_aspect, real_erlang_format_aspect, real_erlang_typecheck_aspect, real_elixir_format_aspect, real_elixir_lint_aspect])

real_dotnet_aspect_subject = _subject_rule(_CORE_ASPECTS + [real_dotnet_lint_aspect, real_dotnet_format_aspect, real_dotnet_typecheck_aspect])

real_js_aspect_subject = _subject_rule(_CORE_ASPECTS + [real_js_lint_aspect, real_js_format_aspect, real_js_typecheck_aspect])

real_python_aspect_subject = _subject_rule(_CORE_ASPECTS + [real_python_lint_aspect, real_python_format_aspect, real_python_typecheck_aspect])

real_cc_aspect_subject = _subject_rule(_CORE_ASPECTS + [real_cc_lint_aspect, real_cc_format_aspect, real_cc_typecheck_aspect])

real_yaml_aspect_subject = _subject_rule(_CORE_ASPECTS + [real_yaml_lint_aspect, real_yaml_format_aspect])

real_sql_aspect_subject = _subject_rule(_CORE_ASPECTS + [real_sql_lint_aspect, real_sql_format_aspect])

real_core_aspect_subject = _subject_rule(_CORE_ASPECTS)

REAL_SUBJECT_BY_CLASS = {
    "asciidoc": real_core_aspect_subject,
    "c": real_cc_aspect_subject,
    "csharp": real_dotnet_aspect_subject,
    "cuda": real_cc_aspect_subject,
    "elixir": real_beam_aspect_subject,
    "erlang": real_beam_aspect_subject,
    "go": real_go_aspect_subject,
    "haskell": real_haskell_aspect_subject,
    "java": real_jvm_aspect_subject,
    "kotlin": real_jvm_aspect_subject,
    "objc": real_cc_aspect_subject,
    "ocaml": real_ocaml_aspect_subject,
    "python": real_python_aspect_subject,
    "rst": real_core_aspect_subject,
    "rust": real_rust_aspect_subject,
    "scala": real_scala_aspect_subject,
    "sql": real_sql_aspect_subject,
    "starlark": real_core_aspect_subject,
    "typescript": real_js_aspect_subject,
    "yaml": real_yaml_aspect_subject,
}

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

_FAMILY_PROBES = {
    "format": {
        "cc": real_cc_format_probe_aspect,
        "core": real_core_format_probe_aspect,
        "js": real_js_format_probe_aspect,
        "python": real_python_format_probe_aspect,
        "rust": real_rust_format_probe_aspect,
    },
    "lint": {
        "cc": real_cc_lint_probe_aspect,
        "core": real_core_lint_probe_aspect,
        "js": real_js_lint_probe_aspect,
        "python": real_python_lint_probe_aspect,
        "rust": real_rust_lint_probe_aspect,
    },
    "typecheck": {
        "cc": real_cc_typecheck_probe_aspect,
        "core": real_core_typecheck_probe_aspect,
        "js": real_js_typecheck_probe_aspect,
        "python": real_python_typecheck_probe_aspect,
        "rust": real_rust_typecheck_probe_aspect,
    },
}

_FAMILY_INFOS = {
    "cc": CcShardProbeInfo,
    "core": CoreShardProbeInfo,
    "js": JsShardProbeInfo,
    "python": PythonShardProbeInfo,
    "rust": RustShardProbeInfo,
}

_FACADE_PROBES = {
    "format": real_format_probe_aspect,
    "lint": real_lint_probe_aspect,
    "typecheck": real_typecheck_probe_aspect,
}

def _real_family_probe_subject_impl(ctx):
    fields = {"label": _label_text(ctx.attr.target.label)}
    for capability in ["lint", "format", "typecheck"]:
        facade = _probe_shards_of(getattr(ctx.attr, capability + "_facade"))
        united = {}
        for family, info in _FAMILY_INFOS.items():
            dep = getattr(ctx.attr, capability + "_" + family)
            if info in dep:
                for shard in dep[info].shards:
                    united[shard] = True
        families = sorted(united.keys())
        fields[capability + "_facade"] = _joined(facade)
        fields[capability + "_families"] = _joined(families)
        fields[capability + "_equal"] = str(families == list(facade))
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = fields),
    ]

def _family_probe_attrs():
    attrs = {"target": attr.label(mandatory = True)}
    for capability, aspect in _FACADE_PROBES.items():
        attrs[capability + "_facade"] = attr.label(aspects = [aspect], mandatory = True)
    for capability, probes in _FAMILY_PROBES.items():
        for family, aspect in probes.items():
            attrs[capability + "_" + family] = attr.label(aspects = [aspect], mandatory = True)
    return attrs

_real_family_probe_subject = rule(
    implementation = _real_family_probe_subject_impl,
    doc = "Compares the per-family shard probes with the facade probe for each capability.",
    attrs = _family_probe_attrs(),
)

def real_family_probe_subject(name, target, **kwargs):
    """Observes, per capability, the family probes of `target` next to the facade probe."""
    attrs = {capability + "_facade": target for capability in _FACADE_PROBES}
    for capability, probes in _FAMILY_PROBES.items():
        for family in probes:
            attrs[capability + "_" + family] = target
    _real_family_probe_subject(name = name, target = target, **(attrs | kwargs))

def _real_docs_subject_impl(ctx):
    target = ctx.attr.target
    shards = []
    if OutputGroupInfo in target and "dx_docs_shards" in target[OutputGroupInfo]:
        shards = sorted([f.short_path for f in target[OutputGroupInfo]["dx_docs_shards"].to_list()])
    results = []
    if OutputGroupInfo in target and "dx_results" in target[OutputGroupInfo]:
        results = sorted([f.basename for f in target[OutputGroupInfo]["dx_results"].to_list()])
    sites = []
    if OutputGroupInfo in target and "dx_docs_sites" in target[OutputGroupInfo]:
        sites = sorted([f.basename for f in target[OutputGroupInfo]["dx_docs_sites"].to_list()])
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = {
            "docs_shard_count": str(len(shards)),
            "docs_shards": ",".join(shards) if shards else "(none)",
            "dx_docs_sites": ",".join(sites) if sites else "(none)",
            "dx_results": ",".join(results) if results else "(none)",
            "label": _label_text(target.label),
        }),
    ]

real_docs_subject = rule(
    implementation = _real_docs_subject_impl,
    doc = "Observes the docs shards that the docs aspects build for a target.",
    attrs = {
        "target": attr.label(aspects = real_docs_aspects, mandatory = True),
    },
)

def _real_docs_shard_files_impl(ctx):
    return [DefaultInfo(files = ctx.attr.target[OutputGroupInfo][ctx.attr.group])]

real_docs_shard_files = rule(
    implementation = _real_docs_shard_files_impl,
    doc = "Exposes one output group that the docs aspects build for a target.",
    attrs = {
        "group": attr.string(default = "dx_docs_shards"),
        "target": attr.label(aspects = real_docs_aspects, mandatory = True),
    },
)
