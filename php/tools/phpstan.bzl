"""The phpstan_tool rule: the pinned PHPStan phar on the pinned PHP, as one executable."""

load("//php/rules:php.bzl", "launcher_script", "runfiles_path")
load("//php/toolchain:php.bzl", "TOOLCHAIN_TYPE")

PHPSTAN_INI = {
    "memory_limit": "2G",
}

def _phpstan_tool_impl(ctx):
    php = ctx.toolchains[TOOLCHAIN_TYPE].php
    phar = ctx.file._phpstan
    launcher = ctx.actions.declare_file(ctx.label.name)
    tail = '"$runfiles/{}" "$@"'.format(runfiles_path(ctx.workspace_name, phar))
    ctx.actions.write(launcher, launcher_script(runfiles_path(ctx.workspace_name, php.interpreter), PHPSTAN_INI, tail), is_executable = True)
    return [DefaultInfo(
        executable = launcher,
        files = depset([launcher]),
        runfiles = ctx.runfiles(files = [launcher, phar], transitive_files = php.files),
    )]

phpstan_tool = rule(
    implementation = _phpstan_tool_impl,
    doc = "Runs the pinned PHPStan phar on the pinned PHP; `dx check` sets it with --@rules_dx//config:tool_phpstan.",
    executable = True,
    attrs = {
        "_phpstan": attr.label(default = Label("@dx_phpstan//:phpstan.phar"), allow_single_file = True, doc = "The pinned PHPStan phar."),
    },
    toolchains = [TOOLCHAIN_TYPE],
)
