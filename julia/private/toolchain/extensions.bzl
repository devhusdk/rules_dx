"""Module extension that creates the patched rules_julia repository."""

load("//core:repo.bzl", "pinned_archive")
load(":pins.bzl", "RULES_JULIA")

def _rules_julia_impl(mctx):
    pinned_archive(
        name = "rules_julia",
        patches = [
            Label("//patches/rules_julia:package_annotation_version.patch"),
            Label("//patches/rules_julia:package_name.patch"),
            Label("//patches/rules_julia:portable_cpu_target.patch"),
            Label("//patches/rules_julia:template_action_input.patch"),
        ],
        sha256 = RULES_JULIA.sha256,
        urls = RULES_JULIA.urls,
    )
    return mctx.extension_metadata(reproducible = True)

rules_julia = module_extension(
    implementation = _rules_julia_impl,
    doc = "Creates rules_julia with the rules_dx patches applied.",
)
