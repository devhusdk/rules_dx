package unit_test

import (
	"bytes"
	"encoding/json"
	"go/ast"
	"os"
	"path/filepath"
	"rules_dx/go/tools/analysis/unit"
	"strings"
	"testing"

	"golang.org/x/tools/go/analysis"
)

var badFunc = &analysis.Analyzer{
	Name: "badfunc",
	Doc:  "reports functions named Bad",
	Run: func(pass *analysis.Pass) (any, error) {
		for _, file := range pass.Files {
			for _, decl := range file.Decls {
				if fn, ok := decl.(*ast.FuncDecl); ok && strings.HasPrefix(fn.Name.Name, "Bad") {
					pass.Reportf(fn.Pos(), "function Bad is forbidden")
				}
			}
		}
		return nil, nil
	},
}

var worseFunc = &analysis.Analyzer{
	Name: "worsefunc",
	Doc:  "reports functions named Worse",
	Run: func(pass *analysis.Pass) (any, error) {
		for _, file := range pass.Files {
			for _, decl := range file.Decls {
				if fn, ok := decl.(*ast.FuncDecl); ok && strings.HasPrefix(fn.Name.Name, "Worse") {
					pass.Reportf(fn.Pos(), "function Worse is forbidden")
				}
			}
		}
		return nil, nil
	},
}

func hasFlag(extra []string, flag string) bool {
	for _, arg := range extra {
		if arg == flag {
			return true
		}
	}
	return false
}

var dropLines bool

var options = unit.Options{
	Configure: func(extra []string) error {
		dropLines = hasFlag(extra, "-drop-lines")
		return nil
	},
	Select: func(extra []string) []*analysis.Analyzer {
		if hasFlag(extra, "-only-worse") {
			return []*analysis.Analyzer{worseFunc}
		}
		return []*analysis.Analyzer{badFunc, worseFunc}
	},
	Filter: func(cfg *unit.Config, tree unit.Tree) unit.Tree {
		if dropLines {
			delete(tree[cfg.ImportPath], "worsefunc")
		}
		return tree
	},
}

func TestMain(m *testing.M) {
	if os.Getenv(unit.ChildEnv) != "" {
		os.Exit(unit.Run("badfunc", []*analysis.Analyzer{badFunc}, options, os.Args[1:], os.Stdout, os.Stderr))
	}
	os.Exit(m.Run())
}

type report map[string]map[string]json.RawMessage

func write(t *testing.T, dir, name, body string) string {
	t.Helper()
	path := filepath.Join(dir, name)
	if err := os.WriteFile(path, []byte(body), 0o644); err != nil {
		t.Fatal(err)
	}
	return path
}

func runUnit(t *testing.T, cfg unit.Config, extra ...string) (report, string) {
	t.Helper()
	dir := t.TempDir()
	cfg.ImportPath = "example.com/p"
	cfg.GOOS = "linux"
	cfg.GOARCH = "amd64"
	raw, err := json.Marshal(cfg)
	if err != nil {
		t.Fatal(err)
	}
	cfgPath := write(t, dir, "cfg.json", string(raw))
	var out, errOut bytes.Buffer
	code := unit.Run("badfunc", []*analysis.Analyzer{badFunc}, options, append([]string{cfgPath}, extra...), &out, &errOut)
	if code != 0 {
		t.Fatalf("exit %d: %s", code, errOut.String())
	}
	var got report
	if err := json.Unmarshal(out.Bytes(), &got); err != nil {
		t.Fatalf("bad report %q: %v", out.String(), err)
	}
	return got, out.String()
}

func TestReportsFindingsAndFiltersOwnedFiles(t *testing.T) {
	dir := t.TempDir()
	owned := write(t, dir, "a.go", "package p\n\nfunc Bad() {}\n")
	other := write(t, dir, "c.go", "package p\n\nfunc Bad2() {}\n")
	got, _ := runUnit(t, unit.Config{GoFiles: []string{owned, other}, OwnedFiles: []string{owned}})
	var list []map[string]string
	if err := json.Unmarshal(got["example.com/p"]["badfunc"], &list); err != nil {
		t.Fatalf("%s: %v", got["example.com/p"]["badfunc"], err)
	}
	if len(list) != 1 || !strings.HasPrefix(list[0]["posn"], owned+":") {
		t.Fatalf("want one finding in %s, got %v", owned, list)
	}
}

func TestFiltersByBuildConstraints(t *testing.T) {
	dir := t.TempDir()
	plain := write(t, dir, "a.go", "package p\n")
	suffix := write(t, dir, "a_windows.go", "package p\n\nfunc Bad() {}\n")
	tagged := write(t, dir, "b.go", "//go:build special\n\npackage p\n\nfunc Bad() {}\n")
	got, raw := runUnit(t, unit.Config{GoFiles: []string{plain, suffix, tagged}})
	if _, ok := got["example.com/p"]["badfunc"]; ok {
		t.Fatalf("filtered files were analyzed: %s", raw)
	}
	got, _ = runUnit(t, unit.Config{GoFiles: []string{plain, tagged}, Tags: []string{"special"}})
	if _, ok := got["example.com/p"]["badfunc"]; !ok {
		t.Fatal("tagged file was not analyzed")
	}
}

func TestChildFailureBecomesError(t *testing.T) {
	dir := t.TempDir()
	broken := write(t, dir, "a.go", "package p\n\nfunc Bad( {\n")
	got, _ := runUnit(t, unit.Config{GoFiles: []string{broken}})
	var fail struct{ Error string }
	if err := json.Unmarshal(got["example.com/p"]["badfunc"], &fail); err != nil || fail.Error == "" {
		t.Fatalf("want error entry, got %s (%v)", got["example.com/p"]["badfunc"], err)
	}
}

func TestNoFilesIsEmptyReport(t *testing.T) {
	dir := t.TempDir()
	vetx := filepath.Join(dir, "vetx")
	got, _ := runUnit(t, unit.Config{VetxOutput: vetx})
	if len(got["example.com/p"]) != 0 {
		t.Fatalf("want empty report, got %v", got)
	}
	if _, err := os.Stat(vetx); err != nil {
		t.Fatal("vetx output was not created")
	}
}

func TestSelectDropsAnalyzer(t *testing.T) {
	dir := t.TempDir()
	file := write(t, dir, "a.go", "package p\n\nfunc Bad() {}\n\nfunc Worse() {}\n")
	got, _ := runUnit(t, unit.Config{GoFiles: []string{file}})
	if _, ok := got["example.com/p"]["badfunc"]; !ok {
		t.Fatalf("default set lost badfunc: %v", got)
	}
	got, _ = runUnit(t, unit.Config{GoFiles: []string{file}}, "-only-worse")
	if _, ok := got["example.com/p"]["badfunc"]; ok {
		t.Fatalf("Select did not drop badfunc: %v", got)
	}
	if _, ok := got["example.com/p"]["worsefunc"]; !ok {
		t.Fatalf("Select dropped worsefunc: %v", got)
	}
}

func TestFilterRemovesDiagnostics(t *testing.T) {
	dir := t.TempDir()
	file := write(t, dir, "a.go", "package p\n\nfunc Bad() {}\n\nfunc Worse() {}\n")
	got, _ := runUnit(t, unit.Config{GoFiles: []string{file}}, "-drop-lines")
	if _, ok := got["example.com/p"]["worsefunc"]; ok {
		t.Fatalf("Filter kept worsefunc: %v", got)
	}
	if _, ok := got["example.com/p"]["badfunc"]; !ok {
		t.Fatalf("Filter dropped badfunc: %v", got)
	}
}
