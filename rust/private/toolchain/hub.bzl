"""The rules_dx Rust toolchain hub: every target triple of the Rust manifest platforms, their musl variants, the Android tuples and the iOS environments."""

load("@rules_rust//rust/platform:triple_mappings.bzl", "triple_to_constraint_set")
load("@rules_rust//rust/private:repositories.bzl", "rust_register_toolchains")
load("//android/private/platforms:defs.bzl", "rust_triples")
load("//apple/private/ios:validate.bzl", "ios_rust_triples")
load("//core/modules:versions.bzl", "RUSTFMT_VERSION", "RUST_EDITION", "RUST_VERSION")

HUB_NAME = "dx_rust_toolchains"

PLATFORM_TRIPLES = {
    "linux_arm64": "aarch64-unknown-linux-gnu",
    "linux_x86_64": "x86_64-unknown-linux-gnu",
    "macos_arm64": "aarch64-apple-darwin",
    "windows_arm64": "aarch64-pc-windows-msvc",
    "windows_x86_64": "x86_64-pc-windows-msvc",
}

MUSL_TRIPLES = {
    "linux_arm64": "aarch64-unknown-linux-musl",
    "linux_x86_64": "x86_64-unknown-linux-musl",
}

def target_triples(rust_platforms):
    """Returns the sorted Rust target triples for the Rust manifest platforms, their musl variants and every Android and iOS triple."""
    unknown = [p for p in rust_platforms if p not in PLATFORM_TRIPLES]
    if unknown:
        fail("rust/lang.json platforms have no Rust triple: {}".format(", ".join(unknown)))
    triples = {PLATFORM_TRIPLES[p]: True for p in rust_platforms}
    triples.update({MUSL_TRIPLES[p]: True for p in rust_platforms if p in MUSL_TRIPLES})
    triples.update({t: True for t in rust_triples()})
    triples.update({t: True for t in ios_rust_triples()})
    return sorted(triples.keys())

def target_constraints(triples):
    """Returns each triple's target constraints; a musl triple also requires the musl libc."""
    musl = str(Label("@llvm//constraints/libc:musl"))
    return {triple: triple_to_constraint_set(triple) + ([musl] if triple.endswith("-linux-musl") else []) for triple in triples}

def _dx_rust_toolchains_impl(module_ctx):
    manifest = json.decode(module_ctx.read(Label("//rust:lang.json")))
    rust_register_toolchains(
        hub_name = HUB_NAME,
        edition = RUST_EDITION,
        extra_target_triples = target_constraints(target_triples(manifest["platforms"])),
        rustfmt_version = RUSTFMT_VERSION,
        target_settings = [str(Label("//config:toolchains_on"))],
        versions = [RUST_VERSION],
        compact_windows_names = True,
    )
    return module_ctx.extension_metadata(reproducible = True)

dx_rust_toolchains = module_extension(
    implementation = _dx_rust_toolchains_impl,
    doc = "Declares the `dx_rust_toolchains` hub that rules_dx registers for consumers.",
)
