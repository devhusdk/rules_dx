"""Subject observation provider and label rendering shared with non-test rules."""

load("//core:canonical.bzl", "strip_canonical")

DxSubjectInfo = provider(
    doc = "Analysis observations a subject rule exposes to starlark_test.",
    fields = {
        "fields": "Observations about the subject.",
    },
)

def display_label(label):
    """Renders a label with the canonical-repository marker stripped."""
    return strip_canonical(str(label))
