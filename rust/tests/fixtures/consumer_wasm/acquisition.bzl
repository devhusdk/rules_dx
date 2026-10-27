"""Opt-in platform acquisition observations for the wasm consumer."""

load("//libs/starlark:canonical.bzl", "strip_canonical")
load("//libs/starlark:defs.bzl", "DxSubjectInfo", "starlark_test")
load("//rust/toolchains:bindings.bzl", "RUST_TOOLCHAIN_TYPE")

_DISABLED_PLATFORM_REPOS = [
    "bundle",
    "dotnet_toolchains",
    "dx_npm",
    "dx_npm_tools",
    "dx_pypi",
    "maven",
    "paket.main",
    "pnpm",
    "python_interpreters",
    "ruby",
    "ruby_toolchains",
]

_WASM_PLATFORM = "@rules_rust//rust/platform:wasm32"

_FOLLOW_ATTRS = ["actual", "binary", "crate", "data", "deps", "proc_macro_deps", "target", "targets", "upstream"]

_WASI_RUNTIME_PREFIX = "dx_wasmtime"

AcquisitionInfo = provider(
    doc = "Transitive repository names behind one consumer target.",
    fields = {
        "blocked": "Disabled payload repos in the closure.",
        "repos": "Every repository in the closure.",
        "wasi_runtime": "Whether the closure reaches a Wasmtime repository.",
    },
)

def _repo_name(label):
    """Returns the repository nickname for one label."""
    if label.repo_name == "":
        return "main"
    return [part for part in label.repo_name.split("+") if part != ""][-1]

def _acquisition_aspect_impl(target, ctx):
    """Collects the transitive repository closure without toolchain edges."""
    repos = {}
    blocked = {}
    wasi_runtime = False
    for name in _FOLLOW_ATTRS:
        value = getattr(ctx.rule.attr, name, [])
        edges = value if type(value) == "list" else [value]
        for dep in edges:
            if type(dep) == "Target" and AcquisitionInfo in dep:
                info = dep[AcquisitionInfo]
                for repo in info.repos:
                    repos[repo] = True
                for repo in info.blocked:
                    blocked[repo] = True
                wasi_runtime = wasi_runtime or info.wasi_runtime
    own = _repo_name(target.label)
    repos[own] = True
    if own in _DISABLED_PLATFORM_REPOS:
        blocked[own] = True
    if own.startswith(_WASI_RUNTIME_PREFIX):
        wasi_runtime = True
    return [AcquisitionInfo(
        blocked = sorted(blocked.keys()),
        repos = sorted(repos.keys()),
        wasi_runtime = wasi_runtime,
    )]

_acquisition_aspect = aspect(
    implementation = _acquisition_aspect_impl,
    attr_aspects = _FOLLOW_ATTRS,
)

def _acquisition_subject_impl(ctx):
    """Exposes the resolved toolchain and payload closure for one target."""
    toolchain = ctx.toolchains[RUST_TOOLCHAIN_TYPE]
    owner = strip_canonical(str(toolchain.rustc.owner))
    payload = ctx.attr.target[AcquisitionInfo]
    blocked = payload.blocked
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = {
            "payload.blocked": ",".join(blocked) if len(blocked) > 0 else "(none)",
            "payload.dep_repos": ",".join(payload.repos),
            "payload.wasi_runtime": str(payload.wasi_runtime),
            "rustc.owner": owner,
            "rustc.target": toolchain.target_triple.str,
        }),
    ]

acquisition_subject = rule(
    implementation = _acquisition_subject_impl,
    attrs = {"target": attr.label(mandatory = True, aspects = [_acquisition_aspect])},
    toolchains = [RUST_TOOLCHAIN_TYPE],
)

def _wasi_runtime_subject_impl(ctx):
    """Exposes whether one target's closure reaches the Wasmtime payload."""
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = {
            "payload.wasi_runtime": str(ctx.attr.target[AcquisitionInfo].wasi_runtime),
        }),
    ]

wasi_runtime_subject = rule(
    implementation = _wasi_runtime_subject_impl,
    attrs = {"target": attr.label(mandatory = True, aspects = [_acquisition_aspect])},
)

def _wasm_transition_impl(_settings, _attr):
    """Selects the raw wasm32 platform without coverage for one dependency edge."""
    return {
        "//command_line_option:collect_code_coverage": False,
        "//command_line_option:platforms": [_WASM_PLATFORM],
    }

wasm_transition = transition(
    implementation = _wasm_transition_impl,
    inputs = [],
    outputs = [
        "//command_line_option:collect_code_coverage",
        "//command_line_option:platforms",
    ],
)

def _wasm_subject_proxy_impl(ctx):
    """Forwards the wasm-configured subject observations into this configuration."""
    inner = ctx.attr.target[0] if type(ctx.attr.target) == "list" else ctx.attr.target
    return [inner[DefaultInfo], inner[DxSubjectInfo]]

wasm_subject_proxy = rule(
    implementation = _wasm_subject_proxy_impl,
    attrs = {"target": attr.label(mandatory = True, cfg = wasm_transition)},
)

_WASM_TRIPLE = "wasm32-unknown-unknown"

_HOSTS = {
    "linux_arm64": ("linux", "aarch64", "linux_aarch64", "aarch64-unknown-linux-gnu"),
    "linux_x86_64": ("linux", "x86_64", "linux_x86_64", "x86_64-unknown-linux-gnu"),
    "macos_arm64": ("macos", "aarch64", "macos_aarch64", "aarch64-apple-darwin"),
    "windows_arm64": ("windows", "aarch64", "windows_aarch64", "aarch64-pc-windows-msvc"),
    "windows_x86_64": ("windows", "x86_64", "windows_x86_64", "x86_64-pc-windows-msvc"),
}

def rust_owner(host_tools, triple):
    """Returns the rules_rust toolchain repo that compiles for triple on host_tools."""
    repo = "rust_" + host_tools + "__" + triple + "__stable"
    if host_tools.startswith("windows"):
        repo = "rw-" + str(abs(hash(repo)))
    return "rules_rust++rust+" + repo + "_tools//:rust_toolchain"

def host_select(name, render):
    """Declares one config_setting per host and selects render(host_tools, host_triple) on it."""
    branches = {}
    for host, (os, cpu, host_tools, triple) in _HOSTS.items():
        native.config_setting(
            name = name + "_" + host,
            constraint_values = [
                "@platforms//os:" + os,
                "@platforms//cpu:" + cpu,
            ],
        )
        branches[":" + name + "_" + host] = render(host_tools, triple)
    return select(branches)

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

def consumer_acquisition_tests(name, lib, binary):
    """Instantiates host and wasm acquisition subjects plus the golden test."""
    package = "//rust/tests/fixtures/consumer_wasm"
    acquisition_subject(
        name = name + "_lib_host",
        target = lib,
    )
    acquisition_subject(
        name = name + "_bin_host",
        target = binary,
    )
    acquisition_subject(
        name = name + "_lib_wasm_inner",
        target = lib,
    )
    wasm_subject_proxy(
        name = name + "_lib_wasm",
        target = ":" + name + "_lib_wasm_inner",
    )
    acquisition_subject(
        name = name + "_bin_wasm_inner",
        target = binary,
    )
    wasm_subject_proxy(
        name = name + "_bin_wasm",
        target = ":" + name + "_bin_wasm_inner",
    )
    starlark_test(
        name = name,
        mode = "analysis",
        subjects = [
            ":" + name + "_bin_host",
            ":" + name + "_bin_wasm",
            ":" + name + "_lib_host",
            ":" + name + "_lib_wasm",
        ],
        expected_observations = host_select(name + "_host", lambda tools, triple: "\n".join([
            _expected_block(package, name + "_bin_host", rust_owner(tools, triple), triple),
            _expected_block(package, name + "_bin_wasm", rust_owner(tools, _WASM_TRIPLE), _WASM_TRIPLE),
            _expected_block(package, name + "_lib_host", rust_owner(tools, triple), triple),
            _expected_block(package, name + "_lib_wasm", rust_owner(tools, _WASM_TRIPLE), _WASM_TRIPLE),
        ])),
    )
