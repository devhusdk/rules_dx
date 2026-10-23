"""Resource cost declarations for heavy actions and tests."""

_EXTRA_MEMORY_MB = 512

def _memory(os_name, linux_mb):
    """Returns the memory estimate for one host OS."""
    if os_name.startswith("win") or os_name in ("osx", "macos", "darwin"):
        return linux_mb + _EXTRA_MEMORY_MB
    return linux_mb

def dx_bundler_resources(os_name, inputs_size):
    """Resource set for JavaScript bundler actions."""
    return {"cpu": 1, "memory": _memory(os_name, 1536)}

def dx_quality_resources(os_name, inputs_size):
    """Resource set for real quality tool actions."""
    return {"cpu": 2, "memory": _memory(os_name, 2048)}

def dx_apple_compile_resources(os_name, inputs_size):
    """Resource set for Xcode toolchain actions."""
    return {"cpu": 2, "memory": _memory(os_name, 2048)}

_HEAVY_TEST_RESOURCES = {
    "large": {"cpu": 2, "memory": 2048},
    "enormous": {"cpu": 4, "memory": 4096},
}

def dx_test_resource_tags(size, tags):
    """Returns tags plus the missing cpu and memory cost of a large or enormous test."""
    tags = list(tags) if tags != None else []
    defaults = _HEAVY_TEST_RESOURCES.get(size, {})
    declared = {}
    for tag in tags:
        parts = tag.split(":")
        if len(parts) == 3 and parts[0] == "resources":
            declared[parts[1]] = True
    for key in ["cpu", "memory"]:
        if key in defaults and key not in declared:
            tags.append("resources:%s:%d" % (key, defaults[key]))
    return tags
