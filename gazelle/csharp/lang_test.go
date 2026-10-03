package csharp

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/ralvik/rules_dx/gazelle/common"

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
	if strings.Join(got.Names, ",") != "Widget" {
		t.Errorf("library imports = %+v, want [Widget]", got.Names)
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
	result := generateFixture(t, map[string]string{
		"pkg/demo/A.cs": "namespace Alpha;\n\nclass A\n",
		"pkg/demo/B.cs": "namespace Beta;\n\nclass B\n",
	}, regular)
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules for mixed packages, want 0 with a recorded failure", len(result.Gen))
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
	regular := []string{"Demo.cs", "Main.cs"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/Demo.cs": "namespace Demo;\n\nclass Demo\n",
		"pkg/demo/Main.cs": "namespace Demo;\n\nclass Main {\n  static void Main(string[] args) {}\n}\n",
	}, regular)
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules for a main mix, want 0 with a recorded failure", len(result.Gen))
	}
}

func TestGenerateKindMismatchFails(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, "pkg/demo/Demo.cs", "namespace Demo;\n\nclass Demo\n")
	existing := &rule.File{}
	fg := rule.NewRule("filegroup", "demo")
	existing.Rules = append(existing.Rules, fg)
	result := NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"Demo.cs"},
		File:         existing,
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules over a kind mismatch, want 0 with a recorded failure", len(result.Gen))
	}
}

func TestImportsIndexNonTestSources(t *testing.T) {
	lang := NewLanguage()
	lib := rule.NewRule(LibraryKind, "demo")
	lib.SetAttr("srcs", []string{"Demo.cs", "Helper.cs", "DemoTest.cs", "notes.txt"})
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
	for _, value := range []string{"csharp Dropped", "csharp csharp Second", "other Foreign", ""} {
		lang.Configure(cfg, "app", &rule.File{Directives: []rule.Directive{{Key: "dx_ignore_import", Value: value}}})
	}
	if lang.Failed() {
		t.Fatal("a foreign or empty ignore was rejected")
	}
	conf := cfg.Exts[languageName].(*common.IgnoreConfig)
	if len(conf.Ignores) != 2 || conf.Ignores[0].Value != "Dropped" || conf.Ignores[1].Value != "Second" {
		t.Fatalf("ignores = %+v, want Dropped and Second", conf.Ignores)
	}
	lang.Configure(cfg, "app", &rule.File{Directives: []rule.Directive{{Key: "dx_ignore_import", Value: "csharp"}}})
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
	lang.Fail("csharp: boom")
	defer func() {
		r := recover()
		if r == nil {
			t.Fatal("AfterResolvingDeps did not panic with recorded errors")
		}
		if msg, ok := r.(string); !ok || !strings.Contains(msg, "CSharp generation failed") {
			t.Fatalf("panic = %v, want CSharp generation failure", r)
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
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"System.Collections.Generic"}), label.New("", "app", "app"))
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
	if len(loads) != 1 || loads[0].Name != "@renamed_dx//csharp/rules:defs.bzl" || strings.Join(loads[0].Symbols, ",") != LibraryKind {
		t.Errorf("apparent loads = %+v", loads)
	}
	if defaults := lang.Loads(); len(defaults) != 1 || defaults[0].Name != "@rules_dx//csharp/rules:defs.bzl" {
		t.Errorf("default loads = %+v", defaults)
	}
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
	merger.MergeFile(file, result.Empty, result.Gen, merger.PreResolve, csharpKinds, nil)
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
