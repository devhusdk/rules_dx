# Kotlin merge and cleanup

Verifies generated attribute refresh, user-owned attribute preservation,
and conservative stale-rule cleanup per `gazelle/kotlin/lang.go`.
The handwritten `demo_test` keeps its `size` while generation refreshes its
`test_class` and replaces the stale dependency with the library.
