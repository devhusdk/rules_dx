"""Public Svelte API."""

load(
    "//svelte/private:defs.bzl",
    _svelte_library = "svelte_library",
    _svelte_build = "svelte_build",
    _svelte_test = "svelte_test",
)

svelte_library = _svelte_library
svelte_build = _svelte_build
svelte_test = _svelte_test
