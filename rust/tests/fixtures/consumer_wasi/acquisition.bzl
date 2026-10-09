"""Opt-in WASI acquisition observations for the wasi consumer."""

load("@rules_dx//libs/starlark:defs.bzl", "DxSubjectInfo", "starlark_test")
load("@rules_dx//rust/tests/fixtures/consumer_wasm:acquisition.bzl", "acquisition_subject")

_WASIP1_PLATFORM = "@rules_rust//rust/platform:wasip1"

_WASI_TRIPLE = "wasm32-wasip1"

_WASI_OWNER = "rules_rust++rust+rust_linux_x86_64__" + _WASI_TRIPLE + "__stable_tools//:rust_toolchain"

_HOST_OWNER = "rules_rust++rust+rust_linux_x86_64__x86_64-unknown-linux-gnu__stable_tools//:rust_toolchain"

def _wasi_transition_impl(_settings, _attr):
    """Selects the WASI preview 1 platform for one dependency edge."""
    return {"//command_line_option:platforms": [_WASIP1_PLATFORM]}

wasi_transition = transition(
    implementation = _wasi_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _wasi_subject_proxy_impl(ctx):
    """Forwards the wasi-configured subject observations into this configuration."""
    inner = ctx.attr.target[0] if type(ctx.attr.target) == "list" else ctx.attr.target
    return [inner[DefaultInfo], inner[DxSubjectInfo]]

wasi_subject_proxy = rule(
    implementation = _wasi_subject_proxy_impl,
    attrs = {"target": attr.label(mandatory = True, cfg = wasi_transition)},
)

def _expected_block(package, name, owner, triple):
    """Renders the golden observation block for one acquisition subject."""
    return "\n".join([
        "subject " + package + ":" + name,
        "field payload.blocked=(none)",
        "field payload.dep_repos=main",
        "field rustc.owner=" + owner,
        "field rustc.target=" + triple,
        "aspect_field aspect_seen=True",
        "aspect_field field_count=4",
        "aspect_field has_subject=True",
        "aspect_field subject_label=" + package + ":" + name,
        "aspect_field transitive_count=0",
    ])

def consumer_wasi_acquisition_tests(name, module):
    """Instantiates host and wasi acquisition subjects plus the golden test."""
    package = "//rust/tests/fixtures/consumer_wasi"
    acquisition_subject(
        name = name + "_module_host",
        target = module,
    )
    acquisition_subject(
        name = name + "_module_wasi_inner",
        target = module,
    )
    wasi_subject_proxy(
        name = name + "_module_wasi",
        target = ":" + name + "_module_wasi_inner",
    )
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [
            ":" + name + "_module_host",
            ":" + name + "_module_wasi",
        ],
        expected_observations = "\n".join([
            _expected_block(package, name + "_module_host", _HOST_OWNER, "x86_64-unknown-linux-gnu"),
            _expected_block(package, name + "_module_wasi", _WASI_OWNER, _WASI_TRIPLE),
        ]),
    )
