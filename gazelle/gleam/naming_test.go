package gleam

import "testing"

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
