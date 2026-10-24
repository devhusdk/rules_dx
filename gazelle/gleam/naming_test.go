package gleam

import (
	"os"
	"path/filepath"
	"testing"
)

func TestModuleName(t *testing.T) {
	for path, want := range map[string]string{
		"src/app.gleam":         "app",
		"src/app/core.gleam":    "app/core",
		"test/app_test.gleam":   "app_test",
		"test/sub/helper.gleam": "sub/helper",
	} {
		if got := ModuleName(path); got != want {
			t.Errorf("ModuleName(%q) = %q, want %q", path, got, want)
		}
	}
}

func TestSourceClasses(t *testing.T) {
	if !IsLibrarySource("src/a.gleam") || IsLibrarySource("test/a.gleam") || IsLibrarySource("src/a.mjs") {
		t.Error("library sources are src/**/*.gleam")
	}
	if !IsTestSource("test/a_test.gleam") || IsTestSource("test/helper.gleam") || IsTestSource("src/a_test.gleam") {
		t.Error("test sources are test/**/*_test.gleam")
	}
}

func TestStdLibAndCandidates(t *testing.T) {
	if !IsStdLib("gleeunit") || !IsStdLib("gleeunit/should") || IsStdLib("gleam/io") {
		t.Error("only gleeunit modules are implicit")
	}
	if got := Candidates("gleam/io"); len(got) != 1 || got[0] != "gleam_stdlib" {
		t.Errorf("candidates = %v", got)
	}
	if Candidates("app/core") != nil {
		t.Error("package modules have no candidates")
	}
}

func TestRootPackageName(t *testing.T) {
	cases := map[string]string{
		"name = \"app\"\n":                                       "app",
		"# name = \"commented\"\n  name   =   'quoted'  \n":       "quoted",
		"version = \"1.0.0\"\n[dependencies]\nname = \"decoy\"\n": "",
		"name = unquoted\n":                                       "",
		"name = \"open\n":                                          "",
		"names = \"other\"\n":                                     "",
	}
	for content, want := range cases {
		if got := manifestName([]byte(content)); got != want {
			t.Errorf("manifestName(%q) = %q, want %q", content, got, want)
		}
	}
	dir := t.TempDir()
	if got := rootPackageName(dir); got != "" {
		t.Errorf("rootPackageName(no gleam.toml) = %q, want empty", got)
	}
	if err := os.WriteFile(filepath.Join(dir, ManifestFile), []byte("name = \"app\"\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	if got := rootPackageName(dir); got != "app" {
		t.Errorf("rootPackageName = %q, want app", got)
	}
}
