package rust

import (
	"fmt"
	"sort"
	"strings"
	"testing"
)

func TestDiscoverCrateRoots(t *testing.T) {
	files := []string{
		"tools/parser/src/lib.rs",
		"tools/parser/src/main.rs",
		"tools/parser/src/parser.rs",
		"tools/parser/tests/login.rs",
		"tools/parser/tests/login_test.rs",
		"tools/parser/tests/common/mod.rs",
		"tools/parser/tests/common.rs",
		"tools/parser/src/bin/tool.rs",
		"tools/parser/examples/demo.rs",
		"tools/parser/benches/speed.rs",
		"tools/parser/build.rs",
		"other/src/lib.rs",
	}
	roots := DiscoverCrateRoots("tools/parser", files)
	if roots.LibRoot != "tools/parser/src/lib.rs" {
		t.Errorf("LibRoot = %q", roots.LibRoot)
	}
	if roots.BinRoot != "tools/parser/src/main.rs" {
		t.Errorf("BinRoot = %q", roots.BinRoot)
	}
	want := []string{
		"tools/parser/tests/common.rs",
		"tools/parser/tests/login.rs",
		"tools/parser/tests/login_test.rs",
	}
	if strings.Join(roots.TestRoots, ",") != strings.Join(want, ",") {
		t.Errorf("TestRoots = %v, want %v", roots.TestRoots, want)
	}
	for _, r := range roots.TestRoots {
		if strings.Contains(r, "common/mod") || strings.Contains(r, "src/bin") {
			t.Errorf("non-root classified as root: %s", r)
		}
	}
}

func TestDiscoverNoRoots(t *testing.T) {
	roots := DiscoverCrateRoots("empty", []string{"empty/README.md", "empty/src/helper.rs"})
	if roots.HasRoots() {
		t.Errorf("expected no roots, got %+v", roots)
	}
}

func TestDiscoverRootDirectory(t *testing.T) {
	roots := DiscoverCrateRoots(".", []string{"src/lib.rs", "tests/smoke.rs"})
	if roots.LibRoot != "src/lib.rs" || len(roots.TestRoots) != 1 {
		t.Errorf("roots = %+v", roots)
	}
}

func TestShapeLibAndBin(t *testing.T) {
	roots := CrateRoots{Dir: "tools/parser", LibRoot: "tools/parser/src/lib.rs", BinRoot: "tools/parser/src/main.rs"}
	shape, err := ShapeCrate(roots, false, false)
	if err != nil {
		t.Fatal(err)
	}
	if shape.Name != "parser" || shape.LibTarget != "parser" || shape.BinTarget != "parser_bin" {
		t.Errorf("shape = %+v", shape)
	}
	if len(shape.Tests) != 0 {
		t.Errorf("tests = %+v", shape.Tests)
	}
}

func TestShapeBinOnlyOwnsName(t *testing.T) {
	roots := CrateRoots{Dir: "tools/tool", BinRoot: "tools/tool/src/main.rs"}
	shape, err := ShapeCrate(roots, false, false)
	if err != nil {
		t.Fatal(err)
	}
	if shape.BinTarget != "tool" || shape.LibTarget != "" {
		t.Errorf("shape = %+v", shape)
	}
}

func TestShapeIntegrationTests(t *testing.T) {
	roots := CrateRoots{
		Dir:       "crates/app",
		LibRoot:   "crates/app/src/lib.rs",
		TestRoots: []string{"crates/app/tests/login.rs", "crates/app/tests/login_test.rs"},
	}
	_, err := ShapeCrate(roots, false, false)
	if err == nil {
		t.Error("expected collision between login.rs and login_test.rs")
	} else if !strings.Contains(err.Error(), "login_test") {
		t.Errorf("wrong error: %v", err)
	}
}

func TestShapeUnitTestCollisionFails(t *testing.T) {
	roots := CrateRoots{
		Dir:       "crates/app",
		LibRoot:   "crates/app/src/lib.rs",
		TestRoots: []string{"crates/app/tests/app.rs"},
	}
	if _, err := ShapeCrate(roots, true, false); err == nil {
		t.Error("expected unit/integration collision on app_test")
	} else if !strings.Contains(err.Error(), "app_test") {
		t.Errorf("wrong error: %v", err)
	}
}

func TestShapeUnitTestNoSyntaxNoTarget(t *testing.T) {
	roots := CrateRoots{Dir: "crates/app", LibRoot: "crates/app/src/lib.rs"}
	shape, err := ShapeCrate(roots, false, false)
	if err != nil {
		t.Fatal(err)
	}
	if shape.LibUnitTest || shape.BinUnitTest {
		t.Errorf("no syntax must mean no unit-test wrapper: %+v", shape)
	}
	shape, err = ShapeCrate(roots, true, false)
	if err != nil {
		t.Fatal(err)
	}
	if !shape.LibUnitTest {
		t.Errorf("recognized syntax must yield one wrapper: %+v", shape)
	}
}

func TestShapeRootDirNeedsCargo(t *testing.T) {
	roots := CrateRoots{Dir: "", LibRoot: "src/lib.rs"}
	if _, err := ShapeCrate(roots, false, false); err == nil {
		t.Error("workspace root must not get a source-only fallback name")
	}
}

func TestShapeInvalidNamesFail(t *testing.T) {
	if _, err := ShapeCrate(CrateRoots{Dir: "crates/---", LibRoot: "crates/---/src/lib.rs"}, false, false); err == nil {
		t.Error("invalid crate directory name accepted")
	}
	if _, err := ShapeCrate(CrateRoots{Dir: "crates/app", TestRoots: []string{"crates/app/tests/---.rs"}}, false, false); err == nil {
		t.Error("invalid integration test name accepted")
	}
}

func TestShapeBinUnitTest(t *testing.T) {
	shape, err := ShapeCrate(CrateRoots{Dir: "crates/app", BinRoot: "crates/app/src/main.rs"}, false, true)
	if err != nil || !shape.BinUnitTest {
		t.Errorf("bin unit-test shape = %+v, %v", shape, err)
	}
}

func TestShapeEmptyDir(t *testing.T) {
	shape, err := ShapeCrate(CrateRoots{Dir: "empty"}, false, false)
	if err != nil {
		t.Fatal(err)
	}
	if shape.LibTarget != "" || shape.BinTarget != "" || len(shape.Tests) != 0 {
		t.Errorf("empty dir must shape to nothing: %+v", shape)
	}
}

func memFS(files map[string]string) (func(string) ([]byte, error), func(string) bool) {
	read := func(p string) ([]byte, error) {
		c, ok := files[p]
		if !ok {
			return nil, fmt.Errorf("missing: " + p)
		}
		return []byte(c), nil
	}
	exists := func(p string) bool {
		_, ok := files[p]
		return ok
	}
	return read, exists
}

func TestLoadCrateTree(t *testing.T) {
	files := map[string]string{
		"a/src/lib.rs":          "mod parser;\n#[path = \"custom/x.rs\"] mod x;\nmod inline {\n use inner::Dep;\n}\n",
		"a/src/parser.rs":       "mod lexer;\n",
		"a/src/parser/lexer.rs": "use std::fmt::Debug;\n",
		"a/src/custom/x.rs":     "",
	}
	read, exists := memFS(files)
	owned, err := LoadCrate("a/src/lib.rs", read, exists)
	if err != nil {
		t.Fatal(err)
	}
	for _, want := range []string{"a/src/lib.rs", "a/src/parser.rs", "a/src/parser/lexer.rs", "a/src/custom/x.rs"} {
		if _, ok := owned[want]; !ok {
			t.Errorf("missing owned file %s (owned=%v)", want, keys(owned))
		}
	}
	if len(owned) != 4 {
		t.Errorf("owned = %v, want exactly 4 files", keys(owned))
	}
}

func TestLoadCrateInlineModuleChildren(t *testing.T) {
	files := map[string]string{
		"a/src/lib.rs":             "mod outer { mod child; }\nmod foo;\nmod n { #[path = \"p.rs\"] mod q; }\n",
		"a/src/foo.rs":             "mod outer { mod child; }\n#[path = \"d\"] mod x { mod y; }\n",
		"a/src/outer/child.rs":     "",
		"a/src/foo/outer/child.rs": "",
		"a/src/n/p.rs":             "",
		"a/src/d/y.rs":             "",
		"a/src/child.rs":           "",
	}
	read, exists := memFS(files)
	owned, err := LoadCrate("a/src/lib.rs", read, exists)
	if err != nil {
		t.Fatal(err)
	}
	want := []string{"a/src/d/y.rs", "a/src/foo.rs", "a/src/foo/outer/child.rs", "a/src/lib.rs", "a/src/n/p.rs", "a/src/outer/child.rs"}
	got := keys(owned)
	sort.Strings(got)
	if strings.Join(got, ",") != strings.Join(want, ",") {
		t.Errorf("owned = %v, want %v", got, want)
	}
	if _, ok := owned["a/src/child.rs"]; ok {
		t.Error("decoy a/src/child.rs must not be owned")
	}
}

func TestLoadCrateInlineModuleOrphanFails(t *testing.T) {
	files := map[string]string{
		"a/src/lib.rs":   "mod outer { mod child; }\n",
		"a/src/child.rs": "",
	}
	read, exists := memFS(files)
	_, err := LoadCrate("a/src/lib.rs", read, exists)
	if err == nil {
		t.Fatal("inline child without a/src/outer/child.rs must fail")
	}
	if !strings.Contains(err.Error(), "orphan module") || !strings.Contains(err.Error(), "a/src/outer/child.rs") {
		t.Errorf("wrong error: %v", err)
	}
}

func TestLoadCrateOrphanFails(t *testing.T) {
	files := map[string]string{"a/src/lib.rs": "mod missing;\n"}
	read, exists := memFS(files)
	if _, err := LoadCrate("a/src/lib.rs", read, exists); err == nil {
		t.Error("orphan module must fail")
	} else if !strings.Contains(err.Error(), "orphan module") {
		t.Errorf("wrong error: %v", err)
	}
}

func TestLoadCrateAmbiguousFails(t *testing.T) {
	files := map[string]string{
		"a/src/lib.rs":     "mod dup;\n",
		"a/src/dup.rs":     "",
		"a/src/dup/mod.rs": "",
	}
	read, exists := memFS(files)
	if _, err := LoadCrate("a/src/lib.rs", read, exists); err == nil {
		t.Error("foo.rs vs foo/mod.rs ambiguity must fail")
	} else if !strings.Contains(err.Error(), "ambiguous module") {
		t.Errorf("wrong error: %v", err)
	}
}

func TestLoadCrateAmbiguousCfgNamesOwnedFile(t *testing.T) {
	files := map[string]string{
		"a/src/lib.rs": "mod sub;\n",
		"a/src/sub.rs": "fn ok() {}\n\n#[cfg(any(unix, test))]\nfn bad() {}\n",
	}
	read, exists := memFS(files)
	_, err := LoadCrate("a/src/lib.rs", read, exists)
	if err == nil || !strings.Contains(err.Error(), "a/src/sub.rs:3:") || strings.Contains(err.Error(), "a/src/lib.rs") {
		t.Errorf("wrong error: %v", err)
	}
}

func TestLoadCrateCycleFails(t *testing.T) {
	files := map[string]string{
		"a/src/lib.rs": "mod a;\n",
		"a/src/a.rs":   "#[path = \"lib.rs\"] mod again;\n",
	}
	read, exists := memFS(files)
	if _, err := LoadCrate("a/src/lib.rs", read, exists); err == nil {
		t.Error("module cycle must fail")
	} else if !strings.Contains(err.Error(), "module cycle") {
		t.Errorf("wrong error: %v", err)
	}
}

func TestLoadCrateReadAndParseFailures(t *testing.T) {
	if _, err := LoadCrate("missing.rs", func(string) ([]byte, error) { return nil, fmt.Errorf("denied") }, func(string) bool { return true }); err == nil || !strings.Contains(err.Error(), "cannot read") {
		t.Errorf("read error = %v", err)
	}
	files := map[string]string{"a/src/lib.rs": "mod;"}
	read, exists := memFS(files)
	if _, err := LoadCrate("a/src/lib.rs", read, exists); err == nil || !strings.Contains(err.Error(), "expected module name") {
		t.Errorf("parse error = %v", err)
	}
}

func TestLoadCrateDuplicateInclusionFails(t *testing.T) {
	files := map[string]string{
		"a/src/lib.rs":    "#[path = \"shared.rs\"] mod one; #[path = \"shared.rs\"] mod two;",
		"a/src/shared.rs": "",
	}
	read, exists := memFS(files)
	if _, err := LoadCrate("a/src/lib.rs", read, exists); err == nil || !strings.Contains(err.Error(), "duplicate module inclusion") {
		t.Errorf("duplicate inclusion error = %v", err)
	}
}

func TestLoadCrateNestedParseFailure(t *testing.T) {
	files := map[string]string{
		"a/src/lib.rs":   "mod child;",
		"a/src/child.rs": "use missing",
	}
	read, exists := memFS(files)
	if _, err := LoadCrate("a/src/lib.rs", read, exists); err == nil || !strings.Contains(err.Error(), "unterminated") {
		t.Errorf("nested parse error = %v", err)
	}
}

func TestLoadCrateTestScopeAndRootPath(t *testing.T) {
	files := map[string]string{
		"a/src/lib.rs":          "#[cfg(test)] mod tests;\n#[path = \"/shared.rs\"] mod shared;\n",
		"a/src/tests.rs":        "mod nested; use dev_only::Thing; extern crate dev_extern;\n",
		"a/src/tests/nested.rs": "",
		"shared.rs":             "",
	}
	read, exists := memFS(files)
	owned, err := LoadCrate("a/src/lib.rs", read, exists)
	if err != nil {
		t.Fatal(err)
	}
	tests := owned["a/src/tests.rs"]
	if !tests.Modules[0].CfgTest || !tests.Uses[0].CfgTest || !tests.Externs[0].CfgTest || joinRel(".", "x.rs") != "x.rs" || joinRel("a", "/x.rs") != "x.rs" {
		t.Errorf("test propagation or root path failed: %+v", owned)
	}
}

func TestLoadCrateTestOnly(t *testing.T) {
	files := map[string]string{
		"a/src/lib.rs": "#![doc = \"x\"]\n#[cfg(test)] mod tests;\nmod prod;\n#[cfg(test)] mod t { mod inner; }\n" +
			"#[cfg(test)]\n#[path = \"fixtures/p.rs\"]\nmod pathed;\nmod inner_only;\n",
		"a/src/tests.rs":               "mod nested;\n",
		"a/src/tests/nested.rs":        "mod deeper;\n",
		"a/src/tests/nested/deeper.rs": "",
		"a/src/prod.rs":                "#[cfg(test)] mod prod_tests;\nmod helper;\n",
		"a/src/prod/prod_tests.rs":     "",
		"a/src/prod/helper.rs":         "",
		"a/src/t/inner.rs":             "",
		"a/src/fixtures/p.rs":          "",
		"a/src/inner_only.rs":          "#![cfg(test)]\nmod child;\n",
		"a/src/inner_only/child.rs":    "",
	}
	read, exists := memFS(files)
	owned, err := LoadCrate("a/src/lib.rs", read, exists)
	if err != nil {
		t.Fatal(err)
	}
	want := map[string]bool{
		"a/src/lib.rs":                 false,
		"a/src/tests.rs":               true,
		"a/src/tests/nested.rs":        true,
		"a/src/tests/nested/deeper.rs": true,
		"a/src/prod.rs":                false,
		"a/src/prod/prod_tests.rs":     true,
		"a/src/prod/helper.rs":         false,
		"a/src/t/inner.rs":             true,
		"a/src/fixtures/p.rs":          true,
		"a/src/inner_only.rs":          false,
		"a/src/inner_only/child.rs":    false,
	}
	if len(owned) != len(want) {
		t.Fatalf("owned = %v, want %d files", keys(owned), len(want))
	}
	for file, testOnly := range want {
		facts := owned[file]
		if facts == nil {
			t.Errorf("%s not owned", file)
			continue
		}
		if facts.TestOnly != testOnly {
			t.Errorf("%s TestOnly = %v, want %v", file, facts.TestOnly, testOnly)
		}
	}
	if !owned["a/src/inner_only/child.rs"].HasCfgTest && !owned["a/src/inner_only.rs"].Modules[0].CfgTest {
		t.Errorf("inner cfg(test) no longer marks its modules as test scope")
	}
}

func keys(m map[string]*FileFacts) []string {
	out := make([]string, 0, len(m))
	for k := range m {
		out = append(out, k)
	}
	return out
}
