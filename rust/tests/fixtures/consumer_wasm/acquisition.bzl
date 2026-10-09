"""Opt-in platform acquisition observations for the wasm consumer."""

load("//libs/starlark:canonical.bzl", "strip_canonical")
load("//libs/starlark:defs.bzl", "DxSubjectInfo", "starlark_test")
load("//rust/toolchains:bindings.bzl", "RUST_TOOLCHAIN_TYPE")

_DISABLED_PLATFORM_REPOS = [
    "bundle",
    "dotnet_toolchains",
    "maven",
    "npm",
    "npm_tools",
    "paket.main",
    "pnpm",
    "pypi",
    "python_interpreters",
    "ruby",
    "ruby_toolchains",
]

_WASM_PLATFORM = "@rules_rust//rust/platform:wasm32"

_FOLLOW_ATTRS = ["actual", "binary", "crate", "deps", "proc_macro_deps", "target", "targets", "upstream"]

AcquisitionInfo = provider(
    doc = "Transitive repository names behind one consumer target.",
    fields = {
        "blocked": "Disabled payload repos in the closure.",
        "repos": "Every repository in the closure.",
    },
)

def _repo_name(text):
    """Returns the repository nickname for one stringified label."""
    label = strip_canonical(text)
    if label.startswith("@"):
        repo = label[1:].split("//")[0]
        if repo == "":
            return "main"
    else:
        return "main"
    name = repo.split("+")[-1]
    if name == "":
        return repo
    return name

def _acquisition_aspect_impl(target, ctx):
    """Collects the transitive repository closure without toolchain edges."""
    repos = {}
    blocked = {}
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
    own = _repo_name(str(target.label))
    repos[own] = True
    if own in _DISABLED_PLATFORM_REPOS:
        blocked[own] = True
    return [AcquisitionInfo(blocked = sorted(blocked.keys()), repos = sorted(repos.keys()))]

_acquisition_aspect = aspect(
    implementation = _acquisition_aspect_impl,
    attr_aspects = _FOLLOW_ATTRS,
)

def _rustc_target(owner):
    """Returns the target triple embedded in one toolchain owner label."""
    parts = owner.split("__")
    if len(parts) >= 3:
        return parts[1]
    return "unknown"

def _acquisition_subject_impl(ctx):
    """Exposes the resolved toolchain and payload closure for one target."""
    rustc = ctx.toolchains[RUST_TOOLCHAIN_TYPE].rustc
    owner = strip_canonical(str(rustc.owner))
    payload = ctx.attr.target[AcquisitionInfo]
    blocked = payload.blocked
    return [
        DefaultInfo(files = depset([])),
        DxSubjectInfo(fields = {
            "payload.blocked": ",".join(blocked) if len(blocked) > 0 else "(none)",
            "payload.dep_repos": ",".join(payload.repos),
            "rustc.owner": owner,
            "rustc.target": _rustc_target(owner),
        }),
    ]

acquisition_subject = rule(
    implementation = _acquisition_subject_impl,
    attrs = {"target": attr.label(mandatory = True, aspects = [_acquisition_aspect])},
    toolchains = [RUST_TOOLCHAIN_TYPE],
)

def _wasm_transition_impl(_settings, _attr):
    """Selects the raw wasm32 platform for one dependency edge."""
    return {"//command_line_option:platforms": [_WASM_PLATFORM]}

wasm_transition = transition(
    implementation = _wasm_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _wasm_subject_proxy_impl(ctx):
    """Forwards the wasm-configured subject observations into this configuration."""
    inner = ctx.attr.target[0] if type(ctx.attr.target) == "list" else ctx.attr.target
    return [inner[DefaultInfo], inner[DxSubjectInfo]]

wasm_subject_proxy = rule(
    implementation = _wasm_subject_proxy_impl,
    attrs = {"target": attr.label(mandatory = True, cfg = wasm_transition)},
)

_HOST_TRIPLE = "x86_64-unknown-linux-gnu"

_WASM_TRIPLE = "wasm32-unknown-unknown"

_HOST_OWNER = "rules_rust++rust+rust_linux_x86_64__" + _HOST_TRIPLE + "__stable_tools//:rust_toolchain"

_WASM_OWNER = "rules_rust++rust+rust_linux_x86_64__" + _WASM_TRIPLE + "__stable_tools//:rust_toolchain"

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
        expected_observations = "\n".join([
            _expected_block(package, name + "_bin_host", _HOST_OWNER, _HOST_TRIPLE),
            _expected_block(package, name + "_bin_wasm", _WASM_OWNER, _WASM_TRIPLE),
            _expected_block(package, name + "_lib_host", _HOST_OWNER, _HOST_TRIPLE),
            _expected_block(package, name + "_lib_wasm", _WASM_OWNER, _WASM_TRIPLE),
        ]),
    )
