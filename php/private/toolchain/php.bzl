"""The PHP toolchain rule."""

TOOLCHAIN_TYPE = Label("//php/private/toolchain:toolchain_type")

EXEC_TOOLCHAIN_TYPE = Label("//php/private/toolchain:exec_toolchain_type")

def _php_toolchain_impl(ctx):
    interpreter = ctx.file.interpreter
    files = depset([interpreter])
    return [platform_common.ToolchainInfo(
        php = struct(
            interpreter = interpreter,
            files = files,
            coverage = struct(
                extensions = ctx.files.coverage_extensions,
                files = depset(ctx.files.coverage_files) if ctx.attr.coverage_interpreter else files,
                interpreter = ctx.file.coverage_interpreter or interpreter,
            ),
            pcov = ctx.file.pcov,
            version = ctx.attr.version,
        ),
    )]

php_toolchain = rule(
    implementation = _php_toolchain_impl,
    doc = "Exposes one PHP interpreter, the files it needs and its version.",
    attrs = {
        "coverage_extensions": attr.label_list(
            allow_files = True,
            doc = "Shared extensions the coverage interpreter loads before PCOV.",
        ),
        "coverage_files": attr.label(
            allow_files = True,
            doc = "Every file the coverage interpreter needs at run time.",
        ),
        "coverage_interpreter": attr.label(
            allow_single_file = True,
            doc = "The php executable that loads PCOV, when it is not the interpreter.",
        ),
        "interpreter": attr.label(
            mandatory = True,
            allow_single_file = True,
            doc = "The php executable.",
        ),
        "pcov": attr.label(
            allow_single_file = True,
            doc = "The PCOV extension built for this interpreter, if it can load one.",
        ),
        "version": attr.string(mandatory = True, doc = "PHP version of the interpreter."),
    },
)
