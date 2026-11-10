load("@rules_dx//core:wrapper.bzl", "dx_setdefaults", "dx_warnings_as_errors")

def fsharp_tfm_with_defaults(kwargs):
    """Returns kwargs defaulting target_frameworks and the upstream treat_warnings_as_errors, which follows //config:warnings_as_errors when unset."""
    rest, warning_flags = dx_warnings_as_errors(kwargs, True, False)
    return dx_setdefaults(rest, {"target_frameworks": ["net10.0"], "treat_warnings_as_errors": warning_flags})
