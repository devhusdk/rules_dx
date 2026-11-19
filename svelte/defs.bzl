"""Public Svelte API."""

load(
    "//svelte/private:defs.bzl",
    _svelte_library = "svelte_library",
    _svelte_build = "svelte_build",
    _svelte_test = "svelte_test",
)
load("//tools/docs/components:defs.bzl", _svelte_doc_shard = "svelte_doc_shard")

svelte_library = _svelte_library
svelte_build = _svelte_build
svelte_test = _svelte_test
svelte_doc_shard = _svelte_doc_shard
