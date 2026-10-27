"""The API reference shards the user documentation site publishes."""

load("//tools/docs/rust:defs.bzl", "rust_doc_shard")

PUBLIC_RUST_CRATES = [
    "//cli/advisory_prep:dx_advisory_prep",
    "//cli/tool_resolve:dx_tool_resolve",
    "//deploy/offline/bootstrap:dx_bootstrap",
    "//deploy/rules:dx_deploy_tools",
    "//quality/tools/javascript/launcher:dx_js_launcher",
    "//tools/docs/rust/extract:dx_docs_rust_extract",
    "//tools/docs/site/aggregate:dx_site_aggregate",
    "//tools/docs/site/browser:dx_site_browser",
    "//tools/docs/site/build:dx_site_build",
    "//tools/docs/site/check:dx_site_check",
    "//tools/docs/typescript/extract:dx_docs_typescript_extract",
    "//wasi/testing:wasi_run",
]

def user_api_shards():
    """Declares one DocIr shard per public crate and returns their labels."""
    shards = []
    for crate in PUBLIC_RUST_CRATES:
        name = crate.split(":")[1] + "_docs"
        rust_doc_shard(name = name, crate = crate)
        shards.append(":" + name)
    return shards
