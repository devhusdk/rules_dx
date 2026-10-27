"""The PHP toolchain rule."""

TOOLCHAIN_TYPE = Label("//php/toolchain:toolchain_type")

EXEC_TOOLCHAIN_TYPE = Label("//php/toolchain:exec_toolchain_type")

def _php_toolchain_impl(ctx):
    interpreter = ctx.file.interpreter
    return [platform_common.ToolchainInfo(
        php = struct(
            interpreter = interpreter,
            files = depset([interpreter]),
            version = ctx.attr.version,
        ),
    )]

php_toolchain = rule(
    implementation = _php_toolchain_impl,
    doc = "Exposes one PHP interpreter, the files it needs and its version.",
    attrs = {
        "interpreter": attr.label(
            mandatory = True,
            allow_single_file = True,
            doc = "The php executable.",
        ),
        "version": attr.string(mandatory = True, doc = "PHP version of the interpreter."),
    },
)
