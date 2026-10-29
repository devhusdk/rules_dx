package common

import (
	"os"
	"path/filepath"
	"testing"
)

func TestIsErlangNifDir(t *testing.T) {
	root := t.TempDir()
	write := func(rel string) {
		p := filepath.Join(root, rel)
		if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(p, nil, 0o644); err != nil {
			t.Fatal(err)
		}
	}
	write("rebar/rebar.config")
	write("rebar/c_src/a.c")
	write("rebar/c_src/vendor/b.c")
	write("appsrc/src/demo.app.src")
	write("appsrc/c_src/a.c")
	write("plain/c_src/a.c")
	write("c_src/a.c")
	cases := map[string]bool{
		"rebar/c_src":        true,
		"rebar/c_src/vendor": true,
		"appsrc/c_src":       true,
		"rebar":              false,
		"plain/c_src":        false,
		"c_src":              false,
		"":                   false,
	}
	for rel, want := range cases {
		if got := IsErlangNifDir(root, rel); got != want {
			t.Errorf("IsErlangNifDir(%q) = %v, want %v", rel, got, want)
		}
	}
}
