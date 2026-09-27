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

def dx_harness_env(env = None, harness_src = None):
    """Returns caller env plus the bootstrap preload entries."""
    entry = {"DX_BOOTSTRAP": "_main/tools/sh/bootstrap.sh"}
    if harness_src != None:
        entry["DX_HARNESS_SRC"] = harness_src
    if env == None:
        return entry
    merged = dict(env)
    if "DX_BOOTSTRAP" not in merged:
        merged["DX_BOOTSTRAP"] = entry["DX_BOOTSTRAP"]
    if harness_src != None and "DX_HARNESS_SRC" not in merged:
        merged["DX_HARNESS_SRC"] = harness_src
    return merged

def dx_harness_entry(package_name, srcs):
    """Returns the preload entry srcs plus payload data and relpath."""
    if len(srcs) == 0:
        fail("dx_shell_harness: srcs must list one payload script")
    head = srcs[0]
    if head.startswith("//"):
        label = head[2:]
        if ":" in label:
            parts = label.split(":")
            rel = parts[0] + "/" + parts[1]
        else:
            rel = label + "/" + label.split("/")[-1]
    elif head.startswith(":"):
        rel = package_name + "/" + head[1:]
    else:
        rel = package_name + "/" + head
    return {
        "srcs": ["//tools/sh:entry.sh"],
        "data": list(srcs),
        "rel": rel,
    }

def dx_shell_harness(name, srcs, data = None, args = None, size = "small", timeout = "short", tags = None, target_compatible_with = None, **kwargs):
    """Declares one Linux-only shell-harness test with shared bootstrap data."""
    if args == None:
        args = []
    entry = dx_harness_entry(native.package_name(), srcs)
    explicit = dict(kwargs)
    explicit["env"] = dx_harness_env(explicit.get("env"), entry["rel"])
    explicit["target_compatible_with"] = dx_harness_linux(target_compatible_with)
    sh_test(
        name = name,
        size = size,
        timeout = timeout,
        srcs = entry["srcs"],
        args = args,
        data = dx_harness_data(data) + entry["data"],
        tags = dx_harness_tags(tags),
        **explicit
    )
