package java

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/devhusdk/rules_dx/gazelle/common"

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
	cfg := config.New()
	cfg.RepoRoot = root
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	resolver.Configure(cfg, "pkg/demo", nil)
	return NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       cfg,
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
	if strings.Join(got.Names, ",") != "com.example.Widget" {
		t.Errorf("library imports = %+v, want [com.example.Widget]", got.Names)
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
	lib.SetPrivateAttr(common.DeclaredAttr, map[string][]string{
		"Demo.java":     {"demo.Demo"},
		"Helper.java":   {"demo.Helper", "demo.Extra"},
		"DemoTest.java": {"demo.DemoTest"},
	})
	imports := lang.Imports(&config.Config{}, lib, nil)
	var got []string
	for _, imp := range imports {
		if imp.Lang != languageName {
			t.Fatalf("import lang = %q", imp.Lang)
		}
		got = append(got, imp.Imp)
	}
	if strings.Join(got, ",") != "demo.Demo,demo.Helper,demo.Extra" {
		t.Errorf("imports = %v, want the declared qualified types", got)
	}
	root := t.TempDir()
	writeFixture(t, root, "pkg/Widget.java", "package com.example;\n\npublic class Widget {}\n")
	f, err := rule.LoadData(filepath.Join(root, "pkg", "BUILD.bazel"), "pkg", nil)
	if err != nil {
		t.Fatal(err)
	}
	disk := rule.NewRule(LibraryKind, "pkg")
	disk.SetAttr("srcs", []string{"Widget.java", "Missing.java"})
	if got := lang.Imports(&config.Config{}, disk, f); len(got) != 1 || got[0].Imp != "com.example.Widget" {
		t.Errorf("on-disk imports = %+v, want com.example.Widget", got)
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

func TestResolveExternalNamespaceUsesMappingAndFailsWhenUnmapped(t *testing.T) {
	setup := func(directives string) (*common.SingleLang, *config.Config, *resolve.RuleIndex) {
		lang := NewLanguage().(*common.SingleLang)
		lang.Before(context.Background())
		cfg := config.New()
		resolver := &resolve.Configurer{}
		resolver.RegisterFlags(nil, "update", cfg)
		if err := resolver.CheckFlags(nil, cfg); err != nil {
			t.Fatal(err)
		}
		f, err := rule.LoadData("BUILD.bazel", "", []byte(directives))
		if err != nil {
			t.Fatal(err)
		}
		resolver.Configure(cfg, "app", f)
		return lang, cfg, resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return nil })
	}
	from := label.New("", "app", "app")
	for _, mapping := range []string{"javax.inject.Inject", "javax.inject"} {
		lang, cfg, index := setup("# gazelle:resolve java " + mapping + " @maven//:javax_inject_javax_inject\n")
		r := rule.NewRule(LibraryKind, "app")
		lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"javax.inject.Inject"}), from)
		if lang.Failed() {
			t.Fatalf("an import mapped by %s was reported", mapping)
		}
		if got := r.AttrStrings("deps"); len(got) != 1 || got[0] != "@maven//:javax_inject_javax_inject" {
			t.Fatalf("deps for %s = %v", mapping, got)
		}
	}

	for _, directives := range []string{"", "# gazelle:resolve java Inject @maven//:javax_inject_javax_inject\n"} {
		lang, cfg, index := setup(directives)
		r := rule.NewRule(LibraryKind, "app")
		lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"javax.inject.Inject"}), from)
		if !lang.Failed() {
			t.Fatalf("an unresolved external import was not reported with %q", directives)
		}
		if got := r.AttrStrings("deps"); len(got) != 0 {
			t.Fatalf("deps = %v", got)
		}
	}
}

func TestResolveStaticImportUsesDeclaringType(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "util/Widget.java", "package com.example;\npublic class Widget { public static int create() { return 1; } }\n")
	writeFixture(t, root, "other/create.java", "package other;\npublic class create {}\n")
	lang := NewLanguage().(*common.SingleLang)
	lang.Before(context.Background())
	cfg := config.New()
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	resolver.Configure(cfg, "consumer", &rule.File{})
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return lang })
	for _, pkg := range []string{"util", "other"} {
		r := rule.NewRule(LibraryKind, pkg)
		src := "Widget.java"
		if pkg == "other" {
			src = "create.java"
		}
		r.SetAttr("srcs", []string{src})
		f, err := rule.LoadData(filepath.Join(root, pkg, "BUILD.bazel"), pkg, nil)
		if err != nil {
			t.Fatal(err)
		}
		index.AddRule(cfg, r, f)
	}
	index.Finish()
	r := rule.NewRule(LibraryKind, "consumer")
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"com.example.Widget"}), label.New("", "consumer", "consumer"))
	if lang.Failed() {
		t.Fatal("static import was not resolved")
	}
	if got := r.AttrStrings("deps"); len(got) != 1 || got[0] != "//util" {
		t.Fatalf("deps = %v", got)
	}

	r = rule.NewRule(LibraryKind, "consumer")
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"com.example.Widget.Inner", "other.create"}), label.New("", "consumer", "consumer"))
	if lang.Failed() {
		t.Fatal("a nested type was not resolved through its outer type")
	}
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//other,//util" {
		t.Fatalf("deps = %v", got)
	}

	r = rule.NewRule(LibraryKind, "consumer")
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"com.example.Missing"}), label.New("", "consumer", "consumer"))
	if !lang.Failed() {
		t.Fatal("an unresolved declaring type was not reported")
	}
	if got := r.AttrStrings("deps"); len(got) != 0 {
		t.Fatalf("deps = %v", got)
	}
}

func generateAndResolve(t *testing.T, directives []rule.Directive) (*rule.Rule, *common.SingleLang) {
	t.Helper()
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/Demo.java", "package demo;\n\nimport javax.annotation.processing.Generated;\n\npublic class Demo {}\n")
	cfg := config.New()
	cfg.RepoRoot = root
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	resolver.Configure(cfg, "pkg/demo", &rule.File{Directives: directives})
	lang := NewLanguage().(*common.SingleLang)
	lang.Before(context.Background())
	result := lang.GenerateRules(language.GenerateArgs{
		Config:       cfg,
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"Demo.java"},
	})
	if len(result.Gen) != 1 || len(result.Imports) != 1 {
		t.Fatalf("generated %d rules and %d import sets, want 1 each", len(result.Gen), len(result.Imports))
	}
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return nil })
	lang.Resolve(cfg, index, nil, result.Gen[0], result.Imports[0], label.New("", "pkg/demo", "demo"))
	return result.Gen[0], lang
}

func TestGeneratedStdLibImportUsesExactMapping(t *testing.T) {
	value := "java java javax.annotation.processing.Generated //third_party:external"
	r, lang := generateAndResolve(t, []rule.Directive{{Key: "resolve", Value: value}})
	if lang.Failed() {
		t.Fatal("mapped import was reported")
	}
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//third_party:external" {
		t.Errorf("deps = %q, want the mapped dependency", got)
	}
}

func TestGeneratedStdLibImportWithoutMappingIsSkipped(t *testing.T) {
	r, lang := generateAndResolve(t, nil)
	if lang.Failed() {
		t.Fatal("standard library import was reported")
	}
	if r.Attr("deps") != nil {
		t.Errorf("deps = %v, want none", r.AttrStrings("deps"))
	}
}
