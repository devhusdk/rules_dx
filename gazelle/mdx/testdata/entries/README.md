# Entry naming and one-source generation

Verifies `main.mdx` and `helper_test.mdx` are ordinary one-source
`mdx_library` targets like any other source (tests and entries stay
JavaScript-owned per `gazelle/common/webcomponent.go`); no test or thin-binary
inference exists here.
