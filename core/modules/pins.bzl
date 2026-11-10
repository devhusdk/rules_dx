"""Root module deps that rules_dx pins for consumers, as labels of each repo."""

PINNED_REPOS = {
    "bazel_features": Label("@bazel_features//:all"),
    "bazel_linux_packages": Label("@bazel_linux_packages//:all"),
    "elfutils": Label("@elfutils//:all"),
    "hermetic_launcher": Label("@hermetic_launcher//:all"),
    "libxml2": Label("@libxml2//:all"),
    "lua": Label("@lua//:all"),
    "rules_android": Label("@rules_android//:all"),
    "rules_apple": Label("@rules_apple//:all"),
    "rules_batch": Label("@rules_batch//:all"),
    "rules_jvm_external": Label("@rules_jvm_external//:all"),
    "rules_rust_prost": Label("@rules_rust_prost//:all"),
    "tar.bzl": Label("@tar.bzl//:all"),
    "windows_support": Label("@windows_support//:all"),
    "yaml.bzl": Label("@yaml.bzl//:all"),
    "zlib": Label("@zlib//:all"),
}
