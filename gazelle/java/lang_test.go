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

func TestGenerateMainFails(t *testing.T) {
	regular := []string{"Demo.java", "Main.java"}
	got := refuseFixture(t, map[string]string{
		"pkg/demo/Demo.java": "package demo;\n\npublic class Demo {}\n",
		"pkg/demo/Main.java": "package demo;\n\npublic class Main {\n  public static void main(String[] args) {}\n}\n",
	}, regular)
	wantRefusal(t, got, "java: pkg/demo: Main.java defines main;")
}

func TestGenerateMixedPackagesFail(t *testing.T) {
	regular := []string{"A.java", "B.java"}
	got := refuseFixture(t, map[string]string{
		"pkg/demo/A.java": "package alpha;\n\npublic class A {}\n",
		"pkg/demo/B.java": "package beta;\n\npublic class B {}\n",
	}, regular)
	wantRefusal(t, got, "java: pkg/demo: mixed packages alpha, beta in one directory;")
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
	imports := common.WrapImportSet([]string{"javax.inject.Inject"})
	from := label.New("", "app", "app")

	lang, cfg, index := setup("# gazelle:resolve java javax.inject @maven//:javax_inject_javax_inject\n")
	r := rule.NewRule(LibraryKind, "app")
	lang.Resolve(cfg, index, nil, r, imports, from)
	if lang.Failed() {
		t.Fatal("a mapped external import was reported")
	}
	if got := r.AttrStrings("deps"); len(got) != 1 || got[0] != "@maven//:javax_inject_javax_inject" {
		t.Fatalf("deps = %v", got)
	}

	for _, directives := range []string{"", "# gazelle:resolve java Inject @maven//:javax_inject_javax_inject\n", "# gazelle:resolve java javax @maven//:javax_inject_javax_inject\n"} {
		lang, cfg, index = setup(directives)
		r = rule.NewRule(LibraryKind, "app")
		lang.Resolve(cfg, index, nil, r, imports, from)
		if !lang.Failed() || len(r.AttrStrings("deps")) != 0 {
			t.Fatalf("%q: an unresolved external import was not reported (deps %v)", directives, r.AttrStrings("deps"))
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
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"com.missing.Widget"}), label.New("", "consumer", "consumer"))
	if !lang.Failed() {
		t.Fatal("an unresolved declaring type was not reported")
	}
	if got := r.AttrStrings("deps"); len(got) != 0 {
		t.Fatalf("deps = %v", got)
	}
}

func TestResolveQualifiedImportPrefersPackage(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "a/Util.java", "package com.a;\npublic class Util {}\n")
	writeFixture(t, root, "b/Util.java", "package com.b;\npublic class Util {}\nclass Helper {}\n")
	writeFixture(t, root, "nested/Outer.java", "package com.a;\npublic class Outer { public static class Inner {} }\n")
	lang := NewLanguage().(*common.SingleLang)
	lang.Before(context.Background())
	cfg := config.New()
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	directives, err := rule.LoadData("BUILD.bazel", "", []byte("# gazelle:resolve java com.x @maven//:javax_inject_javax_inject\n"))
	if err != nil {
		t.Fatal(err)
	}
	resolver.Configure(cfg, "consumer", directives)
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return lang })
	for pkg, src := range map[string]string{"a": "Util.java", "b": "Util.java", "nested": "Outer.java"} {
		r := rule.NewRule(LibraryKind, pkg)
		r.SetAttr("srcs", []string{src})
		f, err := rule.LoadData(filepath.Join(root, pkg, "BUILD.bazel"), pkg, nil)
		if err != nil {
			t.Fatal(err)
		}
		index.AddRule(cfg, r, f)
	}
	index.Finish()
	from := label.New("", "consumer", "consumer")
	for _, tc := range []struct {
		imp  string
		want string
	}{
		{"com.a.Util", "//a"},
		{"com.b.Util", "//b"},
		{"com.a.Outer.Inner", "//nested"},
		{"com.b.Helper", "//b"},
		{"com.b.*", "//b"},
		{"com.x.Inject", "@maven//:javax_inject_javax_inject"},
	} {
		r := rule.NewRule(LibraryKind, "consumer")
		lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{tc.imp}), from)
		if got := r.AttrStrings("deps"); len(got) != 1 || got[0] != tc.want {
			t.Errorf("%s: deps = %v, want %s", tc.imp, got, tc.want)
		}
	}
	if lang.Failed() {
		t.Fatal("a qualified import was reported")
	}
	for _, imp := range []string{"Util", "com.a.Missing", "com.x.sub.Inject"} {
		lang := NewLanguage().(*common.SingleLang)
		lang.Before(context.Background())
		r := rule.NewRule(LibraryKind, "consumer")
		lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{imp}), from)
		if !lang.Failed() || len(r.AttrStrings("deps")) != 0 {
			t.Errorf("%s: not reported (deps %v)", imp, r.AttrStrings("deps"))
		}
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
		result := generateRoot(t, tc.repoName, map[string]string{"Lib.java": "package lib;\n\npublic class Lib {}\n"})
		if len(result.Gen) != 1 || result.Gen[0].Kind() != LibraryKind || result.Gen[0].Name() != tc.want {
			t.Fatalf("repo %q: generated %+v, want one java_library(%s)", tc.repoName, result.Gen, tc.want)
		}
		if got := strings.Join(result.Gen[0].AttrStrings("srcs"), ","); got != "Lib.java" {
			t.Errorf("repo %q: srcs = %q, want Lib.java", tc.repoName, got)
		}
	}
}

func TestResolveQualifiedExternalImport(t *testing.T) {
	for _, tc := range []struct {
		name       string
		directives string
		want       []string
		failed     bool
	}{
		{"mapped", "# gazelle:resolve java javax.inject.Inject @maven//:javax_inject_javax_inject\n", []string{"@maven//:javax_inject_javax_inject"}, false},
		{"unmapped", "", nil, true},
	} {
		lang := NewLanguage().(*common.SingleLang)
		lang.Before(context.Background())
		cfg := config.New()
		resolver := &resolve.Configurer{}
		resolver.RegisterFlags(nil, "update", cfg)
		if err := resolver.CheckFlags(nil, cfg); err != nil {
			t.Fatal(err)
		}
		f, err := rule.LoadData("BUILD.bazel", "", []byte(tc.directives))
		if err != nil {
			t.Fatal(err)
		}
		resolver.Configure(cfg, "app", f)
		index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return nil })
		r := rule.NewRule(LibraryKind, "app")
		lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"javax.inject.Inject"}), label.New("", "app", "app"))
		if lang.Failed() != tc.failed {
			t.Errorf("%s: failed = %v, want %v", tc.name, lang.Failed(), tc.failed)
		}
		if got := r.AttrStrings("deps"); strings.Join(got, ",") != strings.Join(tc.want, ",") {
			t.Errorf("%s: deps = %v, want %v", tc.name, got, tc.want)
		}
	}
}
