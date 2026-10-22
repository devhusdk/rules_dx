"""qmlformat check plus fix wiring (QML format)."""

QMLFORMAT_QT_OBSERVED = "Qt 6.11.3"

QMLFORMAT_ARTIFACT = "@dx_qt//:qmlformat"
QMLFORMAT_RUNTIME = "pinned Qt runtime closure from //qt/sdk:pins.bzl"

QMLFORMAT_CHECK = "qmlformat to stdout (exit 0; dirty when stdout differs from the input, no --check flag)"
QMLFORMAT_FIX = "qmlformat -i in-place (re-read on exit 0, keep input otherwise)"
QMLFORMAT_CONFIG_POLICY = "upstream built-in defaults without .qmlformat.ini, native interpretation with .qmlformat.ini; no auto-supplied preset"

STRUCTURED_FIXTURE_QMLFORMAT = "//quality/tests/fixtures/qmlformat:corpus_starlark"

QMLFORMAT_REJECTED = "ambient .qmlformat.ini discovery rejected; auto-supplied preset rejected"
