"""The TypeScript compiler repository at the rules_dx pin."""

load("@aspect_rules_ts//ts/private:npm_repositories.bzl", "npm_dependencies")
load("//core/modules:versions.bzl", "TYPESCRIPT_INTEGRITY", "TYPESCRIPT_VERSION")

def _typescript_impl(module_ctx):
    npm_dependencies(
        ts_version = TYPESCRIPT_VERSION,
        ts_integrity = TYPESCRIPT_INTEGRITY,
    )
    return module_ctx.extension_metadata(reproducible = True)

typescript = module_extension(implementation = _typescript_impl)
