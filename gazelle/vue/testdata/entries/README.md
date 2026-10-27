# Entry naming and one-source generation

Verifies `main.vue` and `helper_test.vue` are ordinary one-source
`vue_library` targets like any other source (tests and entries stay
JavaScript-owned per `gazelle/common/webcomponent.go`); no test or thin-binary
inference exists here.
