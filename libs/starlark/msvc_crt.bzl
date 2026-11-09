"""Analysis-time check that Rust and its C/C++ deps agree on the MSVC CRT."""

load("//libs/starlark:wrapper.bzl", "dx_upstream")

DxMsvcCrtInfo = provider(
    doc = "How a target and its deps pick the MSVC CRT.",
    fields = {
        "crt_static": "Whether a Rust compile action sets +crt-static.",
        "dynamic": "Depset of labels compiled with /MD or /MDd.",
    },
)

_DYNAMIC = ["/MD", "/MDd"]

MSVC_CRT_FROM_RUST = "//libs/starlark:msvc_crt_from_rust"
MSVC_STATIC_CRT = ["-dynamic_link_msvcrt", "static_link_msvcrt"]
_FEATURES = "//command_line_option:features"

def msvc_crt_static_settings(features):
    """Returns the features and flag after a Rust target forces the static CRT on its subtree."""
    return {
        _FEATURES: features + [f for f in MSVC_STATIC_CRT if f not in features],
        MSVC_CRT_FROM_RUST: True,
    }

def msvc_crt_reset_settings(features, from_rust):
    """Returns the features and flag for a cc_binary: drop the static CRT only when Rust forced it."""
    if not from_rust:
        return {_FEATURES: features, MSVC_CRT_FROM_RUST: from_rust}
    return {
        _FEATURES: [f for f in features if f not in MSVC_STATIC_CRT],
        MSVC_CRT_FROM_RUST: False,
    }

def _crt_reset_impl(settings, _attr):
    return msvc_crt_reset_settings(settings[_FEATURES], settings[MSVC_CRT_FROM_RUST])

dx_msvc_crt_reset_transition = transition(
    implementation = _crt_reset_impl,
    inputs = [_FEATURES, MSVC_CRT_FROM_RUST],
    outputs = [_FEATURES, MSVC_CRT_FROM_RUST],
)

def _crt_static(argv):
    for arg in argv:
        if arg.startswith(("-Ctarget-feature=", "--codegen=target-feature=")) and "+crt-static" in arg:
            return True
    return False

def _dx_msvc_crt_aspect_impl(target, ctx):
    crt_static = False
    dynamic = []
    transitive = []
    for name in ["deps", "link_deps", "crate", "upstream"]:
        value = getattr(ctx.rule.attr, name, None)
        if value == None:
            continue
        for dep in value if type(value) == "list" else [value]:
            if DxMsvcCrtInfo in dep:
                crt_static = crt_static or dep[DxMsvcCrtInfo].crt_static
                transitive.append(dep[DxMsvcCrtInfo].dynamic)
    for action in target.actions:
        if action.mnemonic == "Rustc" and _crt_static(action.argv or []):
            crt_static = True
        elif action.mnemonic == "CppCompile" and any([flag in (action.argv or []) for flag in _DYNAMIC]):
            dynamic.append(str(target.label))
            break
    return [DxMsvcCrtInfo(
        crt_static = crt_static,
        dynamic = depset(dynamic, transitive = transitive),
    )]

dx_msvc_crt_aspect = aspect(
    implementation = _dx_msvc_crt_aspect_impl,
    attr_aspects = ["deps", "link_deps", "crate", "upstream"],
    doc = "Records +crt-static Rust compiles and /MD C/C++ compiles.",
)

def dx_msvc_crt_validate(ctx):
    """Fails when a +crt-static Rust target links a C/C++ dep compiled /MD."""
    upstream = dx_upstream(ctx)
    if DxMsvcCrtInfo not in upstream:
        return
    info = upstream[DxMsvcCrtInfo]
    if not info.crt_static:
        return
    dynamic = info.dynamic.to_list()
    if dynamic:
        fail("MSVC CRT mismatch: {} links Rust with +crt-static (/MT) but {} is compiled /MD".format(ctx.label, ", ".join(dynamic)))
