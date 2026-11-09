"""Fetches the patched rules_dotnet copy."""

load("//libs/starlark:repo.bzl", "pinned_archive")

def _dotnet_tools_impl(mctx):
    pinned_archive(
        name = "dx_libfuzzer_dotnet",
        files = {"BUILD.bazel": "exports_files([\"libfuzzer-dotnet.cc\"])\n"},
        required = ["libfuzzer-dotnet.cc"],
        sha256 = "c7e8281f29072c9a067d703aeeaaf8aa6b7b063cba2d3bd67cb8d85a48fa7c94",
        strip_prefix = "libfuzzer-dotnet-bd39d4e88d715ab460a929943645be2a186cde52",
        urls = ["https://github.com/Metalnem/libfuzzer-dotnet/archive/bd39d4e88d715ab460a929943645be2a186cde52.tar.gz"],
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
        sha256 = "ce35dad18db31cd68323b568dc09c689cc6d75ec16eba1510ccd128bb5f1264e",
        strip_prefix = "rules_dotnet-0.22.2",
        urls = ["https://github.com/bazel-contrib/rules_dotnet/releases/download/v0.22.2/rules_dotnet-v0.22.2.tar.gz"],
    )
    return mctx.extension_metadata(reproducible = True)

dotnet_tools = module_extension(
    implementation = _dotnet_tools_impl,
    doc = "Creates the patched rules_dotnet copy.",
)
