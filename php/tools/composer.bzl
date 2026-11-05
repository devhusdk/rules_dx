"""The composer_tool rule: the pinned Composer phar on the pinned PHP, as one executable."""

load("//libs/launcher:defs.bzl", "NATIVE_LAUNCHER_ATTRS")
load("//php/rules:php.bzl", "rf", "runfiles_path", "write_launcher")
load("//php/toolchain:php.bzl", "TOOLCHAIN_TYPE")

COMPOSER_INI = {
    "memory_limit": "2G",
}

def _composer_tool_impl(ctx):
    php = ctx.toolchains[TOOLCHAIN_TYPE].php
    phar = ctx.file._composer
    launcher, runfiles = write_launcher(ctx, php, COMPOSER_INI, [rf(runfiles_path(ctx.workspace_name, phar))])
    return [DefaultInfo(
        executable = launcher,
        files = depset([launcher]),
        runfiles = runfiles.merge(ctx.runfiles(files = [phar], transitive_files = php.files)),
    )]

composer_tool = rule(
    implementation = _composer_tool_impl,
    doc = "Runs the pinned Composer phar on the pinned PHP; `dx update` uses it to check and update composer sets.",
    executable = True,
    attrs = NATIVE_LAUNCHER_ATTRS | {
        "_composer": attr.label(default = Label("@dx_composer//:composer.phar"), allow_single_file = True, doc = "The pinned Composer phar."),
    },
    toolchains = [TOOLCHAIN_TYPE],
)
