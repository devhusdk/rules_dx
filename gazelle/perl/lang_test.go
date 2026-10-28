package perl

import (
	"context"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

func writeFixture(t *testing.T, root string, files map[string]string) {
	t.Helper()
	for name, content := range files {
		file := filepath.Join(root, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(file), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(file, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}
}

func generate(t *testing.T, rel string, files map[string]string, regular []string) language.GenerateResult {
	t.Helper()
	root := t.TempDir()
	writeFixture(t, root, files)
	return NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, filepath.FromSlash(rel)),
		Rel:          rel,
		RegularFiles: regular,
	})
}

func TestGenerateLibraryAndTests(t *testing.T) {
	result := generate(t, "pkg/demo", map[string]string{
		"pkg/demo/Demo.pm":                "use Demo::Sub;\n",
		"pkg/demo/lib/Demo/Sub.pm":        "use Text::CSV;\nuse strict;\n",
		"pkg/demo/t/sub.t":                "use Test::More;\nuse Demo::Sub;\n",
		"pkg/demo/lib/Nested/BUILD.bazel": "",
		"pkg/demo/lib/Nested/Own.pm":      "1;\n",
		"pkg/demo/bin/run.pl":             "use Demo;\n",
	}, []string{"Demo.pm", "notes.txt"})
	if len(result.Gen) != 2 || len(result.Imports) != 2 {
		t.Fatalf("generated %d rules and %d import sets, want 2 each", len(result.Gen), len(result.Imports))
	}
	lib, test := result.Gen[0], result.Gen[1]
	if lib.Kind() != LibraryKind || lib.Name() != "demo" {
		t.Errorf("library = %s(%s), want perl_library(demo)", lib.Kind(), lib.Name())
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "Demo.pm,lib/Demo/Sub.pm" {
		t.Errorf("library srcs = %q", got)
	}
	if test.Kind() != TestKind || test.Name() != "sub_test" {
		t.Errorf("test = %s(%s), want perl_test(sub_test)", test.Kind(), test.Name())
	}
	libImports := result.Imports[0].(common.ImportSet).Names
	if !reflect.DeepEqual(libImports, []string{"Demo::Sub", "Text::CSV", "strict"}) {
		t.Errorf("library imports = %v", libImports)
	}
}

func TestGenerateSkipsDirectoriesOwnedByParent(t *testing.T) {
	files := map[string]string{
		"pkg/lib/Acme/Widget.pm": "1;\n",
		"pkg/t/x.t":              "1;\n",
	}
	for _, rel := range []string{"pkg/lib", "pkg/lib/Acme", "pkg/t"} {
		result := generate(t, rel, files, []string{"Widget.pm"})
		if len(result.Gen) != 0 || len(result.Empty) != 0 {
			t.Errorf("%s generated %d rules, want none", rel, len(result.Gen))
		}
	}
}

func TestGenerateNestedPackageOwnsItsModules(t *testing.T) {
	result := generate(t, "pkg/lib/Acme", map[string]string{
		"pkg/lib/Acme/BUILD.bazel": "",
		"pkg/lib/Acme/Widget.pm":   "1;\n",
	}, []string{"BUILD.bazel", "Widget.pm"})
	if len(result.Gen) != 1 || result.Gen[0].Name() != "Acme" {
		t.Fatalf("generated %v, want perl_library(Acme)", result.Gen)
	}
}

func TestGenerateNeverEmitsBinaries(t *testing.T) {
	result := generate(t, "pkg/tool", map[string]string{
		"pkg/tool/run.pl": "use strict;\n",
	}, []string{"run.pl"})
	if len(result.Gen) != 0 {
		t.Errorf("generated %d rules for a script-only directory, want none", len(result.Gen))
	}
}

func TestImportsIndexModuleNames(t *testing.T) {
	lib := rule.NewRule(LibraryKind, "demo")
	lib.SetAttr("srcs", []string{"Demo.pm", "lib/Demo/Sub.pm", "notes.txt"})
	var got []string
	for _, spec := range NewLanguage().Imports(&config.Config{}, lib, nil) {
		got = append(got, spec.Imp)
	}
	if !reflect.DeepEqual(got, []string{"Demo", "Demo::Sub"}) {
		t.Errorf("imports = %v", got)
	}
	if specs := NewLanguage().Imports(&config.Config{}, rule.NewRule(TestKind, "x"), nil); specs != nil {
		t.Errorf("test imports = %v, want nil", specs)
	}
}

func resolveImports(t *testing.T, names []string) (*perlLang, *rule.Rule) {
	t.Helper()
	lang := NewLanguage().(*perlLang)
	lang.Before(context.Background())
	cfg := config.New()
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	resolver.Configure(cfg, "app", &rule.File{})
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return nil })
	r := rule.NewRule(LibraryKind, "app")
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet(names), label.New("", "app", "app"))
	return lang, r
}

func TestResolveSkipsCoreModules(t *testing.T) {
	lang, r := resolveImports(t, []string{"strict", "File::Spec", "Test::More"})
	if lang.Failed() {
		t.Fatalf("core imports were reported: %v", lang.Errors())
	}
	if deps := r.AttrStrings("deps"); len(deps) != 0 {
		t.Errorf("deps = %v, want none", deps)
	}
}

func TestResolveReportsCPANImportWithHint(t *testing.T) {
	lang, _ := resolveImports(t, []string{"Text::CSV"})
	errs := lang.Errors()
	if len(errs) != 1 || !strings.Contains(errs[0], "Text::CSV") || !strings.Contains(errs[0], "gazelle:resolve") {
		t.Errorf("errors = %v, want one unresolved Text::CSV with a resolve hint", errs)
	}
}
