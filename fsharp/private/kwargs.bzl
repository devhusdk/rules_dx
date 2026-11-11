load("@rules_dx//core:wrapper.bzl", "dx_setdefaults", "dx_warnings_as_errors")

FSHARP_CORE = Label("@paket.main//fsharp.core")

def fsharp_tfm_with_defaults(kwargs):
    """Returns kwargs defaulting target_frameworks and the upstream treat_warnings_as_errors, which follows //config:warnings_as_errors when unset."""
    rest, warning_flags = dx_warnings_as_errors(kwargs, True, False)
    return dx_setdefaults(rest, {"target_frameworks": ["net10.0"], "treat_warnings_as_errors": warning_flags})

def fsharp_with_core(kwargs):
    """Returns kwargs with FSharp.Core in deps unless deps is a select or already lists it."""
    deps = kwargs.get("deps") or []
    if type(deps) != "list":
        return kwargs
    for dep in deps:
        label = str(dep).lower()
        if label.endswith("//fsharp.core") or label.endswith(":fsharp.core"):
            return kwargs
    return dict(kwargs, deps = deps + [FSHARP_CORE])
