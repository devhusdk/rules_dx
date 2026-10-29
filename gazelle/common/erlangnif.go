package common

import (
	"os"
	"path/filepath"
	"strings"
)

// IsErlangNifDir reports whether rel is, or sits under, the c_src directory of
// an Erlang application, found by a rebar.config or src/*.app.src next to it.
// The Erlang generator lists those sources in the app's own package, so other
// languages must not draw a package boundary there. It only stats files and
// keeps no state, so parallel generation may call it freely.
func IsErlangNifDir(root, rel string) bool {
	parts := strings.Split(rel, "/")
	for i, part := range parts {
		if part != "c_src" {
			continue
		}
		parent := filepath.Join(append([]string{root}, parts[:i]...)...)
		if _, err := os.Stat(filepath.Join(parent, "rebar.config")); err == nil {
			return true
		}
		if m, _ := filepath.Glob(filepath.Join(parent, "src", "*.app.src")); len(m) > 0 {
			return true
		}
	}
	return false
}
