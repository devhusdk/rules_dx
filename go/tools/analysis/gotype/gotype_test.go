package gotype

import (
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func write(t *testing.T, dir, name, body string) string {
	t.Helper()
	path := filepath.Join(dir, name)
	if err := os.WriteFile(path, []byte(body), 0o644); err != nil {
		t.Fatal(err)
	}
	return path
}

func base(files ...string) *config {
	return &config{ImportPath: "example.com/p", GoFiles: files, GOOS: "linux", GOARCH: "amd64", GoVersion: "go1.22", Cgo: false}
}

func rules(res result) map[string]int {
	out := map[string]int{}
	for rule, list := range res.diagnostics {
		out[rule] = len(list)
	}
	return out
}

func TestCleanPackage(t *testing.T) {
	dir := t.TempDir()
	res := check(base(write(t, dir, "a.go", "package p\n\nfunc F() int { return 1 }\n")))
	if res.failure != "" || len(res.diagnostics) != 0 {
		t.Fatalf("expected clean, got %+v", res)
	}
}

func TestAllTypeErrorsReported(t *testing.T) {
	dir := t.TempDir()
	res := check(base(write(t, dir, "a.go", "package p\n\nvar a int = \"x\"\nvar b string = 1\nfunc F() { undefined() }\n")))
	if rules(res)["typecheck"] != 3 {
		t.Fatalf("expected 3 typecheck diagnostics, got %+v", res)
	}
	if !strings.HasSuffix(res.diagnostics["typecheck"][0].Posn, "a.go:3:13") {
		t.Fatalf("unexpected posn %q", res.diagnostics["typecheck"][0].Posn)
	}
}

func TestSyntaxErrorsSkipTypecheck(t *testing.T) {
	dir := t.TempDir()
	res := check(base(write(t, dir, "a.go", "package p\n\nfunc F( {\n"), write(t, dir, "b.go", "package p\n\nvar a int = \"x\"\n")))
	got := rules(res)
	if got["syntax"] == 0 || got["typecheck"] != 0 {
		t.Fatalf("expected syntax only, got %+v", res)
	}
}

func TestBuildTagExcludedFileIsIgnored(t *testing.T) {
	dir := t.TempDir()
	good := write(t, dir, "a.go", "package p\n")
	bad := write(t, dir, "b.go", "//go:build special\n\npackage p\n\nvar a int = \"x\"\n")
	if res := check(base(good, bad)); len(res.diagnostics) != 0 || res.failure != "" {
		t.Fatalf("excluded file leaked: %+v", res)
	}
	cfg := base(good, bad)
	cfg.Tags = []string{"special"}
	if res := check(cfg); rules(res)["typecheck"] != 1 {
		t.Fatalf("tag not honoured: %+v", res)
	}
}

func TestGOOSSuffixExcluded(t *testing.T) {
	dir := t.TempDir()
	good := write(t, dir, "a.go", "package p\n")
	bad := write(t, dir, "b_windows.go", "package p\n\nvar a int = \"x\"\n")
	if res := check(base(good, bad)); len(res.diagnostics) != 0 {
		t.Fatalf("windows file leaked: %+v", res)
	}
}

func TestExternalTestPackageSeesInternalSymbols(t *testing.T) {
	dir := t.TempDir()
	lib := write(t, dir, "a.go", "package p\n\nfunc F() int { return 1 }\n")
	ok := write(t, dir, "a_test.go", "package p_test\n\nimport p \"example.com/p\"\n\nvar _ = p.F()\n")
	if res := check(base(lib, ok)); len(res.diagnostics) != 0 || res.failure != "" {
		t.Fatalf("expected clean, got %+v", res)
	}
	bad := write(t, dir, "b_test.go", "package p_test\n\nimport p \"example.com/p\"\n\nvar _ = p.Missing()\n")
	if res := check(base(lib, ok, bad)); rules(res)["typecheck"] != 1 {
		t.Fatalf("expected one typecheck error, got %+v", res)
	}
}

func TestCgoImportIsFaked(t *testing.T) {
	dir := t.TempDir()
	res := check(base(write(t, dir, "a.go", "package p\n\n// int f(void);\nimport \"C\"\n\nvar _ = C.f\n")))
	if res.failure != "" || rules(res)["typecheck"] != 0 {
		t.Fatalf("cgo should be faked, got %+v", res)
	}
}

func TestMissingExportDataIsFailure(t *testing.T) {
	dir := t.TempDir()
	res := check(base(write(t, dir, "a.go", "package p\n\nimport \"example.com/dep\"\n\nvar _ = dep.X\n")))
	if res.failure == "" {
		t.Fatalf("expected failure, got %+v", res)
	}
}

func TestUnownedFileIsFailure(t *testing.T) {
	dir := t.TempDir()
	cfg := base(write(t, dir, "a.go", "package p\n\nvar a int = \"x\"\n"))
	cfg.OwnedFiles = []string{filepath.Join(dir, "other.go")}
	if res := check(cfg); res.failure == "" {
		t.Fatalf("expected failure, got %+v", res)
	}
}

func TestRunWritesReportAndRejectsBadConfig(t *testing.T) {
	dir := t.TempDir()
	src := write(t, dir, "a.go", "package p\n\nvar a int = \"x\"\n")
	cfg := write(t, dir, "cfg.json", `{"ImportPath":"example.com/p","GoFiles":["`+src+`"],"GOOS":"linux","GOARCH":"amd64"}`)
	out := filepath.Join(dir, "out.json")
	if err := run(cfg, out); err != nil {
		t.Fatal(err)
	}
	raw, _ := os.ReadFile(out)
	if !strings.Contains(string(raw), `"typecheck":[{"posn"`) || !strings.Contains(string(raw), `"example.com/p"`) {
		t.Fatalf("unexpected report %s", raw)
	}
	if err := run(write(t, dir, "bad.json", "{"), out); err == nil {
		t.Fatal("expected config error")
	}
}
