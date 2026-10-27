# Entry naming and one-source generation

Verifies `main.astro` and `helper_test.astro` are ordinary one-source
`astro_library` targets like any other source (tests and entries stay
JavaScript-owned per `gazelle/common/webcomponent.go`); no test or thin-binary
inference exists here.
