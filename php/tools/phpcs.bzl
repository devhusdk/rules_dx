"""The phpcs_tool rule: the pinned PHP_CodeSniffer phar on the pinned PHP, as one executable."""

load("//php/rules:php.bzl", "LAUNCHER_ATTRS", "rf", "runfiles_path", "write_launcher")
load("//php/toolchain:php.bzl", "TOOLCHAIN_TYPE")

PHPCS_INI = {
    "memory_limit": "512M",
}

def _phpcs_tool_impl(ctx):
    php = ctx.toolchains[TOOLCHAIN_TYPE].php
    phar = ctx.file._phpcs
    launcher, runfiles = write_launcher(ctx, php, PHPCS_INI, [rf(runfiles_path(ctx.workspace_name, phar))])
    return [DefaultInfo(
        executable = launcher,
        files = depset([launcher]),
        runfiles = runfiles.merge(ctx.runfiles(files = [phar], transitive_files = php.files)),
    )]

phpcs_tool = rule(
    implementation = _phpcs_tool_impl,
    doc = "Runs the pinned PHP_CodeSniffer phar on the pinned PHP; `dx check` sets it with --@rules_dx//config:tool_phpcs.",
    executable = True,
    attrs = LAUNCHER_ATTRS | {
        "_phpcs": attr.label(default = Label("@dx_phpcs//:phpcs.phar"), allow_single_file = True, doc = "The pinned PHP_CodeSniffer phar."),
    },
    toolchains = [TOOLCHAIN_TYPE],
)
