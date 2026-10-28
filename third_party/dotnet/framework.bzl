"""Pins .NET tool drivers to one target framework, matching the net10.0 lines in paket.dependencies."""

DOTNET_TOOL_FRAMEWORK = "net10.0"

def _pin_impl(_settings, _attr):
    return {"@rules_dotnet//dotnet:target_framework": DOTNET_TOOL_FRAMEWORK}

_pin = transition(
    implementation = _pin_impl,
    inputs = [],
    outputs = ["@rules_dotnet//dotnet:target_framework"],
)

def _framework_pinned_impl(ctx):
    actual = ctx.attr.actual[0][DefaultInfo]
    runfiles = ctx.runfiles(transitive_files = actual.files).merge(actual.default_runfiles)
    return [DefaultInfo(files = actual.files, runfiles = runfiles)]

framework_pinned = rule(
    implementation = _framework_pinned_impl,
    doc = "Forwards a .NET target built for the pinned tool framework, whatever framework the consumer uses.",
    attrs = {
        "actual": attr.label(mandatory = True, cfg = _pin, doc = "The .NET target to forward."),
    },
)
