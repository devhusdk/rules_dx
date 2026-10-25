package kotlin

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

func refusal(t *testing.T, lang language.Language) string {
	t.Helper()
	lm, ok := lang.(language.LifecycleManager)
	if !ok {
		t.Fatalf("%T does not implement language.LifecycleManager", lang)
	}
	var msg string
	func() {
		defer func() {
			r := recover()
			if r == nil {
				t.Fatal("AfterResolvingDeps did not fail the run")
			}
			s, ok := r.(string)
			if !ok {
				t.Fatalf("panic = %v, want a string", r)
			}
			msg = s
		}()
		lm.AfterResolvingDeps(context.Background())
	}()
	const header = "Kotlin generation failed:\n"
	if !strings.HasPrefix(msg, header) {
		t.Fatalf("panic = %q, want prefix %q", msg, header)
	}
	lines := strings.Split(strings.TrimPrefix(msg, header), "\n")
	if len(lines) != 1 {
		t.Fatalf("refusals = %q, want exactly one", lines)
	}
	return lines[0]
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
	regular := []string{"Demo.kt", "Helper.kt", "notes.txt", "DemoTest.kt"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/Demo.kt":     "package demo\n\nimport com.example.Widget\n\nclass Demo\n",
		"pkg/demo/Helper.kt":   "package demo\n\nimport java.util.List\n\nclass Helper\n",
		"pkg/demo/notes.txt":   "not a source\n",
		"pkg/demo/DemoTest.kt": "package demo\n\nimport org.junit.Test\n\nclass DemoTest\n",
	}, regular)
	if len(result.Gen) != 1 || len(result.Imports) != 1 {
		t.Fatalf("generated %d rules and %d import sets, want 1 each", len(result.Gen), len(result.Imports))
	}
	lib := result.Gen[0]
	if lib.Kind() != LibraryKind || lib.Name() != "demo" {
		t.Fatalf("library = %s(%s), want kotlin_library(demo)", lib.Kind(), lib.Name())
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "Demo.kt,Helper.kt" {
		t.Errorf("library srcs = %q, want Demo.kt,Helper.kt", got)
	}
	got := result.Imports[0].(common.ImportSet)
	if strings.Join(got.Names, ",") != "com.example.Widget" {
		t.Errorf("library imports = %+v, want [com.example.Widget]", got.Names)
	}
}

func TestGenerateTestSourcesExcluded(t *testing.T) {
	regular := []string{"Demo.kt", "DemoTest.kt"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/Demo.kt":     "package demo\n\nclass Demo\n",
		"pkg/demo/DemoTest.kt": "package demo\n\nimport com.example.OnlyByTest\n\nclass DemoTest\n",
	}, regular)
	if len(result.Gen) != 1 || len(result.Imports) != 1 {
		t.Fatalf("generated %d rules and %d import sets, want 1 each", len(result.Gen), len(result.Imports))
	}
	if got := strings.Join(result.Gen[0].AttrStrings("srcs"), ","); got != "Demo.kt" {
		t.Errorf("library srcs = %q, want Demo.kt", got)
	}
	if got := result.Imports[0].(common.ImportSet); len(got.Names) != 0 {
		t.Errorf("library imports = %+v, want empty", got.Names)
	}
}

func TestGenerateMixedPackagesFail(t *testing.T) {
	regular := []string{"A.kt", "B.kt"}
	got := refuseFixture(t, map[string]string{
		"pkg/demo/A.kt": "package alpha\n\nclass A\n",
		"pkg/demo/B.kt": "package beta\n\nclass B\n",
	}, regular)
	wantRefusal(t, got, "kotlin: pkg/demo: mixed packages alpha, beta in one directory;")
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
	regular := []string{"Demo.kt", "Main.kt"}
	got := refuseFixture(t, map[string]string{
		"pkg/demo/Demo.kt": "package demo\n\nclass Demo\n",
		"pkg/demo/Main.kt": "package demo\n\nobject Main {\n  fun main() {}\n}\n",
	}, regular)
	wantRefusal(t, got, "kotlin: pkg/demo: Main.kt defines main;")
}

func TestGenerateKindMismatchFails(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/Demo.kt", "package demo\n\nclass Demo\n")
	existing := &rule.File{}
	fg := rule.NewRule("filegroup", "demo")
	existing.Rules = append(existing.Rules, fg)
	lang := NewLanguage()
	result := lang.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"Demo.kt"},
		File:         existing,
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules over a kind mismatch, want 0 with a recorded failure", len(result.Gen))
	}
	wantRefusal(t, refusal(t, lang), "kotlin: pkg/demo: target name \"demo\" is claimed by generated kotlin_library(pkg/demo) and existing filegroup")
}

func TestImportsIndexNonTestSources(t *testing.T) {
	lang := NewLanguage()
	lib := rule.NewRule(LibraryKind, "demo")
	lib.SetAttr("srcs", []string{"Demo.kt", "Helper.kt", "DemoTest.kt", "notes.txt"})
	lib.SetPrivateAttr(common.DeclaredAttr, map[string][]string{
		"Demo.kt":     {"demo.Demo"},
		"Helper.kt":   {"demo.Helper", "demo.helper"},
		"DemoTest.kt": {"demo.DemoTest"},
	})
	imports := lang.Imports(&config.Config{}, lib, nil)
	var got []string
	for _, imp := range imports {
		if imp.Lang != languageName {
			t.Fatalf("import lang = %q", imp.Lang)
		}
		got = append(got, imp.Imp)
	}
	if strings.Join(got, ",") != "demo.Demo,demo.Helper,demo.helper" {
		t.Errorf("imports = %v, want the declared qualified names", got)
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
	for _, value := range []string{"kotlin Dropped", "kotlin kotlin Second", "other Foreign", ""} {
		lang.Configure(cfg, "app", &rule.File{Directives: []rule.Directive{{Key: "dx_ignore_import", Value: value}}})
	}
	if lang.Failed() {
		t.Fatal("a foreign or empty ignore was rejected")
	}
	conf := cfg.Exts[languageName].(*common.IgnoreConfig)
	if len(conf.Ignores) != 2 || conf.Ignores[0].Value != "Dropped" || conf.Ignores[1].Value != "Second" {
		t.Fatalf("ignores = %+v, want Dropped and Second", conf.Ignores)
	}
	lang.Configure(cfg, "app", &rule.File{Directives: []rule.Directive{{Key: "dx_ignore_import", Value: "kotlin"}}})
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
	lang.Fail("kotlin: boom")
	defer func() {
		r := recover()
		if r == nil {
			t.Fatal("AfterResolvingDeps did not panic with recorded errors")
		}
		if msg, ok := r.(string); !ok || !strings.Contains(msg, "Kotlin generation failed") {
			t.Fatalf("panic = %v, want Kotlin generation failure", r)
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
	if len(loads) != 1 || loads[0].Name != "@renamed_dx//kotlin/rules:defs.bzl" || strings.Join(loads[0].Symbols, ",") != LibraryKind {
		t.Errorf("apparent loads = %+v", loads)
	}
	if defaults := lang.Loads(); len(defaults) != 1 || defaults[0].Name != "@rules_dx//kotlin/rules:defs.bzl" {
		t.Errorf("default loads = %+v", defaults)
	}
}

func generateAndResolve(t *testing.T, directives []rule.Directive) (*rule.Rule, *common.SingleLang) {
	t.Helper()
	return generateAndResolveSource(t, "package demo\n\nimport java.util.List\n\nclass Demo\n", directives)
}

func generateAndResolveSource(t *testing.T, src string, directives []rule.Directive) (*rule.Rule, *common.SingleLang) {
	t.Helper()
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/Demo.kt", src)
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
		RegularFiles: []string{"Demo.kt"},
	})
	if len(result.Gen) != 1 || len(result.Imports) != 1 {
		t.Fatalf("generated %d rules and %d import sets, want 1 each", len(result.Gen), len(result.Imports))
	}
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return nil })
	lang.Resolve(cfg, index, nil, result.Gen[0], result.Imports[0], label.New("", "pkg/demo", "demo"))
	return result.Gen[0], lang
}

func TestGeneratedStdLibImportUsesExactMapping(t *testing.T) {
	value := "kotlin kotlin java.util.List //third_party:external"
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

func resolveSetup(t *testing.T, directives string) (*common.SingleLang, *config.Config, *resolve.RuleIndex, string) {
	t.Helper()
	root := t.TempDir()
	writeFixture(t, root, "util/Widget.kt", "package com.example\n\nclass Widget\n\nfun tool() = 1\n")
	lang := NewLanguage().(*common.SingleLang)
	lang.Before(context.Background())
	cfg := config.New()
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	f, err := rule.LoadData(filepath.Join(root, "consumer", "BUILD.bazel"), "consumer", []byte(directives))
	if err != nil {
		t.Fatal(err)
	}
	resolver.Configure(cfg, "consumer", f)
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return lang })
	lib := rule.NewRule(LibraryKind, "util")
	lib.SetAttr("srcs", []string{"Widget.kt"})
	uf, err := rule.LoadData(filepath.Join(root, "util", "BUILD.bazel"), "util", nil)
	if err != nil {
		t.Fatal(err)
	}
	index.AddRule(cfg, lib, uf)
	index.Finish()
	return lang, cfg, index, root
}

func TestResolveLocalQualifiedNames(t *testing.T) {
	lang, cfg, index, _ := resolveSetup(t, "")
	from := label.New("", "consumer", "consumer")
	r := rule.NewRule(LibraryKind, "consumer")
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"com.example.Widget", "com.example.Widget.Nested", "com.example.tool"}), from)
	if lang.Failed() {
		t.Fatal("a local qualified import was not resolved")
	}
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//util" {
		t.Fatalf("deps = %q, want //util", got)
	}

	r = rule.NewRule(LibraryKind, "consumer")
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"other.example.Widget"}), from)
	wantRefusal(t, refusal(t, lang), `"other.example.Widget"`)
}

func TestResolveExternalNamespaceUsesMappingAndFailsWhenUnmapped(t *testing.T) {
	from := label.New("", "consumer", "consumer")
	for _, mapping := range []string{"kotlinx.coroutines.runBlocking", "kotlinx.coroutines"} {
		lang, cfg, index, _ := resolveSetup(t, "# gazelle:resolve kotlin "+mapping+" @maven//:external\n")
		r := rule.NewRule(LibraryKind, "consumer")
		lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"kotlinx.coroutines.runBlocking"}), from)
		if lang.Failed() {
			t.Fatalf("an import mapped by %s was reported", mapping)
		}
		if got := r.AttrStrings("deps"); len(got) != 1 || got[0] != "@maven//:external" {
			t.Fatalf("deps for %s = %v", mapping, got)
		}
	}
	lang, cfg, index, _ := resolveSetup(t, "")
	r := rule.NewRule(LibraryKind, "consumer")
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"kotlinx.coroutines.runBlocking"}), from)
	wantRefusal(t, refusal(t, lang), `"kotlinx.coroutines.runBlocking"`)
	if got := r.AttrStrings("deps"); len(got) != 0 {
		t.Fatalf("deps = %v", got)
	}
}

func generateRoot(t *testing.T, repoName string, files map[string]string) language.GenerateResult {
	t.Helper()
	root := t.TempDir()
	var regular []string
	for name, content := range files {
		if err := os.WriteFile(filepath.Join(root, name), []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
		regular = append(regular, name)
	}
	return NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root, RepoName: repoName},
		Dir:          root,
		Rel:          "",
		RegularFiles: regular,
	})
}

func TestGenerateRootPackage(t *testing.T) {
	for _, tc := range []struct{ repoName, want string }{{"demo-repo", "demo_repo"}, {"", "lib"}} {
		result := generateRoot(t, tc.repoName, map[string]string{"Lib.kt": "package lib\n\nclass Lib\n"})
		if len(result.Gen) != 1 || result.Gen[0].Kind() != LibraryKind || result.Gen[0].Name() != tc.want {
			t.Fatalf("repo %q: generated %+v, want one kotlin_library(%s)", tc.repoName, result.Gen, tc.want)
		}
		if got := strings.Join(result.Gen[0].AttrStrings("srcs"), ","); got != "Lib.kt" {
			t.Errorf("repo %q: srcs = %q, want Lib.kt", tc.repoName, got)
		}
	}
}

func TestGeneratedStdLibImportUsesPackageMapping(t *testing.T) {
	r, lang := generateAndResolve(t, []rule.Directive{{Key: "resolve", Value: "kotlin kotlin java.util //third_party:external"}})
	if lang.Failed() {
		t.Fatal("import mapped by its package was reported")
	}
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//third_party:external" {
		t.Errorf("deps = %q, want the package mapping", got)
	}
}

func TestGeneratedLookalikeImportsNeedMapping(t *testing.T) {
	src := "package demo\n\nimport kotlin.metadata.jvm.KotlinClassMetadata\nimport kotlin.test.*\nimport kotlinx.coroutines.runBlocking\nimport kotlin.math.max\nimport kotlin.collections.*\n\nclass Demo\n"
	directives := []rule.Directive{{Key: "resolve", Value: "kotlin kotlin kotlin.metadata @maven//:metadata"}, {Key: "resolve", Value: "kotlin kotlin kotlin.test @maven//:kotlin_test"}, {Key: "resolve", Value: "kotlin kotlin kotlinx @maven//:kotlinx"}}
	r, lang := generateAndResolveSource(t, src, directives)
	if lang.Failed() {
		t.Fatal("mapped lookalike imports were reported")
	}
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "@maven//:kotlin_test,@maven//:kotlinx,@maven//:metadata" {
		t.Errorf("deps = %q, want every mapped lookalike namespace", got)
	}
	_, lang = generateAndResolveSource(t, src, directives[1:])
	wantRefusal(t, refusal(t, lang), `"kotlin.metadata.jvm.KotlinClassMetadata"`)
}
