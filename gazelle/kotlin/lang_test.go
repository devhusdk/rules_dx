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

func TestGenerateMainFails(t *testing.T) {
	regular := []string{"Demo.kt", "Main.kt"}
	got := refuseFixture(t, map[string]string{
		"pkg/demo/Demo.kt": "package demo\n\nclass Demo\n",
		"pkg/demo/Main.kt": "package demo\n\nobject Main {\n  fun main() {}\n}\n",
	}, regular)
	wantRefusal(t, got, "kotlin: pkg/demo: Main.kt defines main;")
}

func TestImportsIndexNonTestSources(t *testing.T) {
	lang := NewLanguage()
	lib := rule.NewRule(LibraryKind, "demo")
	lib.SetAttr("srcs", []string{"Demo.kt", "Helper.kt", "DemoTest.kt", "notes.txt"})
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

func TestGenerateBOMDoesNotMixPackages(t *testing.T) {
	result := generateFixture(t, map[string]string{
		"pkg/demo/A.kt": "\xEF\xBB\xBFpackage app\n\nclass A\n",
		"pkg/demo/B.kt": "package app\n\nclass B\n",
	}, []string{"A.kt", "B.kt"})
	if len(result.Gen) != 1 {
		t.Fatalf("generated %d rules, want 1", len(result.Gen))
	}
}

func resolveImports(t *testing.T, directives string, srcs map[string]string, imports ...string) (*rule.Rule, *common.SingleLang) {
	t.Helper()
	root := t.TempDir()
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
	resolver.Configure(cfg, "consumer", f)
	lang.Configure(cfg, "consumer", f)
	lang.GenerateRules(language.GenerateArgs{Config: cfg, Dir: filepath.Join(root, "consumer"), Rel: "consumer"})
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return lang })
	for file, content := range srcs {
		writeFixture(t, root, file, content)
		pkg, src := filepath.Dir(file), filepath.Base(file)
		r := rule.NewRule(LibraryKind, pkg)
		r.SetAttr("srcs", []string{src})
		bf, err := rule.LoadData(filepath.Join(root, pkg, "BUILD.bazel"), pkg, nil)
		if err != nil {
			t.Fatal(err)
		}
		index.AddRule(cfg, r, bf)
	}
	index.Finish()
	r := rule.NewRule(LibraryKind, "consumer")
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet(imports), label.New("", "consumer", "consumer"))
	return r, lang
}

func TestResolveExternalImportNeedsAMapping(t *testing.T) {
	r, lang := resolveImports(t, "# gazelle:resolve kotlin kotlinx.coroutines.runBlocking //third_party:ext\n", nil, "kotlinx.coroutines.runBlocking")
	if lang.Failed() {
		t.Fatal("a mapped external import was reported")
	}
	if got := r.AttrStrings("deps"); len(got) != 1 || got[0] != "//third_party:ext" {
		t.Fatalf("deps = %v", got)
	}
	r, lang = resolveImports(t, "", nil, "kotlinx.coroutines.runBlocking")
	if !lang.Failed() {
		t.Fatal("an unmapped external import was not reported")
	}
	if got := r.AttrStrings("deps"); len(got) != 0 {
		t.Fatalf("deps = %v", got)
	}
	lang.AfterResolvingDeps(context.Background())
	msg := strings.Join(common.Messages(common.Errors.Take()), "\n")
	if !strings.Contains(msg, "# gazelle:resolve mapping on the qualified import or its package") || !strings.Contains(msg, "kotlinx.coroutines.runBlocking") {
		t.Fatalf("errors = %q, want the qualified name and the unresolved hint", msg)
	}
}

func TestBareNameIgnoreNoLongerMatches(t *testing.T) {
	r, lang := resolveImports(t, "# gazelle:dx_ignore_import kotlin Widget\n", nil, "com.example.Widget")
	if !lang.Failed() {
		t.Fatal("a bare-name ignore still covered the qualified import")
	}
	if got := r.AttrStrings("deps"); len(got) != 0 {
		t.Fatalf("deps = %v", got)
	}
	lang.AfterResolvingDeps(context.Background())
	msg := strings.Join(common.Messages(common.Errors.Take()), "\n")
	if !strings.Contains(msg, `unresolved import "com.example.Widget"`) || !strings.Contains(msg, "stale # gazelle:dx_ignore_import kotlin Widget") {
		t.Fatalf("errors = %q, want the unresolved import and the stale ignore", msg)
	}
}

func TestResolveQualifiedImportFindsLocalClass(t *testing.T) {
	r, lang := resolveImports(t, "", map[string]string{"greeter/Greeter.kt": "package com.example\n\nclass Greeter\n"}, "com.example.Greeter")
	if lang.Failed() {
		t.Fatal("a local class import was reported")
	}
	if got := r.AttrStrings("deps"); len(got) != 1 || got[0] != "//greeter" {
		t.Fatalf("deps = %v", got)
	}
	r, lang = resolveImports(t, "", map[string]string{"other/Greeter.kt": "package com.other\n\nclass Greeter\n"}, "com.example.Greeter")
	if !lang.Failed() || len(r.AttrStrings("deps")) != 0 {
		t.Fatalf("a same-named class in another package bound: deps = %v", r.AttrStrings("deps"))
	}
}

func TestTopLevelFunctionResolvesToPackage(t *testing.T) {
	srcs := map[string]string{
		"util/Strings.kt":     "package com.example.util\n\nfun shout(s: String) = s\n",
		"grades/BuildInfo.kt": "package grades\n\nobject BuildInfo\nfun shout(s: String) = s\n",
	}
	for _, imp := range []string{"com.example.util.shout", "com.example.util.*", "com.example.util.Other"} {
		r, lang := resolveImports(t, "", srcs, imp)
		if lang.Failed() {
			t.Fatalf("%s was reported", imp)
		}
		if got := r.AttrStrings("deps"); len(got) != 1 || got[0] != "//util" {
			t.Fatalf("%s: deps = %v, want //util", imp, got)
		}
	}
	r, lang := resolveImports(t, "", srcs, "grades.BuildInfo")
	if lang.Failed() || strings.Join(r.AttrStrings("deps"), ",") != "//grades" {
		t.Fatalf("grades.BuildInfo: deps = %v", r.AttrStrings("deps"))
	}
	// A single-segment package is never a candidate, so its top-level
	// functions need the file's class or an explicit mapping.
	r, lang = resolveImports(t, "", srcs, "grades.shout")
	if !lang.Failed() || len(r.AttrStrings("deps")) != 0 {
		t.Fatalf("grades.shout: deps = %v", r.AttrStrings("deps"))
	}
}

func TestExternalPrefixDoesNotBindLocalParent(t *testing.T) {
	srcs := map[string]string{"acme/Core.kt": "package com.acme\n\nclass Core\n"}
	r, lang := resolveImports(t, "", srcs, "com.acme.lib.Client")
	if !lang.Failed() || len(r.AttrStrings("deps")) != 0 {
		t.Fatalf("an external import bound to the local parent package: deps = %v", r.AttrStrings("deps"))
	}
	r, lang = resolveImports(t, "# gazelle:resolve kotlin com.acme.lib //third_party:lib\n", srcs, "com.acme.lib.Client", "com.acme.Core")
	if lang.Failed() || strings.Join(r.AttrStrings("deps"), ",") != "//acme,//third_party:lib" {
		t.Fatalf("deps = %v", r.AttrStrings("deps"))
	}
}

func TestGenerateKeepsMappedStdLibLookalike(t *testing.T) {
	files := map[string]string{"pkg/demo/Demo.kt": "package demo\nimport kotlin.test.assertEquals\n"}
	root := t.TempDir()
	for name, content := range files {
		writeFixture(t, root, name, content)
	}
	cfg := config.New()
	cfg.RepoRoot = root
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	f, err := rule.LoadData("BUILD.bazel", "", []byte("# gazelle:resolve kotlin kotlin.test.assertEquals //third_party:test\n"))
	if err != nil {
		t.Fatal(err)
	}
	resolver.Configure(cfg, "pkg/demo", f)
	result := NewLanguage().GenerateRules(language.GenerateArgs{Config: cfg, Dir: filepath.Join(root, "pkg", "demo"), Rel: "pkg/demo", RegularFiles: []string{"Demo.kt"}})
	got, _ := common.UnwrapImportSet(result.Imports[0])
	if len(got) != 1 || got[0] != "kotlin.test.assertEquals" {
		t.Fatalf("imports = %v, want kotlin.test.assertEquals", got)
	}
}

func TestResolveMavenInstall(t *testing.T) {
	lock := `{"packages": {"com.google.guava:guava": ["com.google.common.collect"]}}`
	for _, tc := range []struct {
		name, file, directive string
	}{
		{"root lock", "maven_install.json", ""},
		{"directive", "third_party/jvm/lock.json", "# gazelle:dx_external maven third_party/jvm/lock.json hub\n"},
	} {
		root := t.TempDir()
		path := filepath.Join(root, filepath.FromSlash(tc.file))
		if err := os.MkdirAll(filepath.Dir(path), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(path, []byte(lock), 0o644); err != nil {
			t.Fatal(err)
		}
		lang := NewLanguage().(*common.SingleLang)
		lang.Before(context.Background())
		cfg := config.New()
		cfg.RepoRoot = root
		resolver := &resolve.Configurer{}
		resolver.RegisterFlags(nil, "update", cfg)
		if err := resolver.CheckFlags(nil, cfg); err != nil {
			t.Fatal(err)
		}
		f, err := rule.LoadData("BUILD.bazel", "", []byte(tc.directive))
		if err != nil {
			t.Fatal(err)
		}
		resolver.Configure(cfg, "", f)
		lang.Configure(cfg, "", f)
		want := "@maven//:com_google_guava_guava"
		if tc.directive != "" {
			want = "@hub//:com_google_guava_guava"
		}
		r := rule.NewRule(LibraryKind, "app")
		lang.Resolve(cfg, resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return nil }), nil, r,
			common.WrapImportSet([]string{"com.google.common.collect.ImmutableList"}), label.New("", "app", "app"))
		if lang.Failed() {
			t.Fatalf("%s: a lockfile import was reported", tc.name)
		}
		if got := strings.Join(r.AttrStrings("deps"), ","); got != want {
			t.Errorf("%s: deps = %q, want %q", tc.name, got, want)
		}
	}
}
