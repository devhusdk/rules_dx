# Entry naming and package-level generation

Verifies a `main.go` source in a library package stays an ordinary
library source (a library directory is never split into a binary), and `helper_test.go` is owned
by the package-level `go_test` via `embed`. The `go_library` `importpath`
derives from the nearest enclosing `go.mod` (`example.com/entries` plus
the subpath).
