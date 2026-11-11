# Qt kinds from Qt includes

Verifies that a library or test whose source has a system include starting with
a `Qt` module directory, such as `<QtCore/QString>`, is generated as
`qt_cc_library` or `qt_cc_test` loaded from `@rules_dx//qt:defs.bzl`.

The `qt` directory gets the Qt kinds and the `plain` directory keeps `cc_library`
and `cc_test`. The `rerun` directory already holds the generated Qt rules and
stays unchanged.

The `flip` directory holds `cc_library` and `cc_test` over Qt sources and ends with
only `qt_cc_library` and `qt_cc_test`.
