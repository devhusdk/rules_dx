package java

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/ralvik/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

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

func TestGeneratePackageLevelLibrary(t *testing.T) {
	regular := []string{"Demo.java", "Helper.java", "notes.txt", "DemoTest.java"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/Demo.java":     "package demo;\n\nimport com.example.Widget;\n\npublic class Demo {}\n",
		"pkg/demo/Helper.java":   "package demo;\n\nimport java.util.List;\n\npublic class Helper {}\n",
		"pkg/demo/notes.txt":     "not a source\n",
		"pkg/demo/DemoTest.java": "package demo;\n\nimport org.junit.Test;\n\npublic class DemoTest {}\n",
	}, regular)
	if len(result.Gen) != 1 || len(result.Imports) != 1 {
		t.Fatalf("generated %d rules and %d import sets, want 1 each", len(result.Gen), len(result.Imports))
	}
	lib := result.Gen[0]
	if lib.Kind() != LibraryKind || lib.Name() != "demo" {
		t.Fatalf("library = %s(%s), want java_library(demo)", lib.Kind(), lib.Name())
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "Demo.java,Helper.java" {
		t.Errorf("library srcs = %q, want Demo.java,Helper.java", got)
	}
	got := result.Imports[0].(common.ImportSet)
	if strings.Join(got.Names, ",") != "Widget" {
		t.Errorf("library imports = %+v, want [Widget]", got.Names)
	}
}

func TestGenerateTestSourcesExcluded(t *testing.T) {
	regular := []string{"Demo.java", "DemoTest.java"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/Demo.java":     "package demo;\n\npublic class Demo {}\n",
		"pkg/demo/DemoTest.java": "package demo;\n\nimport com.example.OnlyByTest;\n\npublic class DemoTest {}\n",
	}, regular)
	if len(result.Gen) != 1 || len(result.Imports) != 1 {
		t.Fatalf("generated %d rules and %d import sets, want 1 each", len(result.Gen), len(result.Imports))
	}
	if got := strings.Join(result.Gen[0].AttrStrings("srcs"), ","); got != "Demo.java" {
		t.Errorf("library srcs = %q, want Demo.java", got)
	}
	if got := result.Imports[0].(common.ImportSet); len(got.Names) != 0 {
		t.Errorf("library imports = %+v, want empty", got.Names)
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

func TestGenerateMainFails(t *testing.T) {
	regular := []string{"Demo.java", "Main.java"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/Demo.java": "package demo;\n\npublic class Demo {}\n",
		"pkg/demo/Main.java": "package demo;\n\npublic class Main {\n  public static void main(String[] args) {}\n}\n",
	}, regular)
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules for a main mix, want 0 with a recorded failure", len(result.Gen))
	}
}

func TestGenerateMixedPackagesFail(t *testing.T) {
	regular := []string{"A.java", "B.java"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/A.java": "package alpha;\n\npublic class A {}\n",
		"pkg/demo/B.java": "package beta;\n\npublic class B {}\n",
	}, regular)
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules for mixed packages, want 0 with a recorded failure", len(result.Gen))
	}
}

func TestGenerateKindMismatchFails(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/Demo.java", "package demo;\n\npublic class Demo {}\n")
	existing := &rule.File{}
	fg := rule.NewRule("filegroup", "demo")
	existing.Rules = append(existing.Rules, fg)
	result := NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"Demo.java"},
		File:         existing,
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules over a kind mismatch, want 0 with a recorded failure", len(result.Gen))
	}
}

func TestImportsIndexNonTestSources(t *testing.T) {
	lang := NewLanguage()
	lib := rule.NewRule(LibraryKind, "demo")
	lib.SetAttr("srcs", []string{"Demo.java", "Helper.java", "DemoTest.java", "notes.txt"})
	imports := lang.Imports(&config.Config{}, lib, nil)
	if len(imports) != 2 || imports[0].Lang != languageName || imports[0].Imp != "Demo" || imports[1].Imp != "Helper" {
		t.Errorf("imports = %+v, want Demo+Helper identities", imports)
	}
	if got := lang.Imports(&config.Config{}, rule.NewRule("filegroup", "demo"), nil); got != nil {
		t.Errorf("other-kind imports = %+v, want nil", got)
	}
	if got := lang.Imports(&config.Config{}, rule.NewRule(LibraryKind, "empty"), nil); got != nil {
		t.Errorf("srcless imports = %+v, want nil", got)
	}
}

func TestIgnoreDirectiveUsesThisLanguageName(t *testing.T) {
	lang := NewLanguage().(*common.SingleLang)
	lang.Before(context.Background())
	cfg := config.New()
	for _, value := range []string{"java Dropped", "java java Second", "kotlin Foreign", ""} {
		lang.Configure(cfg, "app", &rule.File{Directives: []rule.Directive{{Key: "dx_ignore_import", Value: value}}})
	}
	if lang.Failed() {
		t.Fatal("a foreign or empty ignore was rejected")
	}
	conf := cfg.Exts[languageName].(*common.IgnoreConfig)
	if len(conf.Ignores) != 2 || conf.Ignores[0].Value != "Dropped" || conf.Ignores[1].Value != "Second" {
		t.Fatalf("ignores = %+v, want Dropped and Second", conf.Ignores)
	}
	lang.Configure(cfg, "app", &rule.File{Directives: []rule.Directive{{Key: "dx_ignore_import", Value: "java"}}})
	if !lang.Failed() {
		t.Fatal("a one-field ignore was accepted")
	}
	if got := CollectUsedIgnores(cfg); got != nil {
		t.Fatalf("unused ignores = %v, want none", got)
	}
}

func TestAfterResolvingDepsReportsFailures(t *testing.T) {
	lang := NewLanguage().(*common.SingleLang)
	lang.Before(context.Background())
	lang.Fail("java: boom")
	defer func() {
		r := recover()
		if r == nil {
			t.Fatal("AfterResolvingDeps did not panic with recorded errors")
		}
		if msg, ok := r.(string); !ok || !strings.Contains(msg, "Java generation failed") {
			t.Fatalf("panic = %v, want Java generation failure", r)
		}
	}()
	lang.AfterResolvingDeps(context.Background())
}

func TestResolveSkipsStdlibImports(t *testing.T) {
	lang := NewLanguage().(*common.SingleLang)
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
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"java.util.List"}), label.New("", "app", "app"))
	if lang.Failed() {
		t.Fatal("a stdlib import was reported")
	}
	lang.AfterResolvingDeps(context.Background())
}

func TestLanguageMetadata(t *testing.T) {
	lang := NewLanguage().(*common.SingleLang)
	if lang.Name() != languageName || len(lang.Kinds()) != 1 {
		t.Fatalf("metadata = %s / %v", lang.Name(), lang.Kinds())
	}
	loads := lang.ApparentLoads(func(name string) string {
		if name == "rules_dx" {
			return "renamed_dx"
		}
		return ""
	})
	if len(loads) != 1 || loads[0].Name != "@renamed_dx//java/rules:defs.bzl" || strings.Join(loads[0].Symbols, ",") != LibraryKind {
		t.Errorf("apparent loads = %+v", loads)
	}
	if defaults := lang.Loads(); len(defaults) != 1 || defaults[0].Name != "@rules_dx//java/rules:defs.bzl" {
		t.Errorf("default loads = %+v", defaults)
	}
}
