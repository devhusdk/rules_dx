"""Consumer-owned wasm repositories for the wasm_bindgen fixture."""

WASM_MISMATCH_CLI_VERSION = "0.2.100"

_WASM_MISMATCH_ASSETS = {
    "linux_x86_64": (
        "wasm-bindgen-0.2.100-x86_64-unknown-linux-musl",
        "63d6a38deb65bd7023c02bdf382ab66b0d2c0241c8582fd3413b5a808b8aeb5b",
    ),
    "macos_x86_64": (
        "wasm-bindgen-0.2.100-x86_64-apple-darwin",
        "72289c54f63d2a2723aacfb38e7b22044d6aebc849ddee40172cda0e74be4107",
    ),
    "macos_arm64": (
        "wasm-bindgen-0.2.100-aarch64-apple-darwin",
        "69f25cb910de7e19777b3f93347f5e62a64c8f81709b41ba7242d00a9543573c",
    ),
}

def _wasm_mismatch_repo_impl(ctx):
    """Downloads the older wasm-bindgen CLI a consumer might still pin."""
    os = ctx.os.name
    arch = ctx.os.arch
    key = None
    if os.startswith("linux") and arch == "amd64":
        key = "linux_x86_64"
    if os.startswith("mac") and arch == "amd64":
        key = "macos_x86_64"
    if os.startswith("mac") and arch == "arm64":
        key = "macos_arm64"
    if key == None:
        fail("wasm_mismatch: no wasm-bindgen " + WASM_MISMATCH_CLI_VERSION + " asset for " + os + "/" + arch)
    asset, sha256 = _WASM_MISMATCH_ASSETS[key]
    ctx.download_and_extract(
        "https://github.com/wasm-bindgen/wasm-bindgen/releases/download/" + WASM_MISMATCH_CLI_VERSION + "/" + asset + ".tar.gz",
        sha256 = sha256,
        stripPrefix = asset,
    )
    ctx.file("BUILD.bazel", 'exports_files(["wasm-bindgen"])\n')

wasm_mismatch_repo = repository_rule(
    implementation = _wasm_mismatch_repo_impl,
)

_BUILD_BAZEL = '''load("@rules_dx//rust/rules:defs.bzl", "rust_shared_library")
load("@rules_rust_wasm_bindgen//:defs.bzl", "rust_wasm_bindgen")

rust_shared_library(
    name = "consumer",
    srcs = ["consumer.rs"],
    crate_name = "wasm_consumer",
    crate_root = "consumer.rs",
    edition = "2021",
    target_compatible_with = ["@platforms//cpu:wasm32"],
    deps = ["@rules_rust_wasm_bindgen//3rdparty:wasm_bindgen"],
    visibility = ["//visibility:public"],
)

rust_wasm_bindgen(
    name = "consumer_bindings",
    bindgen_flags = ["--no-typescript"],
    out_name = "consumer",
    target = "nodejs",
    wasm_file = ":consumer",
    visibility = ["//visibility:public"],
)
'''

_CONSUMER_RS = '''use wasm_bindgen::prelude::*;

#[wasm_bindgen]
pub fn consumer_double(value: u32) -> u32 {
    value * 2
}
'''

def _wasm_consumer_repo_impl(ctx):
    """Writes the binding sources a consumer of rules_dx would own itself."""
    ctx.file("BUILD.bazel", _BUILD_BAZEL)
    ctx.file("consumer.rs", _CONSUMER_RS)

wasm_consumer_repo = repository_rule(
    implementation = _wasm_consumer_repo_impl,
)
