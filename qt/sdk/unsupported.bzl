"""Qt targets the pinned runtime closure cannot serve."""

def _qt_unsupported_impl(ctx):
    fail("rules_dx: Qt target '" + ctx.label.name + "' is unsupported: " + ctx.attr.reason)

qt_unsupported = rule(
    implementation = _qt_unsupported_impl,
    doc = "Fails analysis with the reason a Qt target is unavailable.",
    attrs = {
        "reason": attr.string(doc = "Why the target is unavailable.", mandatory = True),
    },
)
