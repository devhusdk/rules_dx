"""Unrelated integration hint for aspect_hints coexistence."""

ForeignHintInfo = provider(
    doc = "Marker an unrelated integration publishes through aspect_hints.",
    fields = {
        "marker": "str: opaque payload rules_dx never interprets.",
    },
)

def _foreign_hint_impl(ctx):
    return [
        DefaultInfo(files = depset([])),
        ForeignHintInfo(marker = ctx.attr.marker),
    ]

foreign_hint = rule(
    implementation = _foreign_hint_impl,
    attrs = {
        "marker": attr.string(
            default = "foreign",
        ),
    },
)
