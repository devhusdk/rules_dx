"""Forwards an executable built for the execution platform, linked by the plain rules_rust linker."""

_LINKER_WRAPPER = "@rules_rust//rust/settings:toolchain_linker_wrapper"

def _plain_linker_impl(_settings, _attr):
    return {_LINKER_WRAPPER: Label("@rules_rust//rust/settings:no_toolchain_linker_wrapper")}

# The wrapper flag reaches exec configurations, so the tool that is the wrapper must link without it.
_plain_linker = transition(
    implementation = _plain_linker_impl,
    inputs = [],
    outputs = [_LINKER_WRAPPER],
)

def _exec_tool_impl(ctx):
    tool = ctx.attr.tool[DefaultInfo]
    return [DefaultInfo(
        files = depset([tool.files_to_run.executable]),
        runfiles = tool.default_runfiles,
    )]

exec_tool = rule(
    implementation = _exec_tool_impl,
    doc = "Exposes `tool` built for the execution platform without the rules_rust linker wrapper.",
    cfg = _plain_linker,
    attrs = {
        "tool": attr.label(doc = "Executable to build for the execution platform.", cfg = "exec", executable = True, mandatory = True),
    },
)
