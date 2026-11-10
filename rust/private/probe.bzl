"""Conformance subject for the minimal Rust wrappers."""

load("@rules_cc//cc/common:cc_info.bzl", "CcInfo")
load("@rules_rust//rust:defs.bzl", _rust_common = "rust_common")
load("//core:canonical.bzl", "strip_canonical")
load("//core:defs.bzl", "DxSubjectInfo")
load("//core/quality:sources.bzl", "QualitySourcesInfo")
load("//rust/private/toolchains:bindings.bzl", "RUST_TOOLCHAIN_TYPE")

def _label_text(label):
    return strip_canonical(str(label))

def _sorted_basenames(files):
    return sorted([f.basename for f in files])

def _render_direct_sources(info):
    parts = []
    for class_id in sorted(info.direct_sources.keys()):
        parts.append(class_id + ":" + ",".join(_sorted_basenames(info.direct_sources[class_id].to_list())))
    if len(parts) == 0:
        return "(none)"
    return ";".join(parts)

def _toolchain_file(toolchain, name):
    info = getattr(toolchain, name, None)
    if info == None:
        return "(none)"
    return _without_executable_suffix(info.basename)

def _without_executable_suffix(name):
    """Drops the suffix the host runs a tool under."""
    for suffix in [".exe", ".bat", ".cmd", ".com"]:
        if name.endswith(suffix) and len(name) > len(suffix):
            return name[:-len(suffix)]
    return name

def _wrapper_fields(ctx, crate, ucrate):
    wrapper = ctx.attr.wrapper
    upstream = ctx.attr.upstream
    crate_srcs = sorted([f.path for f in crate.srcs.to_list()])
    ucrate_srcs = sorted([f.path for f in ucrate.srcs.to_list()])

    toolchain = ctx.toolchains[RUST_TOOLCHAIN_TYPE]
    tool_paths = [getattr(toolchain, name, None) for name in ["rustc", "cargo", "rustfmt", "clippy_driver"]]
    pinned = [t != None and "rules_rust" in t.path for t in tool_paths]

    return {
        "cargo_tool": _toolchain_file(toolchain, "cargo"),
        "clippy_tool": _toolchain_file(toolchain, "clippy_driver"),
        "crate_edition": crate.edition,
        "crate_is_test": str(crate.is_test),
        "crate_name": crate.name,
        "crate_owner": _label_text(crate.owner),
        "crate_root": crate.root.basename,
        "crate_srcs": ",".join(_sorted_basenames(crate.srcs.to_list())),
        "crate_type": crate.type,
        "direct_sources": _render_direct_sources(wrapper[QualitySourcesInfo]),
        "preserved_deps": str(len(crate.deps.to_list()) == len(ucrate.deps.to_list())),
        "preserved_edition": str(crate.edition == ucrate.edition),
        "preserved_name": str(crate.name == ucrate.name),
        "preserved_root": str(crate.root.path == ucrate.root.path),
        "preserved_srcs": str(crate_srcs == ucrate_srcs),
        "preserved_type": str(crate.type == ucrate.type),
        "rustc_tool": _toolchain_file(toolchain, "rustc"),
        "rustfmt_tool": _toolchain_file(toolchain, "rustfmt"),
        "tools_pinned": str(len(pinned) == 4 and all(pinned)),
        "upstream": _label_text(upstream.label),
        "upstream_has_instrumented_files": str(InstrumentedFilesInfo in upstream),
        "upstream_has_quality_sources": str(QualitySourcesInfo in upstream),
        "wrapper": _label_text(wrapper.label),
        "wrapper_has_instrumented_files": str(InstrumentedFilesInfo in wrapper),
        "wrapper_has_quality_sources": str(QualitySourcesInfo in wrapper),
    }

def _subject_providers(ctx, fields):
    out = ctx.actions.declare_file(ctx.label.name + ".txt")
    ctx.actions.write(out, "\n".join([k + "=" + fields[k] for k in sorted(fields.keys())]) + "\n")
    return [
        DefaultInfo(files = depset([out])),
        DxSubjectInfo(fields = fields),
    ]

def _wrapper_subject_impl(ctx):
    crate = ctx.attr.wrapper[_rust_common.crate_info]
    ucrate = ctx.attr.upstream[_rust_common.crate_info]
    return _subject_providers(ctx, _wrapper_fields(ctx, crate, ucrate))

dx_wrapper_subject = rule(
    implementation = _wrapper_subject_impl,
    doc = "Analysis subject comparing a Rust wrapper with its upstream crate.",
    attrs = {
        "upstream": attr.label(
            doc = "Upstream crate target the wrapper is compared against.",
            mandatory = True,
        ),
        "wrapper": attr.label(
            doc = "Rust wrapper target whose crate is compared with upstream.",
            mandatory = True,
        ),
    },
    toolchains = [RUST_TOOLCHAIN_TYPE],
)

def _wrapper_cc_subject_impl(ctx):
    wrapper = ctx.attr.wrapper
    upstream = ctx.attr.upstream
    crate = wrapper[_rust_common.test_crate_info].crate
    ucrate = upstream[_rust_common.test_crate_info].crate
    cc_inputs = len(wrapper[CcInfo].linking_context.linker_inputs.to_list())
    ucc_inputs = len(upstream[CcInfo].linking_context.linker_inputs.to_list())
    fields = _wrapper_fields(ctx, crate, ucrate)
    fields["cc_linker_inputs"] = str(cc_inputs)
    fields["preserved_cc_inputs"] = str(cc_inputs == ucc_inputs)
    return _subject_providers(ctx, fields)

dx_wrapper_cc_subject = rule(
    implementation = _wrapper_cc_subject_impl,
    doc = "Analysis subject comparing a Rust test wrapper with its upstream crate.",
    attrs = {
        "upstream": attr.label(
            doc = "Upstream test crate target the wrapper is compared against.",
            mandatory = True,
            providers = [[_rust_common.test_crate_info]],
        ),
        "wrapper": attr.label(
            doc = "Rust test wrapper target whose crate is compared with upstream.",
            mandatory = True,
            providers = [[_rust_common.test_crate_info]],
        ),
    },
    toolchains = [RUST_TOOLCHAIN_TYPE],
)
