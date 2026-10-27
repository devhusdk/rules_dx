# Entry naming and one-source generation

Verifies `main.svelte` and `helper_test.svelte` are ordinary one-source
`svelte_library` targets like any other source (tests and entries stay
JavaScript-owned per `gazelle/common/webcomponent.go`); no test or thin-binary
inference exists here.
