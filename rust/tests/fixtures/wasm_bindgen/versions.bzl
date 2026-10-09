"""Pinned versions and output observations for the wasm fixture."""

load("@rules_rust//rust:defs.bzl", "rust_common")
load("@rules_rust_wasm_bindgen//:providers.bzl", "RustWasmBindgenInfo")
load("//libs/starlark:defs.bzl", "DxSubjectInfo", "starlark_test")

WASM_BINDGEN_VERSION = "0.2.121"

WASM_BINDGEN_TEST_VERSION = "0.3.71"

WASM_PLATFORM = "@rules_rust//rust/platform:wasm32"

WASM_TOOLCHAIN_TYPE = Label("@rules_rust_wasm_bindgen//:toolchain_type")

def _wasm_transition_impl(_settings, _attr):
    """Selects the wasm32 platform for one dependency edge."""
    return {"//command_line_option:platforms": [WASM_PLATFORM]}

wasm_transition = transition(
    implementation = _wasm_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _wasm_file_impl(ctx):
    """Exposes the wasm32 output files of one label."""
    inner = ctx.attr.target[0] if type(ctx.attr.target) == "list" else ctx.attr.target
    return [DefaultInfo(files = inner[DefaultInfo].files)]

wasm_file = rule(
    implementation = _wasm_file_impl,
    attrs = {
        "target": attr.label(
            allow_files = True,
            cfg = wasm_transition,
            mandatory = True,
        ),
    },
)

def _crate_root_owner(lib):
    """Returns the owner of the crate root behind one library target."""
    if rust_common.crate_info in lib:
        return lib[rust_common.crate_info].root.owner
    if rust_common.test_crate_info in lib:
        return lib[rust_common.test_crate_info].crate.root.owner
    fail("wasm fixture: no crate info on " + str(lib.label))

def _bindgen_subject_impl(ctx):
    """Exposes binding outputs and the resolved CLI and lib identities."""
    info = ctx.attr.target[RustWasmBindgenInfo]
    toolchain = ctx.toolchains[WASM_TOOLCHAIN_TYPE]
    js = sorted([f.basename for f in info.js.to_list()])
    ts = sorted([f.basename for f in info.ts.to_list()])
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = {
            "bindgen.js": ",".join(js) if len(js) > 0 else "(none)",
            "bindgen.ts": ",".join(ts) if len(ts) > 0 else "(none)",
            "bindgen.wasm": info.wasm.basename,
            "cli.owner": str(toolchain.wasm_bindgen_cli.owner),
            "dep.label": str(ctx.attr.wasm_dep.label),
            "lib.owner": str(_crate_root_owner(ctx.attr.wasm_dep)),
        }),
    ]

bindgen_subject = rule(
    implementation = _bindgen_subject_impl,
    attrs = {
        "target": attr.label(
            mandatory = True,
            providers = [RustWasmBindgenInfo],
        ),
        "wasm_dep": attr.label(
            default = "@rules_rust_wasm_bindgen//3rdparty:wasm_bindgen",
        ),
    },
    toolchains = [WASM_TOOLCHAIN_TYPE],
)

def _bindgen_target_check_impl(ctx):
    """Rejects an unsupported binding target with an actionable diagnostic."""
    if ctx.attr.target not in ctx.attr.supported:
        fail("unsupported wasm-bindgen target '" + ctx.attr.target + "': want one of " + ", ".join(sorted(ctx.attr.supported)))
    return [DefaultInfo(files = depset([]))]

bindgen_target_check = rule(
    implementation = _bindgen_target_check_impl,
    attrs = {
        "supported": attr.string_list(
            default = ["web", "bundler", "nodejs", "no-modules", "deno"],
        ),
        "target": attr.string(mandatory = True),
    },
)

def _expected_block(package, name, js, ts, wasm):
    """Renders the golden observation block for one binding subject."""
    return "\n".join([
        "subject " + package + ":" + name,
        "field bindgen.js=" + js,
        "field bindgen.ts=" + ts,
        "field bindgen.wasm=" + wasm,
        "field cli.owner=" + _CLI_OWNER,
        "field dep.label=" + _DEP_LABEL,
        "field lib.owner=" + _LIB_OWNER,
        "aspect_field aspect_seen=True",
        "aspect_field field_count=6",
        "aspect_field has_subject=True",
        "aspect_field subject_label=" + package + ":" + name,
        "aspect_field transitive_count=0",
    ])

_CLI_OWNER = "@@rules_rust_wasm_bindgen++rust_ext+rrwbd__wasm-bindgen-cli-0.2.121//:wasm-bindgen__bin"

_DEP_LABEL = "@@rules_rust_wasm_bindgen++rust_ext+rrwbd__wasm-bindgen-0.2.121//:wasm_bindgen"

_LIB_OWNER = "@@rules_rust_wasm_bindgen++rust_ext+rrwbd__wasm-bindgen-0.2.121//:src/lib.rs"

def bindgen_shape_tests(name, package, cases):
    """Instantiates one golden test over the given binding subjects."""
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [":" + label for (label, _js, _ts, _wasm) in cases],
        expected_observations = "\n".join([
            _expected_block(package, label, js, ts, wasm)
            for (label, js, ts, wasm) in cases
        ]),
    )
