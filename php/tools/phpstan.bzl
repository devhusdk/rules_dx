"""The phpstan_tool rule: the pinned PHPStan phar on the pinned PHP, as one executable."""

load("//php/rules:php.bzl", "LAUNCHER_ATTRS", "rf", "runfiles_path", "write_launcher")
load("//php/toolchain:php.bzl", "TOOLCHAIN_TYPE")

PHPSTAN_INI = {
    "memory_limit": "2G",
}

def _phpstan_tool_impl(ctx):
    php = ctx.toolchains[TOOLCHAIN_TYPE].php
    phar = ctx.file._phpstan
    launcher, runfiles = write_launcher(ctx, php, PHPSTAN_INI, [rf(runfiles_path(ctx.workspace_name, phar))])
    return [DefaultInfo(
        executable = launcher,
        files = depset([launcher]),
        runfiles = runfiles.merge(ctx.runfiles(files = [phar], transitive_files = php.files)),
    )]

phpstan_tool = rule(
    implementation = _phpstan_tool_impl,
    doc = "Runs the pinned PHPStan phar on the pinned PHP; `dx check` sets it with --@rules_dx//config:tool_phpstan.",
    executable = True,
    attrs = LAUNCHER_ATTRS | {
        "_phpstan": attr.label(default = Label("@dx_phpstan//:phpstan.phar"), allow_single_file = True, doc = "The pinned PHPStan phar."),
    },
    toolchains = [TOOLCHAIN_TYPE],
)
