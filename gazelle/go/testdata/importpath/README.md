# Go import path resolution

Verifies a generated `go_library` resolves by its `importpath`, whatever its
source files are named. Packages that share a last path element (`a/lib`,
`b/lib`) resolve independently. External imports use exact
`# gazelle:resolve` mappings.
