"""Rust real quality aspects."""

load("@rules_rust//rust:defs.bzl", "rust_clippy_aspect", _rust_common = "rust_common")
load("//quality:family_hooks.bzl", "upstream_files")
load("//quality:pipeline.bzl", "generated_source_paths", "prune_tool_generated_sources")
load("//quality:real_core.bzl", "real_config", "real_context", "real_make_aspect")
load("//rust/rules:edition.bzl", "RUST_EDITION")
load("//rust/toolchains:bindings.bzl", "RUST_TOOLCHAIN_TYPE", "rust_toolchain_rustc", "rust_toolchain_toolchains", "rust_toolchain_tools")

TOOLS = {
    "clippy": {
        "kind": "toolchain",
        "resources": "heavy",
        "shard": "rust",
        "toolchain_file": "clippy_driver",
    },
    "rustc": {
        "kind": "toolchain",
        "resources": "heavy",
        "shard": "rust",
        "toolchain_file": "rustc",
    },
    "rustfmt": {
        "kind": "toolchain",
        "resources": "light",
        "shard": "rust",
        "toolchain_file": "rustfmt",
    },
}

SHARD_SPECS = {
    "real_rust_format": {
        "capability": "format",
        "doc": "Additive Rust format family aspect (toolchain rustfmt).",
        "shard": "rust",
        "suffix": "-rust",
        "toolchains": rust_toolchain_toolchains(),
    },
    "real_rust_lint": {
        "capability": "lint",
        "doc": "Additive Rust lint family aspect (delegated Clippy).",
        "requires": [rust_clippy_aspect],
        "shard": "rust",
        "suffix": "-rust",
        "toolchains": rust_toolchain_toolchains(),
    },
    "real_rust_typecheck": {
        "capability": "typecheck",
        "doc": "Additive Rust typecheck family aspect (delegated rustc).",
        "shard": "rust",
        "suffix": "-rust",
        "toolchains": rust_toolchain_toolchains(),
    },
}

def rustc_diagnostics_group(groups):
    """Returns the rustc diagnostics output group to read, preferring the metadata build."""
    for name in ["rustc_rmeta_output", "rustc_output"]:
        if name in groups:
            return name
    return None

def inherits_test_crate_sources(capability):
    """Whether a Rust test crate's inherited sources feed this capability's diagnostics."""
    return capability == "typecheck"

def merge_inherited_sources(direct_sources, inherited, class_id):
    """Adds inherited in-workspace .rs sources to a copy of direct_sources."""
    merged = {k: v.to_list() for k, v in direct_sources.items()}
    seen = {}
    for files in merged.values():
        for f in files:
            seen[f.path] = True
    added = []
    for f in inherited:
        if not f.is_source or f.extension != "rs" or f.owner.repo_name != "":
            continue
        if f.path in seen:
            continue
        seen[f.path] = True
        added.append(f)
    if added:
        merged[class_id] = merged.get(class_id, []) + added
    return {k: depset(v) for k, v in merged.items()}

def _rustfmt_hook(target, _ctx, resolved, _capability, what, shared):
    if not shared.has_toolchain:
        fail(what + ": rustfmt needs the Rust family aspect")
    if _rust_common.crate_info in target:
        edition = target[_rust_common.crate_info].edition
    elif _rust_common.test_crate_info in target:
        edition = target[_rust_common.test_crate_info].crate.edition
    else:
        edition = RUST_EDITION
    generated = generated_source_paths(shared.direct_files)
    pruned = None
    if len(generated) > 0:
        pruned = prune_tool_generated_sources(resolved, generated, "rustfmt")
    return real_context(resolved = pruned, options = {"edition": edition})

def _crate_deps_files(target, ctx, tool):
    """Writes the crate name to dependency target map that places unused-dependency findings."""
    if ctx.label.workspace_name or _rust_common.crate_info not in target:
        return []
    deps = {}
    for attr_name in ["deps", "proc_macro_deps"]:
        for dep in getattr(ctx.rule.attr, attr_name, []):
            if _rust_common.crate_info in dep:
                deps[dep[_rust_common.crate_info].name] = dep.label.name
    if not deps:
        return []
    line = json.encode({
        "$message_type": "dx_crate_deps",
        "crate": target[_rust_common.crate_info].name,
        "deps": deps,
        "package": ctx.label.package,
        "target": ctx.label.name,
    })
    out = ctx.actions.declare_file(ctx.label.name + "." + tool + ".dx_crate_deps.json")
    ctx.actions.write(out, line + "\n")
    return [out]

def _clippy_hook(target, ctx, _resolved, _capability, what, shared):
    if not shared.has_toolchain:
        fail(what + ": clippy needs the Rust family aspect")
    diagnostics = []
    if OutputGroupInfo in target and "clippy_output" in target[OutputGroupInfo]:
        diagnostics = target[OutputGroupInfo]["clippy_output"].to_list()
        diagnostics = diagnostics + _crate_deps_files(target, ctx, "clippy")
    return real_context(upstream = diagnostics, late_inputs = diagnostics, delegated = len(diagnostics) > 0)

def _rustc_hook(target, ctx, _resolved, _capability, what, shared):
    if not shared.has_toolchain:
        fail(what + ": rustc needs the Rust family aspect")
    diagnostics = []
    if OutputGroupInfo in target:
        group = rustc_diagnostics_group(target[OutputGroupInfo])
        if group:
            diagnostics = target[OutputGroupInfo][group].to_list() + _crate_deps_files(target, ctx, "rustc")
    return real_context(upstream = diagnostics, late_inputs = diagnostics, delegated = len(diagnostics) > 0)

def _rust_sources(target, ctx):
    if _rust_common.crate_info not in target and _rust_common.test_crate_info not in target:
        return None
    return upstream_files(getattr(ctx.rule.files, "srcs", None), "rust")

def harness_sources(target, capability, direct_sources):
    """Adds the sources of a Rust test crate's harness to a typecheck's direct sources."""
    if not inherits_test_crate_sources(capability):
        return direct_sources
    harness = None
    if _rust_common.test_crate_info in target:
        harness = target[_rust_common.test_crate_info].crate
    elif _rust_common.crate_info in target and target[_rust_common.crate_info].is_test:
        harness = target[_rust_common.crate_info]
    if harness == None:
        return direct_sources
    return merge_inherited_sources(direct_sources, harness.srcs.to_list(), "rust")

def _toolchain_file(ctx, name):
    """Returns one tool file from the resolved Rust toolchains."""
    if name == "rustc":
        return rust_toolchain_rustc(ctx)
    clippy_driver, rustfmt = rust_toolchain_tools(ctx)
    return clippy_driver if name == "clippy_driver" else rustfmt

HOOK_ORDER = ["rustfmt", "clippy", "rustc"]

HOOKS = {
    "rustfmt": _rustfmt_hook,
    "clippy": _clippy_hook,
    "rustc": _rustc_hook,
}

RECOGNIZER = _rust_sources

TOOLCHAIN = struct(type = RUST_TOOLCHAIN_TYPE, file = _toolchain_file)

_CFG = real_config(
    tools = TOOLS,
    shard_specs = SHARD_SPECS,
    hook_order = HOOK_ORDER,
    context_hooks = HOOKS,
    source_recognizers = [RECOGNIZER],
    harness_sources = harness_sources,
    toolchain = TOOLCHAIN,
    check_policy = False,
)

real_rust_format_aspect = real_make_aspect(_CFG, "real_rust_format")

real_rust_lint_aspect = real_make_aspect(_CFG, "real_rust_lint")

real_rust_typecheck_aspect = real_make_aspect(_CFG, "real_rust_typecheck")
