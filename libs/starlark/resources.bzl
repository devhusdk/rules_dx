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

_HEAVY_TEST_TAGS = {
    "large": ["resources:cpu:2", "resources:memory:2048"],
    "enormous": ["resources:cpu:4", "resources:memory:4096"],
}

def dx_test_resource_tags(size, tags):
    """Returns tags plus the resource cost of a large or enormous test."""
    tags = list(tags) if tags != None else []
    for tag in tags:
        if tag.startswith("resources:"):
            return tags
    return tags + _HEAVY_TEST_TAGS.get(size, [])
