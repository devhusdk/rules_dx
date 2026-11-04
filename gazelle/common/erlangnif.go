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
		if AppSrcFile(parent) != "" {
			return true
		}
	}
	return false
}

// AppSrcFile returns the first src/*.app.src under dir in name order, or "" if
// there is none. It lists the directory instead of globbing it, so glob
// characters in dir, such as "app[1]", are matched literally.
func AppSrcFile(dir string) string {
	entries, _ := os.ReadDir(filepath.Join(dir, "src"))
	for _, e := range entries {
		if !e.IsDir() && strings.HasSuffix(e.Name(), ".app.src") {
			return filepath.Join(dir, "src", e.Name())
		}
	}
	return ""
}
