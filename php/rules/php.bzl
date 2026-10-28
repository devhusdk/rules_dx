"""Core PHP rules: php_library, php_binary and php_test on the pinned PHP, Composer and PHPUnit."""

load("//php/toolchain:php.bzl", "EXEC_TOOLCHAIN_TYPE", "TOOLCHAIN_TYPE")
load(":providers.bzl", "PhpInfo")

PHP_SOURCE_EXTS = [".php"]

_TOOL = Label("//php/rules:tool.php")

_COMPOSER = Label("@dx_composer//:composer.phar")

_PHPUNIT = Label("@dx_phpunit//:phpunit.phar")

def staged_path(file):
    """Returns where one source sits below the autoload root: its workspace path, external/<repo>/... for other repos."""
    path = file.short_path
    if path.startswith("../"):
        return "external/" + path[3:]
    return path

def autoloader_suffix(label):
    """Returns the Composer autoloader suffix for one label: letters, digits and underscores only."""
    text = (label.repo_name or "main") + "_" + label.package + "_" + label.name
    out = ["Dx_"]
    for c in text.elems():
        out.append(c if c.isalnum() else "_")
    return "".join(out)

def shell_quote(text):
    """Returns text as one single-quoted shell word."""
    return "'" + text.replace("'", "'\\''") + "'"

def ini_flags(ini):
    """Returns the -d flags for an ini dict, sorted by key."""
    return [flag for key in sorted(ini.keys()) for flag in ("-d", key + "=" + ini[key])]

def runfiles_path(workspace_name, file):
    """Returns one file's path below the runfiles root."""
    if file.short_path.startswith("../"):
        return file.short_path[3:]
    return workspace_name + "/" + file.short_path

def _src_args(file):
    return ["--src", file.path, staged_path(file)]

def _package_args(package):
    return ["--package", package.manifest.path, package.root]

def _file_args(file):
    return ["--file", staged_path(file)]

def _php_info(ctx, srcs):
    deps = [dep[PhpInfo] for dep in ctx.attr.deps]
    return PhpInfo(
        autoload_files = depset(
            getattr(ctx.files, "autoload_files", []),
            transitive = [info.autoload_files for info in deps],
        ),
        package_files = depset(transitive = [info.package_files for info in deps]),
        packages = depset(transitive = [info.packages for info in deps]),
        transitive_srcs = depset(srcs, transitive = [info.transitive_srcs for info in deps]),
    )

def _lint(ctx, srcs):
    php = ctx.toolchains[EXEC_TOOLCHAIN_TYPE].php
    markers = []
    for src in srcs:
        marker = ctx.actions.declare_file(ctx.label.name + ".lint/" + staged_path(src) + ".ok")
        ctx.actions.run(
            executable = php.interpreter,
            arguments = ["-n", ctx.file._tool.path, "lint", marker.path, src.path],
            inputs = [src, ctx.file._tool],
            outputs = [marker],
            mnemonic = "PhpLint",
            progress_message = "Linting PHP %{input}",
            toolchain = EXEC_TOOLCHAIN_TYPE,
        )
        markers.append(marker)
    return markers

def _autoload(ctx, info, suffix):
    """Stages every source under <name>.php/ and runs an offline composer dump-autoload."""
    php = ctx.toolchains[EXEC_TOOLCHAIN_TYPE].php
    tree = ctx.actions.declare_directory(ctx.label.name + ".php")
    args = ctx.actions.args()
    args.add_all(["-n", ctx.file._tool, "autoload", ctx.file._composer, tree.path, suffix])
    args.add_all(info.transitive_srcs, map_each = _src_args)
    args.add_all(info.autoload_files, map_each = _file_args)
    args.add_all(info.packages, map_each = _package_args)
    ctx.actions.run(
        executable = php.interpreter,
        arguments = [args],
        inputs = depset([ctx.file._tool, ctx.file._composer], transitive = [info.transitive_srcs, info.package_files]),
        outputs = [tree],
        mnemonic = "PhpAutoload",
        progress_message = "Writing the Composer autoloader of %{label}",
        toolchain = EXEC_TOOLCHAIN_TYPE,
    )
    return tree

_LAUNCHER = """#!/usr/bin/env bash
set -euo pipefail
if [[ -d "$0.runfiles" ]]; then
    runfiles="$0.runfiles"
elif [[ -n "${{RUNFILES_DIR:-}}" ]]; then
    runfiles="$RUNFILES_DIR"
elif [[ "$0" == *.runfiles/* ]]; then
    runfiles="${{0%%.runfiles/*}}.runfiles"
else
    echo "$0: no runfiles tree" >&2
    exit 2
fi
export RUNFILES_DIR="$runfiles"
{body}"""

def launcher_script(php, ini, tail):
    """Returns the launcher that runs the runfiles php with -n, the ini flags and one argument tail."""
    flags = "".join([shell_quote(flag) + " " for flag in ini_flags(ini)])
    return _LAUNCHER.format(body = 'exec "$runfiles/{php}" -n {flags}{tail}\n'.format(php = php, flags = flags, tail = tail))

def _write_launcher(ctx, tail):
    """Writes the launcher of a binary or test."""
    php = ctx.toolchains[TOOLCHAIN_TYPE].php
    launcher = ctx.actions.declare_file(ctx.label.name)
    ctx.actions.write(launcher, launcher_script(runfiles_path(ctx.workspace_name, php.interpreter), ctx.attr.ini, tail), is_executable = True)
    return launcher, php.files

def _runfiles(ctx, files, transitive):
    runfiles = ctx.runfiles(files = files, transitive_files = depset(transitive = transitive))
    for target in ctx.attr.data + ctx.attr.deps:
        runfiles = runfiles.merge(target[DefaultInfo].default_runfiles)
    return runfiles

def _environment(ctx):
    env = {key: ctx.expand_location(value, ctx.attr.data) for key, value in ctx.attr.env.items()}
    return RunEnvironmentInfo(environment = env, inherited_environment = ctx.attr.env_inherit)

def _instrumented(ctx, source_attributes = None):
    return coverage_common.instrumented_files_info(
        ctx,
        source_attributes = source_attributes or ["srcs"],
        dependency_attributes = ["deps", "data"],
        extensions = ["php"],
    )

def _php_library_impl(ctx):
    srcs = ctx.files.srcs + [f for f in ctx.files.autoload_files if f not in ctx.files.srcs]
    info = _php_info(ctx, srcs)
    return [
        info,
        DefaultInfo(files = depset(srcs), runfiles = _runfiles(ctx, srcs, [])),
        OutputGroupInfo(_validation = depset(_lint(ctx, srcs))),
        _instrumented(ctx),
    ]

def _program_srcs(ctx):
    srcs = list(ctx.files.srcs)
    for f in ctx.files.main:
        if f not in srcs:
            srcs.insert(0, f)
    return srcs

def _php_binary_impl(ctx):
    srcs = _program_srcs(ctx)
    info = _php_info(ctx, srcs)
    tree = _autoload(ctx, info, autoloader_suffix(ctx.label))
    root = runfiles_path(ctx.workspace_name, tree)
    tail = '-d "auto_prepend_file=$runfiles/{root}/vendor/autoload.php" "$runfiles/{root}/{main}" "$@"'.format(
        root = root,
        main = staged_path(ctx.file.main),
    )
    launcher, php_files = _write_launcher(ctx, tail)
    return [
        info,
        DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = _runfiles(ctx, [launcher, tree], [php_files])),
        OutputGroupInfo(_validation = depset(_lint(ctx, srcs)), php_autoload = depset([tree])),
        _environment(ctx),
        _instrumented(ctx, ["main", "srcs"]),
    ]

def _php_test_impl(ctx):
    srcs = list(ctx.files.srcs)
    info = _php_info(ctx, srcs)
    tree = _autoload(ctx, info, autoloader_suffix(ctx.label))
    root = runfiles_path(ctx.workspace_name, tree)
    phpunit = ctx.file._phpunit
    config = '--configuration "$runfiles/{}"'.format(runfiles_path(ctx.workspace_name, ctx.file.config)) if ctx.file.config else "--no-configuration"
    tests = " ".join(['"$runfiles/{}/{}"'.format(root, staged_path(f)) for f in srcs])
    tail = (
        '"$runfiles/{phpunit}" --bootstrap "$runfiles/{root}/vendor/autoload.php" {config} ' +
        "--fail-on-empty-test-suite --do-not-cache-test-index " +
        '${{XML_OUTPUT_FILE:+--log-junit "$XML_OUTPUT_FILE"}} {tests} "$@"'
    ).format(phpunit = runfiles_path(ctx.workspace_name, phpunit), root = root, config = config, tests = tests)
    launcher, php_files = _write_launcher(ctx, tail)
    extra = [phpunit] + ([ctx.file.config] if ctx.file.config else [])
    return [
        info,
        DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = _runfiles(ctx, [launcher, tree] + extra, [php_files])),
        OutputGroupInfo(_validation = depset(_lint(ctx, srcs)), php_autoload = depset([tree])),
        _environment(ctx),
        _instrumented(ctx),
    ]

_COMMON_ATTRS = {
    "data": attr.label_list(allow_files = True, doc = "Files the code reads at run time; they land in runfiles."),
    "deps": attr.label_list(providers = [PhpInfo], doc = "php_library targets whose classes this target loads."),
    "_tool": attr.label(default = _TOOL, allow_single_file = True, cfg = "exec", doc = "The build-time helper script."),
}

_RUN_ATTRS = _COMMON_ATTRS | {
    "env": attr.string_dict(doc = "Environment for `bazel run` and `bazel test`; values expand $(location) of data."),
    "env_inherit": attr.string_list(doc = "Environment variable names inherited from the caller."),
    "ini": attr.string_dict(doc = "php.ini settings passed as -d flags; PHP otherwise runs with -n and no php.ini."),
    "_composer": attr.label(default = _COMPOSER, allow_single_file = True, cfg = "exec", doc = "The pinned Composer phar."),
}

php_library = rule(
    implementation = _php_library_impl,
    doc = "A set of PHP sources other PHP targets load through the Composer classmap autoloader.",
    attrs = _COMMON_ATTRS | {
        "autoload_files": attr.label_list(allow_files = PHP_SOURCE_EXTS, doc = "Sources required on every run, like Composer's `files` autoload."),
        "srcs": attr.label_list(allow_files = PHP_SOURCE_EXTS, doc = "PHP sources; every class in them joins the classmap."),
    },
    provides = [PhpInfo],
    toolchains = [EXEC_TOOLCHAIN_TYPE],
)

php_binary = rule(
    implementation = _php_binary_impl,
    doc = "Runs one PHP script on the pinned PHP with the classes of its deps autoloaded.",
    attrs = _RUN_ATTRS | {
        "main": attr.label(mandatory = True, allow_single_file = PHP_SOURCE_EXTS, doc = "The script PHP runs."),
        "srcs": attr.label_list(allow_files = PHP_SOURCE_EXTS, doc = "More PHP sources of this binary."),
    },
    executable = True,
    toolchains = [EXEC_TOOLCHAIN_TYPE, TOOLCHAIN_TYPE],
)

php_test = rule(
    implementation = _php_test_impl,
    doc = "Runs PHPUnit test files on the pinned PHP and PHPUnit; an empty suite fails.",
    attrs = _RUN_ATTRS | {
        "config": attr.label(allow_single_file = [".xml", ".dist"], doc = "A phpunit.xml; without it PHPUnit runs with --no-configuration."),
        "srcs": attr.label_list(mandatory = True, allow_files = PHP_SOURCE_EXTS, doc = "PHPUnit test files PHPUnit runs."),
        "_phpunit": attr.label(default = _PHPUNIT, allow_single_file = True, doc = "The pinned PHPUnit phar."),
    },
    test = True,
    toolchains = [EXEC_TOOLCHAIN_TYPE, TOOLCHAIN_TYPE],
)

def _php_autoload_tree_impl(ctx):
    info = _php_info(ctx, [])
    return [DefaultInfo(files = depset([_autoload(ctx, info, ctx.attr.suffix)]))]

php_autoload_tree = rule(
    implementation = _php_autoload_tree_impl,
    doc = "The staged autoload tree of some deps under a fixed autoloader suffix.",
    attrs = _COMMON_ATTRS | {
        "suffix": attr.string(mandatory = True, doc = "Composer autoloader suffix."),
        "_composer": attr.label(default = _COMPOSER, allow_single_file = True, cfg = "exec", doc = "The pinned Composer phar."),
    },
    toolchains = [EXEC_TOOLCHAIN_TYPE],
)

def _php_composer_package_impl(ctx):
    deps = [dep[PhpInfo] for dep in ctx.attr.deps]
    package = struct(
        name = ctx.attr.package,
        version = ctx.attr.version,
        manifest = ctx.file.manifest,
        root = ctx.label.workspace_root + ("/" + ctx.label.package if ctx.label.package else ""),
    )
    return [
        PhpInfo(
            autoload_files = depset(transitive = [info.autoload_files for info in deps]),
            package_files = depset(ctx.files.srcs + [ctx.file.manifest], transitive = [info.package_files for info in deps]),
            packages = depset([package], transitive = [info.packages for info in deps]),
            transitive_srcs = depset(transitive = [info.transitive_srcs for info in deps]),
        ),
        DefaultInfo(files = depset([ctx.file.manifest])),
    ]

php_composer_package = rule(
    implementation = _php_composer_package_impl,
    doc = "One Composer package fetched from composer.lock. Its files stay out of the classmap; Composer autoloads them from installed.json.",
    attrs = {
        "deps": attr.label_list(providers = [PhpInfo], doc = "The packages this package requires."),
        "manifest": attr.label(mandatory = True, allow_single_file = [".json"], doc = "The filtered composer.lock entry staged into vendor/composer/installed.json."),
        "package": attr.string(mandatory = True, doc = "The Composer package name, vendor/name."),
        "srcs": attr.label_list(allow_files = True, doc = "Every file of the package."),
        "version": attr.string(mandatory = True, doc = "The locked package version."),
    },
    provides = [PhpInfo],
)

def _php_composer_group_impl(ctx):
    deps = [dep[PhpInfo] for dep in ctx.attr.deps]
    return [
        PhpInfo(
            autoload_files = depset(transitive = [info.autoload_files for info in deps]),
            package_files = depset(transitive = [info.package_files for info in deps]),
            packages = depset(transitive = [info.packages for info in deps]),
            transitive_srcs = depset(transitive = [info.transitive_srcs for info in deps]),
        ),
        DefaultInfo(),
    ]

php_composer_group = rule(
    implementation = _php_composer_group_impl,
    doc = "Several Composer packages under one label.",
    attrs = {
        "deps": attr.label_list(providers = [PhpInfo], doc = "The packages in the group."),
    },
    provides = [PhpInfo],
)
