"""The PHP that loads PCOV and the descriptor dx-runtime's php_coverage helper reads under coverage."""

load("//core:cells.bzl", "cell_setting")
load("//php/private:php.bzl", "descriptor_line", "ini_flags", "rf_prefixed")
load("//php/private/toolchain:php.bzl", "TOOLCHAIN_TYPE")

PhpPcovInfo = provider(
    doc = "A PHP that can load PCOV.",
    fields = {
        "extensions": "Shared extensions the interpreter loads before PCOV.",
        "files": "Depset of every file the interpreter needs at run time.",
        "interpreter": "The php executable.",
        "pcov": "The PCOV extension.",
    },
)

def coverage_descriptor(runner, tree, pcov, php, ini, tokens, extensions = []):
    """Returns the descriptor of a php_test under coverage: dx-runtime running its php_coverage helper with the autoload tree, PCOV, php, its extensions, its flags and the tokens."""
    lines = ["program=" + runner, "env=DX_RUNTIME_TOOL\tphp_coverage", "path=" + tree, "path=" + pcov, "path=" + php, "arg=-n"]
    for extension in extensions:
        lines.extend(["arg=-d", descriptor_line(rf_prefixed("extension=", extension))])
    for flag in ini_flags(ini):
        lines.append("arg=" + flag)
    lines.append("arg=--")
    lines.extend([descriptor_line(token) for token in tokens])
    return "\n".join(lines) + "\n"

def _php_pcov_runtime_impl(ctx):
    if ctx.file.interpreter:
        interpreter = ctx.file.interpreter
        files = depset(ctx.files.files)
    else:
        php = ctx.toolchains[TOOLCHAIN_TYPE].php
        interpreter = php.interpreter
        files = php.files
    return [PhpPcovInfo(
        extensions = ctx.files.extensions,
        files = files,
        interpreter = interpreter,
        pcov = ctx.file.pcov,
    )]

php_pcov_runtime = rule(
    implementation = _php_pcov_runtime_impl,
    doc = "A PHP and the PCOV extension it loads; without interpreter, the PHP of the toolchain.",
    attrs = {
        "extensions": attr.label_list(allow_files = True, doc = "Shared extensions the interpreter loads before PCOV."),
        "files": attr.label(allow_files = True, doc = "Every file the interpreter needs at run time."),
        "interpreter": attr.label(allow_single_file = True, doc = "The php executable, when it is not the toolchain's."),
        "pcov": attr.label(mandatory = True, allow_single_file = True, doc = "The PCOV extension."),
    },
    provides = [PhpPcovInfo],
    toolchains = [TOOLCHAIN_TYPE],
)

def php_pcov_runtimes(name, coverage_cells):
    """Declares `name`: the PCOV runtime of the target cell under coverage, an empty filegroup otherwise; `<name>_static` is incompatible with the coverage cells."""
    visibility = ["//:__subpackages__"]
    cases = {}
    for cell in sorted(coverage_cells):
        repo = "@dx_php_coverage_" + cell
        php_pcov_runtime(
            name = name + "_" + cell,
            extensions = [repo + "//:ext/php_mbstring.dll"],
            files = repo + "//:files",
            interpreter = repo + "//:php.exe",
            pcov = repo + "//:php_pcov.dll",
            visibility = visibility,
        )
        cases[cell_setting("//php/private/toolchain", cell)] = ":" + name + "_" + cell
    php_pcov_runtime(
        name = name + "_static",
        pcov = "@dx_prebuilt//:pcov",
        target_compatible_with = select({
            setting: [Label("@platforms//:incompatible")]
            for setting in cases
        } | {"//conditions:default": []}),
        visibility = visibility,
    )
    cases["//conditions:default"] = ":" + name + "_static"
    native.alias(
        name = name + "_on",
        actual = select(cases),
        visibility = visibility,
    )
    native.filegroup(
        name = name + "_off",
        visibility = visibility,
    )
    native.alias(
        name = name,
        actual = select({
            Label("//core:coverage_on"): ":" + name + "_on",
            "//conditions:default": ":" + name + "_off",
        }),
        visibility = visibility,
    )
