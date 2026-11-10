"""Fetches the patched rules_dotnet copy."""

load("//core:repo.bzl", "pinned_archive")
load(":pins.bzl", "LIBFUZZER_DOTNET_SHA256", "LIBFUZZER_DOTNET_STRIP_PREFIX", "LIBFUZZER_DOTNET_URL", "RULES_DOTNET_SHA256", "RULES_DOTNET_STRIP_PREFIX", "RULES_DOTNET_URL")

def _dotnet_tools_impl(mctx):
    pinned_archive(
        name = "dx_libfuzzer_dotnet",
        files = {"BUILD.bazel": "exports_files([\"libfuzzer-dotnet.cc\"])\n"},
        patches = [Label("//patches/libfuzzer_dotnet:macos_section.patch")],
        required = ["libfuzzer-dotnet.cc"],
        sha256 = LIBFUZZER_DOTNET_SHA256,
        strip_prefix = LIBFUZZER_DOTNET_STRIP_PREFIX,
        urls = [LIBFUZZER_DOTNET_URL],
    )
    pinned_archive(
        name = "rules_dotnet",
        dropped = ["MODULE.bazel"],
        patches = [
            Label("//patches/rules_dotnet:reproducible.patch"),
            Label("//patches/rules_dotnet:fsharp_signature.patch"),
            Label("//patches/rules_dotnet:nuget_group_prefix.patch"),
            Label("//patches/rules_dotnet:sdk_versions.patch"),
            Label("//patches/rules_dotnet:canonical_labels.patch"),
        ],
        sha256 = RULES_DOTNET_SHA256,
        strip_prefix = RULES_DOTNET_STRIP_PREFIX,
        urls = [RULES_DOTNET_URL],
    )
    return mctx.extension_metadata(reproducible = True)

dotnet_tools = module_extension(
    implementation = _dotnet_tools_impl,
    doc = "Creates the patched rules_dotnet copy.",
)
