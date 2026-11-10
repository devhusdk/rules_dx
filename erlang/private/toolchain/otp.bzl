"""Relocatable OTP tree exposed through the rules_erlang OtpInfo provider."""

load("@rules_erlang//private:erlang_build.bzl", "OtpInfo")
load("@rules_erlang//tools:erlang_toolchain.bzl", "TREE_ERLANG_HOME")

def _dx_otp_impl(ctx):
    tree = depset(ctx.files.srcs + [ctx.file.root])
    return [
        DefaultInfo(files = tree),
        OtpInfo(
            version = ctx.attr.version,
            release_dir_tar = None,
            install_path = None,
            erlang_home = TREE_ERLANG_HOME,
            version_file = ctx.file.root,
            tree = tree,
        ),
    ]

dx_otp = rule(
    implementation = _dx_otp_impl,
    doc = "Exposes one in-tree OTP installation as OtpInfo.",
    attrs = {
        "root": attr.label(allow_single_file = True, mandatory = True, doc = "Version file at the OTP root."),
        "srcs": attr.label_list(allow_files = True, mandatory = True, doc = "Every file of the OTP tree."),
        "version": attr.string(mandatory = True, doc = "Exact OTP version."),
    },
    provides = [OtpInfo],
)
