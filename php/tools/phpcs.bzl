"""The phpcs_tool rule: the pinned PHP_CodeSniffer phar on the pinned PHP, as one executable."""

load("//php/rules:php.bzl", "launcher_script", "runfiles_path")
load("//php/toolchain:php.bzl", "TOOLCHAIN_TYPE")

PHPCS_INI = {
    "memory_limit": "512M",
}

def _phpcs_tool_impl(ctx):
    php = ctx.toolchains[TOOLCHAIN_TYPE].php
    phar = ctx.file._phpcs
    launcher = ctx.actions.declare_file(ctx.label.name)
    tail = '"$runfiles/{}" "$@"'.format(runfiles_path(ctx.workspace_name, phar))
    ctx.actions.write(launcher, launcher_script(runfiles_path(ctx.workspace_name, php.interpreter), PHPCS_INI, tail), is_executable = True)
    return [DefaultInfo(
        executable = launcher,
        files = depset([launcher]),
        runfiles = ctx.runfiles(files = [launcher, phar], transitive_files = php.files),
    )]

phpcs_tool = rule(
    implementation = _phpcs_tool_impl,
    doc = "Runs the pinned PHP_CodeSniffer phar on the pinned PHP; `dx check` sets it with --@rules_dx//config:tool_phpcs.",
    executable = True,
    attrs = {
        "_phpcs": attr.label(default = Label("@dx_phpcs//:phpcs.phar"), allow_single_file = True, doc = "The pinned PHP_CodeSniffer phar."),
    },
    toolchains = [TOOLCHAIN_TYPE],
)
