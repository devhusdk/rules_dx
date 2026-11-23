package common

import (
	"os"
	"path/filepath"
	"strings"
)

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

func AppSrcFile(dir string) string {
	entries, _ := os.ReadDir(filepath.Join(dir, "src"))
	for _, e := range entries {
		if !e.IsDir() && strings.HasSuffix(e.Name(), ".app.src") {
			return filepath.Join(dir, "src", e.Name())
		}
	}
	return ""
}
