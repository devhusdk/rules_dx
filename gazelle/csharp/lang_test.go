package csharp

import (
	"context"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/merger"
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
	regular := []string{"Demo.cs", "Helper.cs", "notes.txt", "DemoTest.cs"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/Demo.cs":     "namespace Demo;\n\nusing Acme.Widget;\n\nclass Demo\n",
		"pkg/demo/Helper.cs":   "namespace Demo;\n\nusing System.Collections.Generic;\n\nclass Helper\n",
		"pkg/demo/notes.txt":   "not a source\n",
		"pkg/demo/DemoTest.cs": "namespace Demo;\n\nusing System;\n\nclass DemoTest\n",
	}, regular)
	if len(result.Gen) != 1 || len(result.Imports) != 1 {
		t.Fatalf("generated %d rules and %d import sets, want 1 each", len(result.Gen), len(result.Imports))
	}
	lib := result.Gen[0]
	if lib.Kind() != LibraryKind || lib.Name() != "demo" {
		t.Fatalf("library = %s(%s), want csharp_library(demo)", lib.Kind(), lib.Name())
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "Demo.cs,Helper.cs" {
		t.Errorf("library srcs = %q, want Demo.cs,Helper.cs", got)
	}
	got := result.Imports[0].(common.ImportSet)
	if strings.Join(got.Names, ",") != "Acme.Widget" {
		t.Errorf("library imports = %+v, want [Acme.Widget]", got.Names)
	}
}

func TestGenerateTestSourcesExcluded(t *testing.T) {
	regular := []string{"Demo.cs", "DemoTest.cs"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/Demo.cs":     "namespace Demo;\n\nclass Demo\n",
		"pkg/demo/DemoTest.cs": "namespace Demo;\n\nusing Acme.OnlyByTest;\n\nclass DemoTest\n",
	}, regular)
	if len(result.Gen) != 1 || len(result.Imports) != 1 {
		t.Fatalf("generated %d rules and %d import sets, want 1 each", len(result.Gen), len(result.Imports))
	}
	if got := strings.Join(result.Gen[0].AttrStrings("srcs"), ","); got != "Demo.cs" {
		t.Errorf("library srcs = %q, want Demo.cs", got)
	}
	if got := result.Imports[0].(common.ImportSet); len(got.Names) != 0 {
		t.Errorf("library imports = %+v, want empty", got.Names)
	}
}

func TestGenerateMixedPackagesFail(t *testing.T) {
	regular := []string{"A.cs", "B.cs"}
	got := refuseFixture(t, map[string]string{
		"pkg/demo/A.cs": "namespace Alpha;\n\nclass A\n",
		"pkg/demo/B.cs": "namespace Beta;\n\nclass B\n",
	}, regular)
	wantRefusal(t, got, "csharp: pkg/demo: mixed packages Alpha, Beta in one directory;")
}

func TestGenerateMainFails(t *testing.T) {
	regular := []string{"Demo.cs", "Main.cs"}
	got := refuseFixture(t, map[string]string{
		"pkg/demo/Demo.cs": "namespace Demo;\n\nclass Demo\n",
		"pkg/demo/Main.cs": "namespace Demo;\n\nclass Main {\n  static void Main(string[] args) {}\n}\n",
	}, regular)
	wantRefusal(t, got, "csharp: pkg/demo: Main.cs defines main;")
}

func TestGenerateTopLevelStatementsFail(t *testing.T) {
	regular := []string{"Demo.cs", "Program.cs"}
	got := refuseFixture(t, map[string]string{
		"pkg/demo/Demo.cs":    "namespace Demo;\n\nclass Demo\n",
		"pkg/demo/Program.cs": "using System;\n\nConsole.WriteLine(\"hi\");\n",
	}, regular)
	wantRefusal(t, got, "csharp: pkg/demo: Program.cs defines main;")
}

func TestImportsIndexNonTestSources(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/Demo.cs", "namespace Acme.Demo;\n\nclass Demo {}\n")
	writeFixture(t, root, "pkg/demo/Helper.cs", "namespace Acme.Demo;\n\nclass Helper {}\n")
	writeFixture(t, root, "pkg/demo/DemoTest.cs", "namespace Acme.Demo.Tests;\n\nclass DemoTest {}\n")
	writeFixture(t, root, "pkg/demo/notes.txt", "namespace Acme.Notes;\n")
	lang := NewLanguage()
	lib := rule.NewRule(LibraryKind, "demo")
	lib.SetAttr("srcs", []string{"Demo.cs", "Helper.cs", "DemoTest.cs", "notes.txt"})
	imports := lang.Imports(&config.Config{}, lib, &rule.File{Path: filepath.Join(root, "pkg", "demo", "BUILD.bazel"), Pkg: "pkg/demo"})
	var got []string
	for _, imp := range imports {
		if imp.Lang != languageName {
			t.Errorf("import %+v has lang %q", imp, imp.Lang)
		}
		got = append(got, imp.Imp)
	}
	sort.Strings(got)
	if want := "Acme.Demo,Acme.Demo.Demo,Acme.Demo.Helper"; strings.Join(got, ",") != want {
		t.Errorf("imports = %v, want %s and no bare file stems", got, want)
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
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"System.Collections.Generic"}), label.New("", "app", "app"))
	if lang.Failed() {
		t.Fatal("a stdlib import was reported")
	}
	lang.AfterResolvingDeps(context.Background())
}

func TestResolveNamespaceExactParentAndAmbiguous(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "lib/Greeter.cs", "namespace Acme.Lib;\n\nclass Greeter {}\n")
	writeFixture(t, root, "split/a/A.cs", "namespace Acme.Split;\n\nclass A {}\n")
	writeFixture(t, root, "split/b/B.cs", "namespace Acme.Split;\n\nclass B {}\n")
	setup := func() (*common.SingleLang, *config.Config, *resolve.RuleIndex) {
		lang := NewLanguage().(*common.SingleLang)
		lang.Before(context.Background())
		cfg := config.New()
		resolver := &resolve.Configurer{}
		resolver.RegisterFlags(nil, "update", cfg)
		if err := resolver.CheckFlags(nil, cfg); err != nil {
			t.Fatal(err)
		}
		resolver.Configure(cfg, "app", &rule.File{})
		index := resolve.NewRuleIndex(func(r *rule.Rule, _ string) resolve.Resolver {
			if r.Kind() == LibraryKind {
				return lang
			}
			return nil
		})
		for _, lib := range []struct{ pkg, src string }{{"lib", "Greeter.cs"}, {"split/a", "A.cs"}, {"split/b", "B.cs"}} {
			r := rule.NewRule(LibraryKind, filepath.Base(lib.pkg))
			r.SetAttr("srcs", []string{lib.src})
			index.AddRule(config.New(), r, rule.EmptyFile(filepath.Join(root, filepath.FromSlash(lib.pkg), "BUILD.bazel"), lib.pkg))
		}
		index.Finish()
		return lang, cfg, index
	}
	from := label.New("", "app", "app")
	for _, imp := range []string{"Acme.Lib", "Acme.Lib.Greeter", "Acme.Lib.Other"} {
		lang, cfg, index := setup()
		r := rule.NewRule(LibraryKind, "app")
		lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{imp}), from)
		if lang.Failed() {
			t.Fatalf("%s: unexpected failure", imp)
		}
		if got := strings.Join(r.AttrStrings("deps"), ","); got != "//lib" {
			t.Errorf("%s: deps = %q, want //lib", imp, got)
		}
	}
	lang, cfg, index := setup()
	lang.Resolve(cfg, index, nil, rule.NewRule(LibraryKind, "app"), common.WrapImportSet([]string{"Acme.Split"}), from)
	got := refusal(t, lang)
	wantRefusal(t, got, `ambiguous import "Acme.Split"`)
	wantRefusal(t, got, "//split/a")
	wantRefusal(t, got, "//split/b")

	lang, cfg, index = setup()
	r := rule.NewRule(LibraryKind, "app")
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"Greeter"}), from)
	if r.Attr("deps") != nil {
		t.Errorf("bare type name bound deps = %v", r.AttrStrings("deps"))
	}
	wantRefusal(t, refusal(t, lang), `unresolved import "Greeter"; `+unresolvedHint)
}

func TestAspectHintsSurviveRegen(t *testing.T) {
	root := t.TempDir()
	for name, content := range map[string]string{
		"pkg/demo/Demo.cs":   "namespace Demo;\n\nclass Demo\n",
		"pkg/demo/Helper.cs": "namespace Demo;\n\nclass Helper\n",
	} {
		writeFixture(t, root, name, content)
	}
	build := `load("@rules_dx//csharp/rules:defs.bzl", "csharp_library")

csharp_library(
    name = "demo",
    srcs = [
        "Demo.cs",
        "Helper.cs",
    ],
    aspect_hints = [":my_config"],
)
`
	file, err := rule.LoadData("BUILD.bazel", "pkg/demo", []byte(build))
	if err != nil {
		t.Fatal(err)
	}
	result := NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"Demo.cs", "Helper.cs"},
		File:         file,
	})
	if len(result.Gen) != 1 {
		t.Fatalf("generated %d rules, want 1", len(result.Gen))
	}
	merger.MergeFile(file, result.Empty, result.Gen, merger.PreResolve, NewLanguage().Kinds(), nil)
	var merged *rule.Rule
	for _, r := range file.Rules {
		if r.Kind() == LibraryKind && r.Name() == "demo" {
			merged = r
		}
	}
	if merged == nil {
		t.Fatal("merged demo rule missing")
	}
	if got := strings.Join(merged.AttrStrings("aspect_hints"), ","); got != ":my_config" {
		t.Errorf("merged aspect_hints = %q, want :my_config to survive regen", got)
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
		result := generateRoot(t, tc.repoName, map[string]string{"Lib.cs": "namespace Lib;\n\nclass Lib\n"})
		if len(result.Gen) != 1 || result.Gen[0].Kind() != LibraryKind || result.Gen[0].Name() != tc.want {
			t.Fatalf("repo %q: generated %+v, want one csharp_library(%s)", tc.repoName, result.Gen, tc.want)
		}
		if got := strings.Join(result.Gen[0].AttrStrings("srcs"), ","); got != "Lib.cs" {
			t.Errorf("repo %q: srcs = %q, want Lib.cs", tc.repoName, got)
		}
	}
}

func TestResolveNuGetNamespaceUsesMappingAndFailsWhenUnmapped(t *testing.T) {
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
	parsed := ParseImports([]byte("using System.CommandLine;\n"))
	if len(parsed) != 1 {
		t.Fatalf("ParseImports = %v, want one import", parsed)
	}
	imports := common.WrapImportSet(parsed)
	from := label.New("", "app", "app")

	lang, cfg, index := setup("# gazelle:resolve csharp " + parsed[0] + " @nuget//system.commandline\n")
	r := rule.NewRule(LibraryKind, "app")
	lang.Resolve(cfg, index, nil, r, imports, from)
	if lang.Failed() {
		t.Fatal("a mapped NuGet import was reported")
	}
	if got := r.AttrStrings("deps"); len(got) != 1 || got[0] != "@nuget//system.commandline" {
		t.Fatalf("deps = %v", got)
	}

	lang, cfg, index = setup("")
	r = rule.NewRule(LibraryKind, "app")
	lang.Resolve(cfg, index, nil, r, imports, from)
	if !lang.Failed() {
		t.Fatal("an unresolved NuGet import was not reported")
	}
}

func TestGenerateBOMKeepsUsing(t *testing.T) {
	result := generateFixture(t, map[string]string{
		"pkg/demo/Demo.cs": "\xEF\xBB\xBFusing Acme.Shared;\n\nnamespace Demo;\n\nclass Demo\n",
	}, []string{"Demo.cs"})
	if len(result.Imports) != 1 {
		t.Fatalf("generated %d import sets, want 1", len(result.Imports))
	}
	got := result.Imports[0].(common.ImportSet)
	if strings.Join(got.Names, ",") != "Acme.Shared" {
		t.Errorf("imports = %+v, want [Acme.Shared]", got.Names)
	}
}
