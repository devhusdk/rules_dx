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

_BARE = "abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789_./=-"

def lit(text):
    """Returns a launcher token for one literal argument."""
    return ("arg", text)

def rf(key):
    """Returns a launcher token for the absolute path of one runfiles key."""
    return ("path", key)

def rf_prefixed(prefix, key):
    """Returns a launcher token for a prefix followed by the absolute path of one runfiles key."""
    return ("prefixed", prefix, key)

def env_flag(flag, var):
    """Returns a launcher token for a flag and the value of an environment variable, left out when the variable is unset or empty."""
    return ("env_flag", flag, var)

def _bash_word(token):
    kind = token[0]
    if kind == "arg":
        text = token[1]
        return text if text and all([c in _BARE for c in text.elems()]) else shell_quote(text)
    if kind == "path":
        return '"$runfiles/{}"'.format(token[1])
    if kind == "prefixed":
        return '"{}$runfiles/{}"'.format(token[1], token[2])
    if kind == "env_flag":
        return '${{{var}:+{flag} "${var}"}}'.format(flag = token[1], var = token[2])
    fail("unknown launcher token " + kind)

def launcher_script(php, ini, tokens):
    """Returns the bash launcher that runs the runfiles php with -n, the ini flags, the tokens and the user's arguments."""
    flags = "".join([shell_quote(flag) + " " for flag in ini_flags(ini)])
    tail = " ".join([_bash_word(token) for token in tokens] + ['"$@"'])
    return _LAUNCHER.format(body = 'exec "$runfiles/{php}" -n {flags}{tail}\n'.format(php = php, flags = flags, tail = tail))

def _descriptor_line(token):
    kind = token[0]
    for text in token[1:]:
        if "\n" in text or "\r" in text or (kind in ("prefixed", "env_flag") and "\t" in text):
            fail("launcher token " + repr(token) + " cannot hold a line break or tab")
    if kind in ("prefixed", "env_flag"):
        return kind + "=" + token[1] + "\t" + token[2]
    return kind + "=" + token[1]

def launcher_descriptor(php, ini, tokens):
    """Returns the descriptor text the native launcher reads: the same program and arguments as launcher_script."""
    lines = ["version=1", "program=" + php, "arg=-n"]
    for flag in ini_flags(ini):
        lines.append("arg=" + flag)
    lines.extend([_descriptor_line(token) for token in tokens])
    return "\n".join(lines) + "\n"

LAUNCHER_ATTRS = {
    "_exec_launcher": attr.label(default = Label("//libs/launcher:exec_launcher"), executable = True, cfg = "target", doc = "The native launcher used on Windows."),
    "_windows_os": attr.label(default = Label("@platforms//os:windows")),
}

def write_launcher(ctx, php, ini, tokens):
    """Writes the launcher of a php program: a bash script, or the native launcher with a descriptor on Windows.

    Returns the executable and the runfiles the launcher needs beside the php files.
    """
    key = runfiles_path(ctx.workspace_name, php.interpreter)
    if ctx.target_platform_has_constraint(ctx.attr._windows_os[platform_common.ConstraintValueInfo]):
        launcher = ctx.actions.declare_file(ctx.label.name + ".exe")
        descriptor = ctx.actions.declare_file(ctx.label.name + ".launch")
        ctx.actions.write(descriptor, launcher_descriptor(key, ini, tokens))
        ctx.actions.symlink(output = launcher, target_file = ctx.executable._exec_launcher, is_executable = True)
        files = [launcher, descriptor]
        runfiles = ctx.runfiles(files = files).merge(ctx.attr._exec_launcher[DefaultInfo].default_runfiles)
    else:
        launcher = ctx.actions.declare_file(ctx.label.name)
        ctx.actions.write(launcher, launcher_script(key, ini, tokens), is_executable = True)
        runfiles = ctx.runfiles(files = [launcher])
    return launcher, runfiles

def _write_launcher(ctx, tokens):
    """Writes the launcher of a binary or test."""
    php = ctx.toolchains[TOOLCHAIN_TYPE].php
    launcher, runfiles = write_launcher(ctx, php, ctx.attr.ini, tokens)
    return launcher, runfiles.merge(ctx.runfiles(transitive_files = php.files))

def _runfiles(ctx, files, transitive, base = None):
    runfiles = ctx.runfiles(files = files + ctx.files.data, transitive_files = depset(transitive = transitive))
    if base:
        runfiles = runfiles.merge(base)
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
    tokens = [
        lit("-d"),
        rf_prefixed("auto_prepend_file=", root + "/vendor/autoload.php"),
        rf(root + "/" + staged_path(ctx.file.main)),
    ]
    launcher, base = _write_launcher(ctx, tokens)
    return [
        info,
        DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = _runfiles(ctx, [tree], [], base)),
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
    tokens = [rf(runfiles_path(ctx.workspace_name, phpunit)), lit("--bootstrap"), rf(root + "/vendor/autoload.php")]
    if ctx.file.config:
        tokens += [lit("--configuration"), rf(runfiles_path(ctx.workspace_name, ctx.file.config))]
    else:
        tokens.append(lit("--no-configuration"))
    tokens += [lit("--fail-on-empty-test-suite"), lit("--do-not-cache-test-index"), env_flag("--log-junit", "XML_OUTPUT_FILE")]
    tokens += [rf(root + "/" + staged_path(f)) for f in srcs]
    launcher, base = _write_launcher(ctx, tokens)
    extra = [phpunit] + ([ctx.file.config] if ctx.file.config else [])
    return [
        info,
        DefaultInfo(executable = launcher, files = depset([launcher]), runfiles = _runfiles(ctx, [tree] + extra, [], base)),
        OutputGroupInfo(_validation = depset(_lint(ctx, srcs)), php_autoload = depset([tree])),
        _environment(ctx),
        _instrumented(ctx),
    ]

_COMMON_ATTRS = {
    "data": attr.label_list(allow_files = True, doc = "Files the code reads at run time; they land in runfiles."),
    "deps": attr.label_list(providers = [PhpInfo], doc = "php_library targets whose classes this target loads."),
    "_tool": attr.label(default = _TOOL, allow_single_file = True, cfg = "exec", doc = "The build-time helper script."),
}

_RUN_ATTRS = _COMMON_ATTRS | LAUNCHER_ATTRS | {
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
