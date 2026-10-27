package golang

import (
	"context"
	"flag"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
	"github.com/devhusdk/rules_dx/gazelle/common"
)

func importNames(t *testing.T, raw any) []string {
	t.Helper()
	names, ok := common.UnwrapImportSet(raw)
	if !ok {
		t.Fatalf("import set = %#v, want common.ImportSet", raw)
	}
	return names
}

func writeFixture(t *testing.T, root, name, content string) {
	t.Helper()
	file := filepath.Join(root, filepath.FromSlash(name))
	if err := os.MkdirAll(filepath.Dir(file), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(file, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

func TestConfigureRoutesIgnoreDirectivesToTheSharedConfig(t *testing.T) {
	cfg := config.New()
	lang := newLang()
	lang.Configure(cfg, "parent", &rule.File{Directives: []rule.Directive{
		{Key: "other"},
		{Key: "dx_ignore_import", Value: "go go example.com/widget"},
		{Key: "dx_ignore_import", Value: "go"},
	}})
	if _, ok := cfg.Exts[languageName].(*common.IgnoreConfig); !ok {
		t.Fatalf("ext = %T, want *common.IgnoreConfig", cfg.Exts[languageName])
	}
	entry := common.MatchingIgnore(cfg, languageName, "example.com/widget")
	if entry == nil || entry.Path != "parent" {
		t.Fatalf("declared ignore = %+v", entry)
	}
	if len(lang.Errors()) != 1 {
		t.Fatalf("malformed directive accepted: %v", lang.Errors())
	}
}

func generateFixture(t *testing.T, files map[string]string, regular []string) language.GenerateResult {
	t.Helper()
	root := t.TempDir()
	for name, content := range files {
		writeFixture(t, root, name, content)
	}
	dir := filepath.Join(root, "pkg", "demo")
	return NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          dir,
		Rel:          "pkg/demo",
		RegularFiles: regular,
	})
}

func refusal(t *testing.T, lang language.Language) string {
	t.Helper()
	lm, ok := lang.(language.LifecycleManager)
	if !ok {
		t.Fatalf("%T does not implement language.LifecycleManager", lang)
	}
	lm.AfterResolvingDeps(context.Background())
	got := common.Errors.Take()
	if len(got) != 1 {
		t.Fatalf("refusals = %#v, want exactly one", got)
	}
	return got[0].Message
}

func refuseFixture(t *testing.T, files map[string]string, regular []string) string {
	t.Helper()
	root := t.TempDir()
	for name, content := range files {
		writeFixture(t, root, name, content)
	}
	lang := NewLanguage()
	result := lang.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: regular,
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules, want 0 with a recorded failure", len(result.Gen))
	}
	return refusal(t, lang)
}

func wantRefusal(t *testing.T, got, want string) {
	t.Helper()
	if !strings.Contains(got, want) {
		t.Fatalf("refusal = %q, want it to contain %q", got, want)
	}
}

func TestGeneratePackageLevelLibrary(t *testing.T) {
	regular := []string{"demo.go", "helper.go", "notes.txt", "demo_test.go"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.go":      "package demo\n\nimport \"rules_dx/stuff/thing\"\n",
		"pkg/demo/helper.go":    "package demo\n\nimport \"fmt\"\n",
		"pkg/demo/notes.txt":    "not a source\n",
		"pkg/demo/demo_test.go": "package demo_test\n\nimport \"testing\"\n",
	}, regular)
	if len(result.Gen) != 2 || len(result.Imports) != 2 {
		t.Fatalf("generated %d rules and %d import sets, want 2 each (library + package-level test)", len(result.Gen), len(result.Imports))
	}
	lib := result.Gen[0]
	if lib.Kind() != LibraryKind || lib.Name() != "demo" {
		t.Fatalf("library = %s(%s), want go_library(demo)", lib.Kind(), lib.Name())
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "demo.go,helper.go" {
		t.Errorf("library srcs = %q, want demo.go,helper.go", got)
	}
	got := importNames(t, result.Imports[0])
	if strings.Join(got, ",") != "rules_dx/stuff/thing" {
		t.Errorf("library imports = %+v, want [rules_dx/stuff/thing]", got)
	}
	testRule := result.Gen[1]
	if testRule.Kind() != TestKind || testRule.Name() != "demo_test" {
		t.Fatalf("test = %s(%s), want go_test(demo_test)", testRule.Kind(), testRule.Name())
	}
	if got := strings.Join(testRule.AttrStrings("srcs"), ","); got != "demo_test.go" {
		t.Errorf("test srcs = %q, want demo_test.go", got)
	}
	if got := strings.Join(testRule.AttrStrings("embed"), ","); got != ":demo" {
		t.Errorf("test embed = %q, want :demo", got)
	}
	if got := importNames(t, result.Imports[1]); len(got) != 0 {
		t.Errorf("test imports = %+v, want empty (stdlib-only)", got)
	}
}

func TestGenerateTestSourcesExcluded(t *testing.T) {
	regular := []string{"demo.go", "demo_test.go"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.go":      "package demo\n",
		"pkg/demo/demo_test.go": "package demo_test\n\nimport \"rules_dx/stuff/only_by_test\"\n",
	}, regular)
	if len(result.Gen) != 2 || len(result.Imports) != 2 {
		t.Fatalf("generated %d rules and %d import sets, want 2 each", len(result.Gen), len(result.Imports))
	}
	if got := strings.Join(result.Gen[0].AttrStrings("srcs"), ","); got != "demo.go" {
		t.Errorf("library srcs = %q, want demo.go", got)
	}
	if got := importNames(t, result.Imports[0]); len(got) != 0 {
		t.Errorf("library imports = %+v, want empty (test-only import stays off the library)", got)
	}
	if got := strings.Join(result.Gen[1].AttrStrings("srcs"), ","); got != "demo_test.go" {
		t.Errorf("test srcs = %q, want demo_test.go", got)
	}
	if got := importNames(t, result.Imports[1]); strings.Join(got, ",") != "rules_dx/stuff/only_by_test" {
		t.Errorf("test imports = %+v, want [rules_dx/stuff/only_by_test]", got)
	}
}

func TestGenerateNoSourcesStaleSweep(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, "pkg", "demo")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	result := NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          dir,
		Rel:          "pkg/demo",
		RegularFiles: []string{"notes.txt"},
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules, want 0", len(result.Gen))
	}
}

func TestGeneratePackageMainFails(t *testing.T) {
	regular := []string{"demo.go", "main.go"}
	got := refuseFixture(t, map[string]string{
		"pkg/demo/demo.go": "package demo\n",
		"pkg/demo/main.go": "package main\n\nfunc main() {}\n",
	}, regular)
	wantRefusal(t, got, "go: pkg/demo: main.go declares package main;")
}

func TestGenerateMixedPackagesFail(t *testing.T) {
	regular := []string{"a.go", "b.go"}
	got := refuseFixture(t, map[string]string{
		"pkg/demo/a.go": "package alpha\n",
		"pkg/demo/b.go": "package beta\n",
	}, regular)
	wantRefusal(t, got, "go: pkg/demo: mixed packages alpha, beta in one directory;")
}

func TestGenerateBuildIgnoredMainSkipped(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/lib.go", "package demo\n")
	writeFixture(t, root, "pkg/demo/gen.go", "//go:build ignore\n\npackage main\n\nfunc main() {}\n")
	result := generateAt(t, root, "pkg/demo", []string{"gen.go", "lib.go"})
	if len(result.Gen) != 1 {
		t.Fatalf("generated %d rules, want 1", len(result.Gen))
	}
	if got := result.Gen[0].AttrStrings("srcs"); strings.Join(got, ",") != "lib.go" {
		t.Errorf("srcs = %v, want [lib.go]", got)
	}
}

func TestGenerateToolsTaggedPackageSkipped(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/lib.go", "package demo\n")
	writeFixture(t, root, "pkg/demo/lib_linux.go", "//go:build linux\n\npackage demo\n")
	writeFixture(t, root, "pkg/demo/tools.go", "//go:build tools\n\npackage tools\n\nimport _ \"example.test/tool\"\n")
	result := generateAt(t, root, "pkg/demo", []string{"lib.go", "lib_linux.go", "tools.go"})
	if len(result.Gen) != 1 {
		t.Fatalf("generated %d rules, want 1", len(result.Gen))
	}
	if got := result.Gen[0].AttrStrings("srcs"); strings.Join(got, ",") != "lib.go,lib_linux.go" {
		t.Errorf("srcs = %v, want [lib.go lib_linux.go]", got)
	}
	if imports := importNames(t, result.Imports[0]); len(imports) != 0 {
		t.Errorf("imports = %v, want none", imports)
	}
}

func TestGenerateBuildIgnoredTestSkipped(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/lib.go", "package demo\n")
	writeFixture(t, root, "pkg/demo/old_test.go", "//go:build ignore\n\npackage other\n")
	result := generateAt(t, root, "pkg/demo", []string{"lib.go", "old_test.go"})
	if len(result.Gen) != 1 || result.Gen[0].Kind() != LibraryKind {
		t.Fatalf("generated %d rules, want one %s", len(result.Gen), LibraryKind)
	}
}

func TestGenerateCustomTaggedTestKept(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/lib.go", "package demo\n")
	writeFixture(t, root, "pkg/demo/integration_test.go", "//go:build integration\n\npackage demo\n\nimport \"testing\"\n\nfunc TestIntegration(t *testing.T) {}\n")
	result := generateAt(t, root, "pkg/demo", []string{"lib.go", "integration_test.go"})
	if len(result.Gen) != 2 {
		t.Fatalf("generated %d rules, want 2", len(result.Gen))
	}
	for _, r := range result.Gen {
		want := "lib.go"
		if r.Kind() == TestKind {
			want = "integration_test.go"
		}
		if got := strings.Join(r.AttrStrings("srcs"), ","); got != want {
			t.Errorf("%s srcs = %s, want %s", r.Kind(), got, want)
		}
	}
}

func TestGenerateOnlyExcludedSourcesSweep(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/gen.go", "//go:build ignore\n\npackage main\n")
	existing, err := rule.LoadData("BUILD.bazel", "pkg/demo", []byte("load(\"@rules_dx//go/rules:defs.bzl\", \"go_library\", \"go_test\")\n\ngo_library(name = \"demo\", srcs = [\"gen.go\"])\n\ngo_test(name = \"demo_test\", srcs = [\"old_test.go\"], embed = [\":demo\"])\n"))
	if err != nil {
		t.Fatal(err)
	}
	l := newLang()
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"gen.go"},
		File:         existing,
	})
	if len(l.Errors()) != 0 {
		t.Fatalf("errors = %v, want none", l.Errors())
	}
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules, want 0", len(result.Gen))
	}
	names := map[string]bool{}
	for _, r := range result.Empty {
		names[r.Kind()+":"+r.Name()] = true
	}
	if len(result.Empty) != 2 || !names[LibraryKind+":demo"] || !names[TestKind+":demo_test"] {
		t.Fatalf("empty = %+v, want go_library demo and go_test demo_test swept", result.Empty)
	}
}

func TestGenerateBadBuildConstraintFails(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/lib.go", "package demo\n")
	writeFixture(t, root, "pkg/demo/bad.go", "//go:build &&\n\npackage demo\n")
	l := newLang()
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"bad.go", "lib.go"},
	})
	if len(l.Errors()) != 1 {
		t.Fatalf("errors = %v, want one build constraint failure", l.Errors())
	}
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules, want 0", len(result.Gen))
	}
}

func TestGenerateKindMismatchFails(t *testing.T) {
	root := t.TempDir()
	for name, content := range map[string]string{
		"pkg/demo/demo.go": "package demo\n",
	} {
		writeFixture(t, root, name, content)
	}
	existing := &rule.File{}
	fg := rule.NewRule("filegroup", "demo")
	existing.Rules = append(existing.Rules, fg)
	lang := NewLanguage()
	result := lang.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"demo.go"},
		File:         existing,
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules over a kind mismatch, want 0 with a recorded failure", len(result.Gen))
	}
	wantRefusal(t, refusal(t, lang), "go: pkg/demo: target name \"demo\" is claimed by generated go_library(pkg/demo) and existing filegroup")
}

func TestGenerateInternalAndExternalTestsCoexist(t *testing.T) {
	regular := []string{"demo.go", "internal_test.go", "external_test.go", "helper_test.go"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.go":          "package demo\n",
		"pkg/demo/internal_test.go": "package demo\n\nimport \"testing\"\n",
		"pkg/demo/external_test.go": "package demo_test\n\nimport \"testing\"\n",
		"pkg/demo/helper_test.go":   "package demo\n\nimport \"testing\"\n",
	}, regular)
	if len(result.Gen) != 2 {
		t.Fatalf("generated %d rules, want 2 (library + one package-level test)", len(result.Gen))
	}
	testRule := result.Gen[1]
	if testRule.Kind() != TestKind || testRule.Name() != "demo_test" {
		t.Fatalf("test = %s(%s), want go_test(demo_test)", testRule.Kind(), testRule.Name())
	}
	if got := strings.Join(testRule.AttrStrings("srcs"), ","); got != "external_test.go,helper_test.go,internal_test.go" {
		t.Errorf("test srcs = %q, want sorted package-level set", got)
	}
	if got := strings.Join(testRule.AttrStrings("embed"), ","); got != ":demo" {
		t.Errorf("test embed = %q, want :demo", got)
	}
}

func TestGenerateTestMainAndSharedImport(t *testing.T) {
	regular := []string{"demo.go", "main_test.go", "extra_test.go"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.go":       "package demo\n\nimport \"rules_dx/stuff/shared\"\n",
		"pkg/demo/main_test.go":  "package demo\n\nimport (\n\"testing\"\n\"rules_dx/stuff/shared\"\n\"rules_dx/stuff/only_by_test\"\n)\n\nfunc TestMain(m *testing.M) {}\n",
		"pkg/demo/extra_test.go": "package demo_test\n\nimport \"testing\"\n",
	}, regular)
	if len(result.Gen) != 2 {
		t.Fatalf("generated %d rules, want 2", len(result.Gen))
	}
	if got := importNames(t, result.Imports[0]); strings.Join(got, ",") != "rules_dx/stuff/shared" {
		t.Errorf("library imports = %+v, want [rules_dx/stuff/shared]", got)
	}
	if got := importNames(t, result.Imports[1]); strings.Join(got, ",") != "rules_dx/stuff/only_by_test" {
		t.Errorf("test imports = %+v, want [rules_dx/stuff/only_by_test]", got)
	}
}

func TestGenerateLibraryOnlyWithoutTestSources(t *testing.T) {
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.go": "package demo\n",
	}, []string{"demo.go"})
	if len(result.Gen) != 1 || len(result.Imports) != 1 {
		t.Fatalf("generated %d rules, want 1 (no test without *_test.go)", len(result.Gen))
	}
	if result.Gen[0].Kind() != LibraryKind {
		t.Errorf("rule = %s, want go_library", result.Gen[0].Kind())
	}
}

func TestGenerateTestOnlyDirFails(t *testing.T) {
	got := refuseFixture(t, map[string]string{
		"pkg/demo/demo_test.go": "package demo_test\n\nimport \"testing\"\n",
	}, []string{"demo_test.go"})
	wantRefusal(t, got, "go: pkg/demo: test sources demo_test.go without non-test sources;")
}

func TestGenerateMismatchedTestPackageFails(t *testing.T) {
	got := refuseFixture(t, map[string]string{
		"pkg/demo/demo.go":      "package demo\n",
		"pkg/demo/demo_test.go": "package other\n",
	}, []string{"demo.go", "demo_test.go"})
	wantRefusal(t, got, "go: pkg/demo: demo_test.go declares package other, want demo or demo_test;")
}

func TestGenerateTestKindMismatchFails(t *testing.T) {
	root := t.TempDir()
	for name, content := range map[string]string{
		"pkg/demo/demo.go":      "package demo\n",
		"pkg/demo/demo_test.go": "package demo_test\n",
	} {
		writeFixture(t, root, name, content)
	}
	existing := &rule.File{}
	bin := rule.NewRule("go_binary", "demo_test")
	existing.Rules = append(existing.Rules, bin)
	lang := NewLanguage()
	result := lang.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"demo.go", "demo_test.go"},
		File:         existing,
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules over a test kind mismatch, want 0 with a recorded failure", len(result.Gen))
	}
	wantRefusal(t, refusal(t, lang), "go: pkg/demo: target name \"demo_test\" is claimed by generated go_test(pkg/demo) and existing go_binary")
}

func TestGenerateStaleTestSwept(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/demo.go", "package demo\n")
	existing, err := rule.LoadData("BUILD.bazel", "pkg/demo", []byte("load(\"@rules_dx//go/rules:defs.bzl\", \"go_library\", \"go_test\")\n\ngo_library(name = \"demo\", srcs = [\"demo.go\"])\n\ngo_test(name = \"demo_test\", srcs = [\"old_test.go\"], embed = [\":demo\"])\n"))
	if err != nil {
		t.Fatal(err)
	}
	result := NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"demo.go"},
		File:         existing,
	})
	if len(result.Gen) != 1 {
		t.Fatalf("generated %d rules, want 1 library", len(result.Gen))
	}
	if len(result.Empty) != 1 || result.Empty[0].Kind() != TestKind || result.Empty[0].Name() != "demo_test" {
		t.Fatalf("empty = %+v, want stale go_test(demo_test) swept", result.Empty)
	}
}

func TestGoImportPathUsesModfile(t *testing.T) {
	for _, tc := range []struct {
		gomod string
		want  string
		ok    bool
	}{
		{"module example.com/adopt-go\n\ngo 1.26\n", "example.com/adopt-go/pkg/demo", true},
		{"// leading comment\nmodule example.com/x // trailing\n", "example.com/x/pkg/demo", true},
		{"module \"example.com/quoted\"\n", "example.com/quoted/pkg/demo", true},
		{"go 1.26\n", "", false},
		{"", "", false},
	} {
		root := t.TempDir()
		writeFixture(t, root, "go.mod", tc.gomod)
		got, ok := goImportPath(root, filepath.Join(root, "pkg", "demo"))
		if got != tc.want || ok != tc.ok {
			t.Errorf("goImportPath(%q) = %q,%v, want %q,%v", tc.gomod, got, ok, tc.want, tc.ok)
		}
	}
}

func TestGenerateImportPathFromGoMod(t *testing.T) {
	root := t.TempDir()
	for name, content := range map[string]string{
		"go.mod":           "module example.com/adopt-go\n\ngo 1.26\n",
		"pkg/demo/demo.go": "package demo\n",
	} {
		writeFixture(t, root, name, content)
	}
	result := NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"demo.go"},
	})
	if len(result.Gen) != 1 {
		t.Fatalf("generated %d rules, want 1", len(result.Gen))
	}
	if got := result.Gen[0].AttrString("importpath"); got != "example.com/adopt-go/pkg/demo" {
		t.Errorf("importpath = %q, want example.com/adopt-go/pkg/demo", got)
	}
}

func TestGenerateImportPathModuleRoot(t *testing.T) {
	root := t.TempDir()
	for name, content := range map[string]string{
		"pkg/demo/go.mod":  "module example.com/demo\n",
		"pkg/demo/demo.go": "package demo\n",
	} {
		writeFixture(t, root, name, content)
	}
	result := NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"demo.go"},
	})
	if len(result.Gen) != 1 {
		t.Fatalf("generated %d rules, want 1", len(result.Gen))
	}
	if got := result.Gen[0].AttrString("importpath"); got != "example.com/demo" {
		t.Errorf("importpath = %q, want example.com/demo", got)
	}
}

func TestGenerateImportPathOmittedWithoutGoMod(t *testing.T) {
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.go": "package demo\n",
	}, []string{"demo.go"})
	if len(result.Gen) != 1 {
		t.Fatalf("generated %d rules, want 1", len(result.Gen))
	}
	if attr := result.Gen[0].Attr("importpath"); attr != nil {
		t.Errorf("importpath = %v, want nil without go.mod", result.Gen[0].AttrString("importpath"))
	}
}

func TestImportsExportImportpath(t *testing.T) {
	lang := NewLanguage()
	lib := rule.NewRule(LibraryKind, "lib")
	lib.SetAttr("srcs", []string{"impl.go", "more.go", "impl_test.go"})
	lib.SetAttr("importpath", "example.test/demo/lib")
	imports := lang.Imports(&config.Config{}, lib, nil)
	if len(imports) != 1 || imports[0].Lang != languageName || imports[0].Imp != "example.test/demo/lib" {
		t.Errorf("imports = %+v, want the library importpath only", imports)
	}
	if got := lang.Imports(&config.Config{}, rule.NewRule("filegroup", "demo"), nil); got != nil {
		t.Errorf("other-kind imports = %+v, want nil", got)
	}
	bare := rule.NewRule(LibraryKind, "bare")
	bare.SetAttr("srcs", []string{"bare.go"})
	if got := lang.Imports(&config.Config{}, bare, nil); got != nil {
		t.Errorf("importpath-less imports = %+v, want nil", got)
	}
	testRule := rule.NewRule(TestKind, "lib_test")
	testRule.SetAttr("importpath", "example.test/demo/lib")
	if got := lang.Imports(&config.Config{}, testRule, nil); got != nil {
		t.Errorf("test imports = %+v, want nil (tests are never dependencies)", got)
	}
}

func TestAfterResolvingDepsReportsFailures(t *testing.T) {
	l := newLang()
	l.Before(context.Background())
	l.Fail("go: boom")
	l.AfterResolvingDeps(context.Background())
	got := common.Errors.Take()
	if len(got) != 1 || got[0].Language != languageName || got[0].Message != "go: boom" {
		t.Fatalf("sink = %#v, want one go failure", got)
	}
}

func TestAfterResolvingDepsClean(t *testing.T) {
	l := newLang()
	l.Before(context.Background())
	l.AfterResolvingDeps(context.Background())
	if len(l.Errors()) != 0 {
		t.Errorf("errors = %v", l.Errors())
	}
}

func TestLanguageMetadata(t *testing.T) {
	l := newLang()
	if l.Name() != "go" || len(l.Kinds()) != 2 || l.CheckFlags(flag.NewFlagSet("test", flag.ContinueOnError), config.New()) != nil {
		t.Fatal("invalid language metadata")
	}
	if _, ok := l.Kinds()[LibraryKind]; !ok {
		t.Fatalf("kinds = %v, want go_library", l.Kinds())
	}
	if _, ok := l.Kinds()[TestKind]; !ok {
		t.Fatalf("kinds = %v, want go_test", l.Kinds())
	}
	l.RegisterFlags(flag.NewFlagSet("test", flag.ContinueOnError), "update", config.New())
	l.Configure(config.New(), "pkg", nil)
	l.DoneGeneratingRules()
	if got := l.KnownDirectives(); len(got) != 1 || got[0] != "dx_ignore_import" {
		t.Fatalf("directives = %v, want [dx_ignore_import]", got)
	}
	if l.Embeds(nil, label.NoLabel) != nil {
		t.Fatal("invalid directives or embeds")
	}
	loads := l.ApparentLoads(func(name string) string {
		if name == "rules_dx" {
			return "renamed_dx"
		}
		return ""
	})
	if len(loads) != 1 || loads[0].Name != "@renamed_dx//go/rules:defs.bzl" || strings.Join(loads[0].Symbols, ",") != "go_library,go_test" {
		t.Errorf("apparent loads = %+v", loads)
	}
	if defaults := l.Loads(); len(defaults) != 1 || defaults[0].Name != "@rules_dx//go/rules:defs.bzl" {
		t.Errorf("default loads = %+v", defaults)
	}
	defaultApparent := l.ApparentLoads(func(string) string { return "" })
	if defaultApparent[0].Name != "@rules_dx//go/rules:defs.bzl" {
		t.Errorf("default apparent loads = %+v", defaultApparent)
	}
}

func resolverConfig(t *testing.T, directives []rule.Directive) *config.Config {
	t.Helper()
	cfg := config.New()
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	resolver.Configure(cfg, "app", &rule.File{Directives: directives})
	return cfg
}

type indexEntry struct {
	pkg        string
	name       string
	importpath string
}

func resolverIndex(lang *goLang, entries ...indexEntry) *resolve.RuleIndex {
	index := resolve.NewRuleIndex(func(r *rule.Rule, _ string) resolve.Resolver {
		if _, ok := goKinds[r.Kind()]; ok {
			return lang
		}
		return nil
	})
	for _, entry := range entries {
		r := rule.NewRule(LibraryKind, entry.name)
		r.SetAttr("srcs", []string{"impl.go"})
		r.SetAttr("importpath", entry.importpath)
		index.AddRule(config.New(), r, rule.EmptyFile(filepath.Join(entry.pkg, "BUILD.bazel"), entry.pkg))
	}
	index.Finish()
	return index
}

func TestResolveBranches(t *testing.T) {
	l := newLang()
	index := resolverIndex(l,
		indexEntry{"lib/b", "b", "example.test/lib/b"},
		indexEntry{"lib/a", "a", "example.test/lib/a"},
	)
	cfg := resolverConfig(t, nil)
	r := rule.NewRule(LibraryKind, "app")
	l.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"example.test/lib/b", "example.test/lib/a"}), label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//lib/a,//lib/b" {
		t.Errorf("resolved deps = %q", got)
	}

	self := rule.NewRule(LibraryKind, "a")
	l.Resolve(cfg, index, nil, self, common.WrapImportSet([]string{"example.test/lib/a"}), label.New("", "lib/a", "a"))
	if self.Attr("deps") != nil {
		t.Errorf("self dependency was emitted: %v", self.AttrStrings("deps"))
	}

	otherKind := rule.NewRule("filegroup", "other")
	l.Resolve(cfg, index, nil, otherKind, common.WrapImportSet([]string{"example.test/lib/a"}), label.New("", "app", "other"))
	if otherKind.Attr("deps") != nil {
		t.Errorf("non-library resolution emitted deps = %v", otherKind.AttrStrings("deps"))
	}

	std := rule.NewRule(LibraryKind, "uses_std")
	l.Resolve(cfg, index, nil, std, common.WrapImportSet([]string{"fmt"}), label.New("", "app", "uses_std"))
	if std.Attr("deps") != nil || len(l.Errors()) != 0 {
		t.Errorf("stdlib resolution = %v, errors = %v", std.AttrStrings("deps"), l.Errors())
	}

	hand := rule.NewRule(LibraryKind, "hand")
	hand.SetAttr("deps", []string{":kept"})
	l.Resolve(cfg, index, nil, hand, common.WrapImportSet([]string{"example.test/lib/a"}), label.New("", "app", "hand"))
	if got := strings.Join(hand.AttrStrings("deps"), ","); got != "//lib/a,:kept" {
		t.Errorf("merged deps = %q", got)
	}

	l.Resolve(cfg, index, nil, rule.NewRule(LibraryKind, "unknown"), common.WrapImportSet([]string{"example.test/unknown"}), label.New("", "app", "unknown"))
	if len(l.Errors()) != 1 || !strings.Contains(l.Errors()[0], "unresolved import") || !strings.Contains(l.Errors()[0], `or add "# gazelle:dx_ignore_import go example.test/unknown" to app/BUILD.bazel`) {
		t.Errorf("unresolved errors = %v", l.Errors())
	}
	l.Resolve(cfg, index, nil, r, nil, label.New("", "app", "app"))
}

func TestResolveOverride(t *testing.T) {
	l := newLang()
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "go go github.com/google/go-cmp/cmp @com_github_google_go_cmp//cmp:cmp"}})
	r := rule.NewRule(LibraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, common.WrapImportSet([]string{"github.com/google/go-cmp/cmp"}), label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "@com_github_google_go_cmp//cmp" {
		t.Errorf("override deps = %q", got)
	}
	if len(l.Errors()) != 0 {
		t.Errorf("errors = %v", l.Errors())
	}
}

func TestResolveDuplicateLeafPackagesStayDistinct(t *testing.T) {
	l := newLang()
	index := resolverIndex(l,
		indexEntry{"a/lib", "lib", "example.test/demo/a/lib"},
		indexEntry{"b/lib", "lib", "example.test/demo/b/lib"},
	)
	cfg := resolverConfig(t, nil)
	r := rule.NewRule(LibraryKind, "app")
	l.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"example.test/demo/a/lib", "example.test/demo/b/lib"}), label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//a/lib,//b/lib" {
		t.Errorf("deps = %q, want //a/lib,//b/lib", got)
	}
	only := rule.NewRule(LibraryKind, "only")
	l.Resolve(cfg, index, nil, only, common.WrapImportSet([]string{"example.test/demo/b/lib"}), label.New("", "app", "only"))
	if got := strings.Join(only.AttrStrings("deps"), ","); got != "//b/lib" {
		t.Errorf("deps = %q, want //b/lib", got)
	}
	l.Resolve(cfg, index, nil, rule.NewRule(LibraryKind, "leaf"), common.WrapImportSet([]string{"lib"}), label.New("", "app", "leaf"))
	if len(l.Errors()) != 1 || !strings.Contains(l.Errors()[0], `unresolved import "lib"`) {
		t.Errorf("leaf import errors = %v, want unresolved", l.Errors())
	}
}

func TestResolveLeafMappingIsNotReinterpreted(t *testing.T) {
	l := newLang()
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "go go lib //lib"}})
	r := rule.NewRule(LibraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, common.WrapImportSet([]string{"example.test/demo/lib"}), label.New("", "app", "app"))
	if r.Attr("deps") != nil {
		t.Errorf("leaf mapping applied to a qualified import: %v", r.AttrStrings("deps"))
	}
	if len(l.Errors()) != 1 || !strings.Contains(l.Errors()[0], "# gazelle:resolve go lib //lib names only the last path element") {
		t.Errorf("errors = %v, want leaf mapping failure", l.Errors())
	}
	exact := rule.NewRule(LibraryKind, "exact")
	l.Resolve(cfg, resolverIndex(l), nil, exact, common.WrapImportSet([]string{"lib"}), label.New("", "app", "exact"))
	if got := strings.Join(exact.AttrStrings("deps"), ","); got != "//lib" || len(l.Errors()) != 1 {
		t.Errorf("exact leaf import deps = %q, errors = %v", got, l.Errors())
	}
}

func TestResolveTestBranches(t *testing.T) {
	l := newLang()
	index := resolverIndex(l,
		indexEntry{"lib/sib", "sib", "example.test/lib/sib"},
		indexEntry{"pkg/demo", "demo", "example.test/pkg/demo"},
	)
	cfg := resolverConfig(t, nil)

	sib := rule.NewRule(TestKind, "demo_test")
	l.Resolve(cfg, index, nil, sib, common.WrapImportSet([]string{"example.test/lib/sib"}), label.New("", "pkg/demo", "demo_test"))
	if got := strings.Join(sib.AttrStrings("deps"), ","); got != "//lib/sib" {
		t.Errorf("test sibling deps = %q, want //lib/sib", got)
	}

	self := rule.NewRule(TestKind, "demo_test")
	self.SetAttr("embed", []string{":demo"})
	l.Resolve(cfg, index, nil, self, common.WrapImportSet([]string{"example.test/pkg/demo"}), label.New("", "pkg/demo", "demo_test"))
	if self.Attr("deps") != nil {
		t.Errorf("same-package test dep was emitted: %v", self.AttrStrings("deps"))
	}

	std := rule.NewRule(TestKind, "uses_std_test")
	l.Resolve(cfg, index, nil, std, common.WrapImportSet([]string{"fmt"}), label.New("", "pkg/demo", "uses_std_test"))
	if std.Attr("deps") != nil || len(l.Errors()) != 0 {
		t.Errorf("stdlib test resolution = %v, errors = %v", std.AttrStrings("deps"), l.Errors())
	}
	l.Resolve(cfg, index, nil, rule.NewRule(TestKind, "unknown_test"), common.WrapImportSet([]string{"example.test/unknown"}), label.New("", "pkg/demo", "unknown_test"))
	if len(l.Errors()) != 1 || !strings.Contains(l.Errors()[0], "unresolved import") {
		t.Errorf("unresolved test errors = %v", l.Errors())
	}
}

func TestResolveTestKeepsNonEmbeddedSamePackageDeps(t *testing.T) {
	from := label.New("", "pkg", "pkg_test")
	cases := []struct {
		name    string
		mapping string
		embed   []string
		want    string
	}{
		{"external same package", "go helper @other//pkg:helper", []string{":pkg"}, "@other//pkg:helper"},
		{"external different package", "go helper @other//different:helper", []string{":pkg"}, "@other//different:helper"},
		{"same repo distinct library", "go helper //pkg:helper", []string{":pkg"}, ":helper"},
		{"embedded library", "go helper //pkg:pkg", []string{":pkg"}, ""},
		{"embedded library absolute", "go helper //pkg:pkg", []string{"//pkg:pkg"}, ""},
		{"self reference", "go helper //pkg:pkg_test", []string{":pkg"}, ""},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			l := newLang()
			cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: tc.mapping}})
			r := rule.NewRule(TestKind, "pkg_test")
			r.SetAttr("embed", tc.embed)
			l.Resolve(cfg, resolverIndex(l), nil, r, common.WrapImportSet([]string{"helper"}), from)
			if got := strings.Join(r.AttrStrings("deps"), ","); got != tc.want {
				t.Errorf("deps = %q, want %q", got, tc.want)
			}
		})
	}
}

func TestResolveIndexedTestKeepsNonEmbeddedSamePackageDeps(t *testing.T) {
	l := newLang()
	index := resolverIndex(l,
		indexEntry{"pkg", "pkg", "example.test/pkg"},
		indexEntry{"pkg", "helper", "example.test/pkg/helper"},
	)
	cfg := resolverConfig(t, nil)
	r := rule.NewRule(TestKind, "pkg_test")
	r.SetAttr("embed", []string{":pkg"})
	l.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"example.test/pkg", "example.test/pkg/helper"}), label.New("", "pkg", "pkg_test"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != ":helper" {
		t.Errorf("deps = %q, want :helper", got)
	}
}

func TestResolveIgnore(t *testing.T) {
	l := newLang()
	l.Before(context.Background())
	cfg := resolverConfig(t, nil)
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import go dropped\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "app", file)
	r := rule.NewRule(LibraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, common.WrapImportSet([]string{"dropped"}), label.New("", "app", "app"))
	if r.Attr("deps") != nil {
		t.Errorf("ignored import emitted deps = %v", r.AttrStrings("deps"))
	}
	l.AfterResolvingDeps(context.Background())
	if len(l.Errors()) != 0 {
		t.Errorf("ignore errors = %v", l.Errors())
	}
}

func TestAfterResolvingDepsSkipsIgnoresOutsideGeneratedPackages(t *testing.T) {
	l := newLang()
	l.Before(context.Background())
	cfg := resolverConfig(t, nil)
	file, err := rule.LoadData("BUILD.bazel", "c", []byte("# gazelle:dx_ignore_import go unused\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "c", file)
	l.GenerateRules(language.GenerateArgs{Config: cfg, Dir: t.TempDir(), Rel: "a"})
	l.AfterResolvingDeps(context.Background())
	if len(l.Errors()) != 0 {
		t.Errorf("errors = %v, want the out-of-scope directive skipped", l.Errors())
	}
}

func TestResolveStaleIgnoreFails(t *testing.T) {
	l := newLang()
	l.Before(context.Background())
	cfg := resolverConfig(t, nil)
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import go unused\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "app", file)
	l.GenerateRules(language.GenerateArgs{Config: cfg, Dir: t.TempDir(), Rel: "app"})
	l.AfterResolvingDeps(context.Background())
	if msg := strings.Join(common.Messages(common.Errors.Take()), "\n"); !strings.Contains(msg, "stale # gazelle:dx_ignore_import") {
		t.Fatalf("errors = %q, want stale ignore failure", msg)
	}
}

func TestResolveAmbiguousFails(t *testing.T) {
	l := newLang()
	index := resolverIndex(l,
		indexEntry{"lib/one", "dup", "example.test/dup"},
		indexEntry{"lib/two", "dup", "example.test/dup"},
	)
	cfg := resolverConfig(t, nil)
	l.Resolve(cfg, index, nil, rule.NewRule(LibraryKind, "app"), common.WrapImportSet([]string{"example.test/dup"}), label.New("", "app", "app"))
	if len(l.Errors()) != 1 || !strings.Contains(l.Errors()[0], "ambiguous import") {
		t.Errorf("ambiguous errors = %v", l.Errors())
	}
}

func TestGenerateCgoStaysHandwritten(t *testing.T) {
	regular := []string{"cgo.go", "helper.go"}
	got := refuseFixture(t, map[string]string{
		"pkg/demo/cgo.go":    "package demo\n\nimport \"C\"\n",
		"pkg/demo/helper.go": "package demo\n",
	}, regular)
	wantRefusal(t, got, "go: pkg/demo: cgo.go imports cgo (\"C\");")
}

func TestGenerateCgoTestStaysHandwritten(t *testing.T) {
	regular := []string{"demo.go", "cgo_test.go"}
	got := refuseFixture(t, map[string]string{
		"pkg/demo/demo.go":     "package demo\n",
		"pkg/demo/cgo_test.go": "package demo_test\n\nimport \"C\"\n",
	}, regular)
	wantRefusal(t, got, "go: pkg/demo: cgo_test.go imports cgo (\"C\");")
}

func TestGeneratePureGoScopeHasNoCgoRaceAttrs(t *testing.T) {
	regular := []string{"demo.go", "demo_test.go"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.go":      "package demo\n",
		"pkg/demo/demo_test.go": "package demo_test\n\nimport \"testing\"\n",
	}, regular)
	if len(result.Gen) != 2 {
		t.Fatalf("generated %d rules, want 2", len(result.Gen))
	}
	for _, r := range result.Gen {
		for _, attr := range []string{"cgo", "pure", "race", "msan", "static", "goos", "goarch", "gotags", "cdeps", "copts", "cxxopts", "clinkopts", "cppopts"} {
			if r.Attr(attr) != nil {
				t.Errorf("%s(%s) carries %q, want pure-Go scope without cgo/race attrs", r.Kind(), r.Name(), attr)
			}
		}
	}
}

func generateAt(t *testing.T, root, rel string, regular []string) language.GenerateResult {
	t.Helper()
	l := newLang()
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, filepath.FromSlash(rel)),
		Rel:          rel,
		RegularFiles: regular,
	})
	if len(l.Errors()) != 0 {
		t.Fatalf("generate %s: %v", rel, l.Errors())
	}
	return result
}

func resolveGenerated(t *testing.T, root string, libSources []string) (string, string) {
	t.Helper()
	l := newLang()
	index := resolve.NewRuleIndex(func(r *rule.Rule, _ string) resolve.Resolver {
		if _, ok := goKinds[r.Kind()]; ok {
			return l
		}
		return nil
	})
	generated := map[string]language.GenerateResult{
		"lib":   generateAt(t, root, "lib", libSources),
		"a/lib": generateAt(t, root, "a/lib", []string{"a.go"}),
		"b/lib": generateAt(t, root, "b/lib", []string{"b.go"}),
		"app":   generateAt(t, root, "app", []string{"app.go", "app_test.go"}),
	}
	for rel, result := range generated {
		for _, r := range result.Gen {
			index.AddRule(config.New(), r, rule.EmptyFile(filepath.Join(rel, "BUILD.bazel"), rel))
		}
	}
	index.Finish()
	cfg := resolverConfig(t, nil)
	for rel, result := range generated {
		for i, r := range result.Gen {
			l.Resolve(cfg, index, nil, r, result.Imports[i], label.New("", rel, r.Name()))
		}
	}
	if len(l.Errors()) != 0 {
		t.Fatalf("resolve errors = %v", l.Errors())
	}
	if deps := generated["lib"].Gen[1].Attr("deps"); deps != nil {
		t.Errorf("external test of its own package gained deps %v", generated["lib"].Gen[1].AttrStrings("deps"))
	}
	app := generated["app"]
	return strings.Join(app.Gen[0].AttrStrings("deps"), ","), strings.Join(app.Gen[1].AttrStrings("deps"), ",")
}

func TestGeneratedLibraryResolvesByImportpathAcrossRenames(t *testing.T) {
	root := t.TempDir()
	for name, content := range map[string]string{
		"go.mod":           "module example.test/demo\n\ngo 1.26\n",
		"lib/impl.go":      "package lib\n",
		"lib/more.go":      "package lib\n\nimport \"strings\"\n\nvar _ = strings.ToUpper\n",
		"lib/impl_test.go": "package lib_test\n\nimport _ \"example.test/demo/lib\"\n",
		"a/lib/a.go":       "package lib\n",
		"b/lib/b.go":       "package lib\n",
		"app/app.go":       "package app\n\nimport (\n\t_ \"example.test/demo/a/lib\"\n\t_ \"example.test/demo/lib\"\n)\n",
		"app/app_test.go":  "package app\n\nimport _ \"example.test/demo/b/lib\"\n",
	} {
		writeFixture(t, root, name, content)
	}
	libDeps, testDeps := resolveGenerated(t, root, []string{"impl.go", "impl_test.go", "more.go"})
	if libDeps != "//a/lib,//lib" || testDeps != "//b/lib" {
		t.Fatalf("deps = %q / %q, want //a/lib,//lib / //b/lib", libDeps, testDeps)
	}
	if err := os.Rename(filepath.Join(root, "lib", "impl.go"), filepath.Join(root, "lib", "renamed.go")); err != nil {
		t.Fatal(err)
	}
	libDeps, testDeps = resolveGenerated(t, root, []string{"impl_test.go", "more.go", "renamed.go"})
	if libDeps != "//a/lib,//lib" || testDeps != "//b/lib" {
		t.Fatalf("deps after rename = %q / %q, want //a/lib,//lib / //b/lib", libDeps, testDeps)
	}
}

func TestGenerateRootPackage(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "go.mod", "module example.com/root/v2\n\ngo 1.26\n")
	writeFixture(t, root, "root.go", "package root\n")
	writeFixture(t, root, "root_test.go", "package root\n\nimport \"testing\"\n")
	result := NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root, RepoName: "demo-repo"},
		Dir:          root,
		Rel:          "",
		RegularFiles: []string{"go.mod", "root.go", "root_test.go"},
	})
	if len(result.Gen) != 2 {
		t.Fatalf("generated %d rules, want go_library and go_test", len(result.Gen))
	}
	if lib := result.Gen[0]; lib.Kind() != LibraryKind || lib.Name() != "root" || lib.AttrString("importpath") != "example.com/root/v2" {
		t.Fatalf("library = %s(%s, importpath=%s), want go_library(root, example.com/root/v2)", lib.Kind(), lib.Name(), lib.AttrString("importpath"))
	}
	if test := result.Gen[1]; test.Kind() != TestKind || test.Name() != "root_test" {
		t.Fatalf("test = %s(%s), want go_test(root_test)", test.Kind(), test.Name())
	}

	bare := t.TempDir()
	writeFixture(t, bare, "root.go", "package root\n")
	result = NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: bare},
		Dir:          bare,
		Rel:          "",
		RegularFiles: []string{"root.go"},
	})
	if len(result.Gen) != 1 || result.Gen[0].Kind() != LibraryKind || result.Gen[0].Name() != "lib" {
		t.Fatalf("no go.mod: generated %+v, want one go_library(lib)", result.Gen)
	}
}

func TestGoRootHint(t *testing.T) {
	cases := map[string]string{
		"module example.com/root/v2\n": "root",
		"module example.com/widget\n":  "widget",
		"module v3\n":                  "v3",
	}
	for mod, want := range cases {
		root := t.TempDir()
		writeFixture(t, root, "go.mod", mod)
		if got := goRootHint(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: root}); got != want {
			t.Errorf("goRootHint(%q) = %q, want %q", mod, got, want)
		}
	}
	if got := goRootHint(language.GenerateArgs{Rel: "pkg", Config: &config.Config{RepoRoot: t.TempDir()}}); got != "" {
		t.Errorf("goRootHint(pkg) = %q, want empty", got)
	}
	if got := goRootHint(language.GenerateArgs{}); got != "" {
		t.Errorf("goRootHint(nil config) = %q, want empty", got)
	}
}
