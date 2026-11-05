load("@rules_cc//cc:defs.bzl", "cc_binary")

package(default_visibility = ["//visibility:public"])

cc_binary(
    name = "pcov.so",
    srcs = [
        "pcov-1.0.12/pcov.c",
        "pcov-1.0.12/php_pcov.h",
    ],
    copts = [
        "-DCOMPILE_DL_PCOV",
        "-DZEND_ENABLE_STATIC_TSRMLS_CACHE=1",
        "-Ipcov-1.0.12",
    ],
    linkshared = True,
    deps = [
        "@dx_php_pcre2//:headers",
    ] + select({
        "@platforms//cpu:aarch64": ["@dx_php_headers_linux_arm64//:headers"],
        "//conditions:default": ["@dx_php_headers_linux_amd64//:headers"],
    }),
)
