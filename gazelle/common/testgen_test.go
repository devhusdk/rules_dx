package common

import (
	"errors"
	"fmt"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
)

type recordingRep struct{ errs []string }

func (r *recordingRep) Fail(_ PackageRef, format string, args ...interface{}) {
	r.errs = append(r.errs, fmt.Sprintf(format, args...))
}

func (r *recordingRep) Failed() bool { return len(r.errs) > 0 }

const (
	fakeLib  = "fake_library"
	fakeTest = "fake_test"
)

func fakeTestSpec() *TestSpec {
	return &TestSpec{
		Kind:     fakeTest,
		IsSource: func(name string) bool { return strings.HasSuffix(name, "Test.fk") },
		Name:     func(src string) (string, error) { return SnakeName(src, []string{".fk"}) },
		IsImplicit: func(imp string) bool {
			return strings.HasPrefix(imp, "runner.")
		},
	}
}

func fakeDirSpec(tests *TestSpec) SingleDirSpec {
	return SingleDirSpec{
		LanguageName: "fake",
		LibraryKind:  fakeLib,
		IsSource:     func(name string) bool { return strings.HasSuffix(name, ".fk") && !strings.HasSuffix(name, "Test.fk") },
		DirName:      DirTargetName,
		DefinesMain:  func([]byte) bool { return false },
		ParseImports: func(content []byte) []string {
			var out []string
			for _, line := range strings.Split(string(content), "\n") {
				if imp, ok := strings.CutPrefix(line, "import "); ok {
					out = append(out, imp)
				}
			}
			return out
		},
		Wrap:  WrapImportSet,
		Tests: tests,
	}
}

func fakeKinds() map[string]rule.KindInfo {
	return map[string]rule.KindInfo{fakeLib: LibraryKindInfo(), fakeTest: TestKindInfo()}
}

func runFakeDir(t *testing.T, tests *TestSpec, files map[string]string, existing *rule.File) (language.GenerateResult, *recordingRep) {
	t.Helper()
	root := t.TempDir()
	dir := filepath.Join(root, "pkg", "demo")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	var regular []string
	for name, content := range files {
		regular = append(regular, name)
		if err := os.WriteFile(filepath.Join(dir, name), []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	rep := &recordingRep{}
	result := GenerateSingleDir(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          dir,
		Rel:          "pkg/demo",
		RegularFiles: regular,
		File:         existing,
	}, fakeKinds(), fakeDirSpec(tests), rep)
	return result, rep
}

func ruleSummary(rules []*rule.Rule) string {
	var parts []string
	for _, r := range rules {
		parts = append(parts, r.Kind()+"("+r.Name()+")")
	}
	return strings.Join(parts, ",")
}

func existingFile(rules ...*rule.Rule) *rule.File {
	f := &rule.File{}
	f.Rules = append(f.Rules, rules...)
	return f
}

func newRule(kind, name string, srcs ...string) *rule.Rule {
	r := rule.NewRule(kind, name)
	if srcs != nil {
		r.SetAttr("srcs", srcs)
	}
	return r
}

func TestGenerateSingleDirTestsNilUnchanged(t *testing.T) {
	result, rep := runFakeDir(t, nil, map[string]string{
		"A.fk":     "import x\n",
		"ATest.fk": "import y\n",
	}, existingFile(newRule(fakeTest, "a_test", "ATest.fk")))
	if rep.Failed() {
		t.Fatal(rep.errs)
	}
	if got := ruleSummary(result.Gen); got != "fake_library(demo)" {
		t.Errorf("Gen = %s, want only the library", got)
	}
	if len(result.Imports) != 1 || strings.Join(result.Imports[0].(ImportSet).Names, ",") != "x" {
		t.Errorf("Imports = %+v", result.Imports)
	}
}

func TestGenerateSingleDirTests(t *testing.T) {
	result, rep := runFakeDir(t, fakeTestSpec(), map[string]string{
		"A.fk":         "import x\n",
		"HTTPTest.fk":  "import runner.Assert\nimport z\nimport z\n",
		"AlphaTest.fk": "import y\n",
	}, nil)
	if rep.Failed() {
		t.Fatal(rep.errs)
	}
	if got := ruleSummary(result.Gen); got != "fake_library(demo),fake_test(alpha_test),fake_test(http_test)" {
		t.Fatalf("Gen = %s", got)
	}
	want := []string{"x", "y", "z"}
	for i, imp := range result.Imports {
		if got := strings.Join(imp.(ImportSet).Names, ","); got != want[i] {
			t.Errorf("Imports[%d] = %q, want %q", i, got, want[i])
		}
	}
	for _, r := range result.Gen[1:] {
		if got := strings.Join(r.AttrStrings("deps"), ","); got != ":demo" {
			t.Errorf("%s deps = %q, want :demo", r.Name(), got)
		}
	}
}

func TestGenerateSingleDirTestsOnly(t *testing.T) {
	result, rep := runFakeDir(t, fakeTestSpec(), map[string]string{"ATest.fk": ""}, nil)
	if rep.Failed() {
		t.Fatal(rep.errs)
	}
	if got := ruleSummary(result.Gen); got != "fake_test(a_test)" {
		t.Fatalf("Gen = %s", got)
	}
	if result.Gen[0].Attr("deps") != nil {
		t.Errorf("deps = %v, want none without a library", result.Gen[0].AttrStrings("deps"))
	}
}

func TestGenerateSingleDirTestsGlobOwner(t *testing.T) {
	suite := rule.NewRule(fakeTest, "suite")
	suite.SetAttr("srcs", rule.GlobValue{Patterns: []string{"*Test.fk"}})
	result, rep := runFakeDir(t, fakeTestSpec(), map[string]string{"ATest.fk": "", "BTest.fk": ""}, existingFile(suite))
	if rep.Failed() {
		t.Fatal(rep.errs)
	}
	if len(result.Gen) != 0 || len(result.Empty) != 0 {
		t.Errorf("Gen = %s, Empty = %s; want nothing generated or swept", ruleSummary(result.Gen), ruleSummary(result.Empty))
	}
}

func TestGenerateSingleDirTestsOtherKindGlobNotOwner(t *testing.T) {
	sibling := rule.NewRule("sh_test", "scripts")
	sibling.SetAttr("srcs", rule.GlobValue{Patterns: []string{"*.sh"}})
	result, rep := runFakeDir(t, fakeTestSpec(), map[string]string{"ATest.fk": ""}, existingFile(sibling))
	if rep.Failed() {
		t.Fatal(rep.errs)
	}
	if len(result.Gen) != 1 || result.Gen[0].Kind() != fakeTest || result.Gen[0].Name() != "a_test" {
		t.Errorf("Gen = %s, want fake_test(a_test)", ruleSummary(result.Gen))
	}
}

func TestGenerateSingleDirTestsClaimError(t *testing.T) {
	_, rep := runFakeDir(t, fakeTestSpec(), map[string]string{"ATest.fk": ""}, existingFile(newRule("filegroup", "a_test", "other.txt")))
	if len(rep.errs) != 1 || !strings.Contains(rep.errs[0], `target name "a_test" is claimed by generated fake_test(pkg/demo/ATest.fk) and existing filegroup`) {
		t.Errorf("errors = %v", rep.errs)
	}
}

func TestGenerateSingleDirTestsCollision(t *testing.T) {
	_, rep := runFakeDir(t, fakeTestSpec(), map[string]string{"FooTest.fk": "", "foo_Test.fk": ""}, nil)
	if len(rep.errs) != 1 || !strings.Contains(rep.errs[0], `normalized name "foo_test"`) {
		t.Errorf("errors = %v, want a collision", rep.errs)
	}
}

func TestGenerateSingleDirTestsSweep(t *testing.T) {
	result, rep := runFakeDir(t, fakeTestSpec(), map[string]string{"A.fk": ""}, existingFile(
		newRule(fakeTest, "gone_test", "GoneTest.fk"),
		newRule(fakeTest, "custom", "CustomTest.fk"),
		newRule(fakeTest, "multi", "XTest.fk", "helper.fk"),
	))
	if rep.Failed() {
		t.Fatal(rep.errs)
	}
	if got := ruleSummary(result.Empty); got != "fake_test(gone_test)" {
		t.Errorf("Empty = %s, want only the generated-shaped gone_test", got)
	}
}

func TestGenerateSingleDirTestsHandwrittenOwner(t *testing.T) {
	result, rep := runFakeDir(t, fakeTestSpec(), map[string]string{"A.fk": "", "ATest.fk": "", "BTest.fk": ""}, existingFile(
		newRule(fakeTest, "custom", ":ATest.fk"),
		newRule("filegroup", "files", "BTest.fk"),
	))
	if rep.Failed() {
		t.Fatal(rep.errs)
	}
	if got := ruleSummary(result.Gen); got != "fake_library(demo),fake_test(b_test)" {
		t.Errorf("Gen = %s; want ATest.fk left to its owner and BTest.fk generated", got)
	}
	if len(result.Empty) != 0 {
		t.Errorf("Empty = %s, want the handwritten rule kept", ruleSummary(result.Empty))
	}
}

func TestGenerateSingleDirTestsMakeError(t *testing.T) {
	spec := fakeTestSpec()
	spec.Make = func(string, string, []byte) (*rule.Rule, error) { return nil, errors.New("bad test") }
	result, rep := runFakeDir(t, spec, map[string]string{"ATest.fk": ""}, nil)
	if len(result.Gen) != 0 || len(rep.errs) != 1 || !strings.Contains(rep.errs[0], "ATest.fk: bad test") {
		t.Errorf("Gen = %s, errors = %v", ruleSummary(result.Gen), rep.errs)
	}
}

func TestTestKindInfo(t *testing.T) {
	info := TestKindInfo("test_class")
	if !info.MergeableAttrs["test_class"] || !info.MergeableAttrs["deps"] || !info.ResolveAttrs["deps"] {
		t.Errorf("TestKindInfo = %+v", info)
	}
	if LibraryKindInfo().MergeableAttrs["test_class"] {
		t.Error("TestKindInfo changed LibraryKindInfo")
	}
}
