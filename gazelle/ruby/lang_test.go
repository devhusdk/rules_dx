package ruby

import (
	"context"
	"os"
	"path"
	"path/filepath"
	"reflect"
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
	regular := []string{"demo.rb", "helper.rb", "notes.txt", "demo_spec.rb"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.rb":      "require \"acme/widget\"\n\nmodule Demo\n",
		"pkg/demo/helper.rb":    "require \"json\"\n\nmodule Helper\n",
		"pkg/demo/notes.txt":    "not a source\n",
		"pkg/demo/demo_spec.rb": "require \"json\"\n\nRSpec.describe Demo\n",
	}, regular)
	if len(result.Gen) != 1 || len(result.Imports) != 1 {
		t.Fatalf("generated %d rules and %d import sets, want 1 each", len(result.Gen), len(result.Imports))
	}
	lib := result.Gen[0]
	if lib.Kind() != LibraryKind || lib.Name() != "demo" {
		t.Fatalf("library = %s(%s), want ruby_library(demo)", lib.Kind(), lib.Name())
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "demo.rb,helper.rb" {
		t.Errorf("library srcs = %q, want demo.rb,helper.rb", got)
	}
	got := result.Imports[0].(common.ImportSet)
	if strings.Join(got.Names, ",") != "acme/widget" {
		t.Errorf("library imports = %+v, want [acme/widget]", got.Names)
	}
}

func TestGenerateTestSourcesExcluded(t *testing.T) {
	regular := []string{"demo.rb", "demo_spec.rb"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.rb":      "module Demo\n",
		"pkg/demo/demo_spec.rb": "require \"acme/only_by_test\"\n\nRSpec.describe Demo\n",
	}, regular)
	if len(result.Gen) != 1 || len(result.Imports) != 1 {
		t.Fatalf("generated %d rules and %d import sets, want 1 each", len(result.Gen), len(result.Imports))
	}
	if got := strings.Join(result.Gen[0].AttrStrings("srcs"), ","); got != "demo.rb" {
		t.Errorf("library srcs = %q, want demo.rb", got)
	}
	if got := result.Imports[0].(common.ImportSet); len(got.Names) != 0 {
		t.Errorf("library imports = %+v, want empty", got.Names)
	}
}

func TestGenerateMixedPackagesSucceed(t *testing.T) {
	regular := []string{"a.rb", "b.rb"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/a.rb": "require \"alpha\"\n\nmodule A\n",
		"pkg/demo/b.rb": "require \"beta\"\n\nmodule B\n",
	}, regular)
	if len(result.Gen) != 1 {
		t.Fatalf("generated %d rules, want 1 (no mixed-package failure for Ruby)", len(result.Gen))
	}
}

func TestGenerateMainFails(t *testing.T) {
	regular := []string{"demo.rb", "main.rb"}
	got := refuseFixture(t, map[string]string{
		"pkg/demo/demo.rb": "module Demo\n",
		"pkg/demo/main.rb": "puts Hello.hello(\"world\") if __FILE__ == $0\n",
	}, regular)
	wantRefusal(t, got, "ruby: pkg/demo: main.rb defines main;")
}

func TestImportsIndexNonTestSources(t *testing.T) {
	lang := NewLanguage()
	lib := rule.NewRule(LibraryKind, "demo")
	lib.SetAttr("srcs", []string{"demo.rb", "helper.rb", "demo_spec.rb", "notes.txt"})
	imports := lang.Imports(&config.Config{}, lib, nil)
	if got := impNames(imports); !reflect.DeepEqual(got, []string{"demo", "//demo", "helper", "//helper"}) {
		t.Errorf("imports = %v, want demo+helper identities", got)
	}
	nested := lang.Imports(&config.Config{}, lib, &rule.File{Pkg: "lib/acme"})
	if got := impNames(nested); !reflect.DeepEqual(got, []string{"acme/demo", "//lib/acme/demo", "acme/helper", "//lib/acme/helper"}) {
		t.Errorf("lib imports = %v, want acme identities", got)
	}
	if got := lang.Imports(&config.Config{}, rule.NewRule("filegroup", "demo"), nil); got != nil {
		t.Errorf("other-kind imports = %+v, want nil", got)
	}
	if got := lang.Imports(&config.Config{}, rule.NewRule(LibraryKind, "empty"), nil); got != nil {
		t.Errorf("srcless imports = %+v, want nil", got)
	}
}

func TestResolveSkipsStdlibImports(t *testing.T) {
	lang := NewLanguage().(*rubyLang)
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
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"json"}), label.New("", "app", "app"))
	if lang.Failed() {
		t.Fatal("a stdlib import was reported")
	}
	lang.AfterResolvingDeps(context.Background())
}

func TestAspectHintsSurviveRegen(t *testing.T) {
	root := t.TempDir()
	for name, content := range map[string]string{
		"pkg/demo/demo.rb":   "module Demo\n",
		"pkg/demo/helper.rb": "module Helper\n",
	} {
		writeFixture(t, root, name, content)
	}
	build := `load("@rules_dx//ruby/rules:defs.bzl", "ruby_library")

ruby_library(
    name = "demo",
    srcs = [
        "demo.rb",
        "helper.rb",
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
		RegularFiles: []string{"demo.rb", "helper.rb"},
		File:         file,
	})
	if len(result.Gen) != 1 {
		t.Fatalf("generated %d rules, want 1", len(result.Gen))
	}
	merger.MergeFile(file, result.Empty, result.Gen, merger.PreResolve, rubyKinds, nil)
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
		result := generateRoot(t, tc.repoName, map[string]string{"lib.rb": "module Lib\nend\n"})
		if len(result.Gen) != 1 || result.Gen[0].Kind() != LibraryKind || result.Gen[0].Name() != tc.want {
			t.Fatalf("repo %q: generated %+v, want one ruby_library(%s)", tc.repoName, result.Gen, tc.want)
		}
		if got := strings.Join(result.Gen[0].AttrStrings("srcs"), ","); got != "lib.rb" {
			t.Errorf("repo %q: srcs = %q, want lib.rb", tc.repoName, got)
		}
	}
}

func TestResolveLoadPath(t *testing.T) {
	lang := NewLanguage().(*rubyLang)
	lang.Before(context.Background())
	cfg := config.New()
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	resolver.Configure(cfg, "app", &rule.File{})
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return lang })
	for _, pkg := range []string{"lib/acme", "tools/lib"} {
		lib := rule.NewRule(LibraryKind, path.Base(pkg))
		lib.SetAttr("srcs", []string{"widget.rb"})
		index.AddRule(cfg, lib, &rule.File{Pkg: pkg})
	}
	index.Finish()
	r := rule.NewRule(LibraryKind, "app")
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"acme/widget", "widget"}), label.New("", "app", "app"))
	if lang.Failed() {
		t.Fatal("a load path was reported unresolved or ambiguous")
	}
	got := r.AttrStrings("deps")
	if strings.Join(got, ",") != "//lib/acme,//tools/lib" {
		t.Fatalf("deps = %v, want //lib/acme and //tools/lib", got)
	}
}

func impNames(specs []resolve.ImportSpec) []string {
	var out []string
	for _, s := range specs {
		out = append(out, s.Imp)
	}
	return out
}

func TestImportsIndexPathIdentity(t *testing.T) {
	lib := rule.NewRule(LibraryKind, "lib")
	lib.SetAttr("srcs", []string{"foo.rb", "sub/bar.rb", "foo_test.rb"})
	got := impNames(NewLanguage().Imports(&config.Config{}, lib, &rule.File{Pkg: "pkg/lib"}))
	want := []string{"foo", "//pkg/lib/foo", "sub/bar", "//pkg/lib/sub/bar"}
	if !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}
