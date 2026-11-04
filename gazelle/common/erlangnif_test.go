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

func TestErlangAppWithGlobCharsInRoot(t *testing.T) {
	root := t.TempDir()
	for _, name := range []string{"app[1]", "a*b"} {
		dir := filepath.Join(root, name)
		for _, rel := range []string{"demo/src/demo.app.src", "demo/c_src/a.c"} {
			p := filepath.Join(dir, rel)
			if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
				t.Fatal(err)
			}
			if err := os.WriteFile(p, nil, 0o644); err != nil {
				t.Fatal(err)
			}
		}
		if !IsErlangNifDir(dir, "demo/c_src") {
			t.Errorf("%s: IsErlangNifDir(demo/c_src) = false, want true", name)
		}
		want := filepath.Join(dir, "demo", "src", "demo.app.src")
		if got := AppSrcFile(filepath.Join(dir, "demo")); got != want {
			t.Errorf("%s: AppSrcFile = %q, want %q", name, got, want)
		}
	}
}
