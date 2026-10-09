"""Raw wasm32 platform transition for one dependency edge."""

def _raw_wasm_transition_impl(_settings, _attr):
    """Selects the raw wasm32 platform for the wrapped target."""
    return {"//command_line_option:platforms": ["@rules_rust//rust/platform:wasm32"]}

raw_wasm_transition = transition(
    implementation = _raw_wasm_transition_impl,
    inputs = [],
    outputs = ["//command_line_option:platforms"],
)

def _raw_wasm_proxy_impl(ctx):
    """Forwards the wasm-configured binary file into this configuration."""
    inner = ctx.attr.target[0] if type(ctx.attr.target) == "list" else ctx.attr.target
    return [DefaultInfo(files = inner[DefaultInfo].files)]

raw_wasm_proxy = rule(
    implementation = _raw_wasm_proxy_impl,
    attrs = {"target": attr.label(mandatory = True, cfg = raw_wasm_transition)},
)
