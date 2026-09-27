"""Linux-only shell-harness test with shared bootstrap data."""

load("@rules_shell//shell:sh_test.bzl", "sh_test")

def dx_harness_data(data = None):
    """Returns caller data plus the shared harness bootstrap data."""
    shared = ["//tools/sh:bootstrap", "//tools/sh:lib"]
    if data == None:
        return shared
    return data + shared

def dx_harness_tags(tags = None):
    """Returns caller tags plus the harness no-coverage marker."""
    if tags == None:
        return ["no-coverage"]
    if "no-coverage" in tags:
        return tags
    return tags + ["no-coverage"]

def dx_harness_linux(compatible_with = None):
    """Returns the Linux-only constraint for shell-harness targets."""
    if compatible_with == None:
        return ["@platforms//os:linux"]
    return compatible_with

def dx_harness_env(env = None):
    """Returns caller env plus the bootstrap preload entry."""
    entry = {"DX_BOOTSTRAP": "_main/tools/sh/bootstrap.sh"}
    if env == None:
        return entry
    merged = dict(env)
    if "DX_BOOTSTRAP" not in merged:
        merged["DX_BOOTSTRAP"] = entry["DX_BOOTSTRAP"]
    return merged

def dx_shell_harness(name, srcs, data = None, args = None, size = "small", timeout = "short", tags = None, target_compatible_with = None, **kwargs):
    """Declares one Linux-only shell-harness test with shared bootstrap data."""
    if args == None:
        args = []
    explicit = dict(kwargs)
    explicit["env"] = dx_harness_env(explicit.get("env"))
    explicit["target_compatible_with"] = dx_harness_linux(target_compatible_with)
    sh_test(
        name = name,
        size = size,
        timeout = timeout,
        srcs = srcs,
        args = args,
        data = dx_harness_data(data),
        tags = dx_harness_tags(tags),
        **explicit
    )
