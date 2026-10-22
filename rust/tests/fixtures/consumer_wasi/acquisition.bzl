"""Opt-in WASI acquisition observations for the wasi consumer."""

load("@rules_dx//libs/starlark:defs.bzl", "DxSubjectInfo", "starlark_test")
load("@rules_dx//rust/tests/fixtures/consumer_wasm:acquisition.bzl", "acquisition_subject", "wasi_runtime_subject")

_WASIP1_PLATFORM = Label("@rules_rust//rust/platform:wasip1")

_WASI_TRIPLE = "wasm32-wasip1"

_WASI_OWNER = "rules_rust++rust+rust_linux_x86_64__" + _WASI_TRIPLE + "__stable_tools//:rust_toolchain"

_HOST_OWNER = "rules_rust++rust+rust_linux_x86_64__x86_64-unknown-linux-gnu__stable_tools//:rust_toolchain"

def _wasi_transition_impl(_settings, _attr):
    """Selects the WASI preview 1 platform for one dependency edge."""
    return {"//command_line_option:platforms": [str(_WASIP1_PLATFORM)]}

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
        "field payload.wasi_runtime=False",
        "field rustc.owner=" + owner,
        "field rustc.target=" + triple,
        "aspect_field aspect_seen=True",
        "aspect_field field_count=5",
        "aspect_field has_subject=True",
        "aspect_field subject_label=" + package + ":" + name,
        "aspect_field transitive_count=0",
    ])

def _runtime_block(package, name, reached):
    """Renders the golden Wasmtime reach block for one subject."""
    return "\n".join([
        "subject " + package + ":" + name,
        "field payload.wasi_runtime=" + str(reached),
        "aspect_field aspect_seen=True",
        "aspect_field field_count=1",
        "aspect_field has_subject=True",
        "aspect_field subject_label=" + package + ":" + name,
        "aspect_field transitive_count=0",
    ])

def consumer_wasi_acquisition_tests(name, binary, module, run):
    """Instantiates host, WASI and runtime acquisition subjects plus the golden test."""
    package = "//rust/tests/fixtures/consumer_wasi"
    acquisition_subject(
        name = name + "_binary_host",
        target = binary,
    )
    acquisition_subject(
        name = name + "_binary_wasi_inner",
        target = binary,
    )
    wasi_subject_proxy(
        name = name + "_binary_wasi",
        target = ":" + name + "_binary_wasi_inner",
    )
    wasi_runtime_subject(
        name = name + "_module_runtime",
        target = module,
    )
    wasi_runtime_subject(
        name = name + "_run_runtime",
        target = run,
    )
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [
            ":" + name + "_binary_host",
            ":" + name + "_binary_wasi",
            ":" + name + "_module_runtime",
            ":" + name + "_run_runtime",
        ],
        expected_observations = "\n".join([
            _expected_block(package, name + "_binary_host", _HOST_OWNER, "x86_64-unknown-linux-gnu"),
            _expected_block(package, name + "_binary_wasi", _WASI_OWNER, _WASI_TRIPLE),
            _runtime_block(package, name + "_module_runtime", False),
            _runtime_block(package, name + "_run_runtime", True),
        ]),
    )
