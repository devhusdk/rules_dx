"""Unrelated integration hint for aspect_hints coexistence fixtures."""

ForeignHintInfo = provider(
    doc = "Unrelated integration metadata rules_dx does not own.",
    fields = {
        "tag": "str: opaque caller tag.",
    },
)

def _foreign_hint_impl(ctx):
    return [ForeignHintInfo(tag = ctx.attr.tag)]

foreign_hint = rule(
    implementation = _foreign_hint_impl,
    doc = "Produces an unrelated hint provider for an aspect_hints entry.",
    attrs = {
        "tag": attr.string(
            doc = "Opaque caller tag.",
            default = "",
        ),
    },
)
