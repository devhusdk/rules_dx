"""qmllint check wiring (QML lint)."""

QMLLINT_QT_OBSERVED = "Qt 6.11.3"

QMLLINT_ARTIFACT = "@dx_qt//:qmllint"

QMLLINT_CHECK = "qmllint -W 0 --json - -I <dirs> (exit 0 clean, exit 255 with revision 4 JSON diagnostics when dirty)"
QMLLINT_CONFIG_POLICY = "--ignore-settings without .qmllint.ini, native interpretation with .qmllint.ini; //qmllint enable/disable scoping native"

STRUCTURED_FIXTURE_QMLLINT = "//quality/tests/fixtures/qmllint:corpus_starlark"

QMLLINT_REJECTED = "console-parse rejected; IN_PLACE patching outside sandbox rejected; ambient .qmllint.ini discovery rejected; auto-supplied preset rejected"
