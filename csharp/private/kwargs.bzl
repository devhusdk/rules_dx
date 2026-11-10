load("@rules_dx//libs/starlark:wrapper.bzl", "dx_setdefaults", "dx_warnings_as_errors")

def csharp_tfm_with_defaults(kwargs):
    """Returns kwargs defaulting target_frameworks and the upstream treat_warnings_as_errors, which follows //config:warnings_as_errors when unset."""
    rest, warning_flags = dx_warnings_as_errors(kwargs, True, False)
    return dx_setdefaults(rest, {"target_frameworks": ["net10.0"], "treat_warnings_as_errors": warning_flags})

def csharp_tfm_forward_kwargs(name, kwargs):
    """Returns the forwarder kwargs that fan Roslyn out over every declared target framework."""
    frameworks = kwargs.get("target_frameworks", None)
    if type(frameworks) != "list" or len(frameworks) < 2:
        return {}
    return {
        "dx_target_frameworks": frameworks,
        "dx_tfm_variants": ":" + name + "_upstream",
    }
