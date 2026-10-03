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
	"github.com/ralvik/rules_dx/gazelle/common"
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

func TestConfigureRoutesIgnoreDirectivesToTheSharedConfig(t *testing.T) {
	cfg := config.New()
	lang := &goLang{}
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
	if len(lang.errors) != 1 {
		t.Fatalf("malformed directive accepted: %v", lang.errors)
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
	got := result.Imports[0].(targetImports)
	if strings.Join(got.imports, ",") != "thing" {
		t.Errorf("library imports = %+v, want [thing]", got.imports)
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
	if got := result.Imports[1].(testTargetImports); len(got.imports) != 0 {
		t.Errorf("test imports = %+v, want empty (stdlib-only)", got.imports)
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
	if got := result.Imports[0].(targetImports); len(got.imports) != 0 {
		t.Errorf("library imports = %+v, want empty (test-only import stays off the library)", got.imports)
	}
	if got := strings.Join(result.Gen[1].AttrStrings("srcs"), ","); got != "demo_test.go" {
		t.Errorf("test srcs = %q, want demo_test.go", got)
	}
	if got := result.Imports[1].(testTargetImports); strings.Join(got.imports, ",") != "only_by_test" {
		t.Errorf("test imports = %+v, want [only_by_test]", got.imports)
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
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.go": "package demo\n",
		"pkg/demo/main.go": "package main\n\nfunc main() {}\n",
	}, regular)
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules for a package-main mix, want 0 with a recorded failure", len(result.Gen))
	}
}

func TestGenerateMixedPackagesFail(t *testing.T) {
	regular := []string{"a.go", "b.go"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/a.go": "package alpha\n",
		"pkg/demo/b.go": "package beta\n",
	}, regular)
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules for mixed packages, want 0 with a recorded failure", len(result.Gen))
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
	result := NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"demo.go"},
		File:         existing,
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules over a kind mismatch, want 0 with a recorded failure", len(result.Gen))
	}
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
	if got := result.Imports[0].(targetImports); strings.Join(got.imports, ",") != "shared" {
		t.Errorf("library imports = %+v, want [shared]", got.imports)
	}
	if got := result.Imports[1].(testTargetImports); strings.Join(got.imports, ",") != "only_by_test" {
		t.Errorf("test imports = %+v, want [only_by_test]", got.imports)
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
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo_test.go": "package demo_test\n\nimport \"testing\"\n",
	}, []string{"demo_test.go"})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules for a test-only dir, want 0 with a recorded failure", len(result.Gen))
	}
}

func TestGenerateMismatchedTestPackageFails(t *testing.T) {
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.go":      "package demo\n",
		"pkg/demo/demo_test.go": "package other\n",
	}, []string{"demo.go", "demo_test.go"})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules for a mismatched test package, want 0 with a recorded failure", len(result.Gen))
	}
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
	result := NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"demo.go", "demo_test.go"},
		File:         existing,
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules over a test kind mismatch, want 0 with a recorded failure", len(result.Gen))
	}
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

func TestParseGoModule(t *testing.T) {
	if got, ok := parseGoModule("module example.com/adopt-go\n\ngo 1.26\n"); !ok || got != "example.com/adopt-go" {
		t.Errorf("parse = %q,%v, want example.com/adopt-go,true", got, ok)
	}
	if got, ok := parseGoModule("// leading comment\nmodule example.com/x\n"); !ok || got != "example.com/x" {
		t.Errorf("parse with comment = %q,%v, want example.com/x,true", got, ok)
	}
	if _, ok := parseGoModule("package foo\n"); ok {
		t.Error("parse without module line = ok, want false")
	}
	if _, ok := parseGoModule(""); ok {
		t.Error("parse empty = ok, want false")
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

func TestImportsIndexNonTestSources(t *testing.T) {
	lang := NewLanguage()
	lib := rule.NewRule(LibraryKind, "demo")
	lib.SetAttr("srcs", []string{"demo.go", "helper.go", "demo_test.go", "notes.txt"})
	imports := lang.Imports(&config.Config{}, lib, nil)
	if len(imports) != 2 || imports[0].Lang != languageName || imports[0].Imp != "demo" || imports[1].Imp != "helper" {
		t.Errorf("imports = %+v, want demo+helper stems", imports)
	}
	if got := lang.Imports(&config.Config{}, rule.NewRule("filegroup", "demo"), nil); got != nil {
		t.Errorf("other-kind imports = %+v, want nil", got)
	}
	if got := lang.Imports(&config.Config{}, rule.NewRule(LibraryKind, "empty"), nil); got != nil {
		t.Errorf("srcless imports = %+v, want nil", got)
	}
	if got := lang.Imports(&config.Config{}, rule.NewRule(TestKind, "demo_test"), nil); got != nil {
		t.Errorf("test imports = %+v, want nil (tests are never dependencies)", got)
	}
}

func TestAfterResolvingDepsReportsFailures(t *testing.T) {
	l := &goLang{}
	l.Before(context.Background())
	l.fail("go: boom")
	defer func() {
		r := recover()
		if r == nil {
			t.Fatal("AfterResolvingDeps did not panic with recorded errors")
		}
		if msg, ok := r.(string); !ok || !strings.Contains(msg, "Go generation failed") {
			t.Fatalf("panic = %v, want Go generation failure", r)
		}
	}()
	l.AfterResolvingDeps(context.Background())
}

func TestAfterResolvingDepsClean(t *testing.T) {
	l := &goLang{}
	l.Before(context.Background())
	l.AfterResolvingDeps(context.Background())
	if len(l.errors) != 0 {
		t.Errorf("errors = %v", l.errors)
	}
}

func TestLanguageMetadata(t *testing.T) {
	l := &goLang{}
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

func resolverIndex(lang *goLang, entries ...struct {
	pkg  string
	name string
}) *resolve.RuleIndex {
	index := resolve.NewRuleIndex(func(r *rule.Rule, _ string) resolve.Resolver {
		if _, ok := goKinds[r.Kind()]; ok {
			return lang
		}
		return nil
	})
	for _, entry := range entries {
		r := rule.NewRule(LibraryKind, entry.name)
		r.SetAttr("srcs", []string{entry.name + ".go"})
		index.AddRule(config.New(), r, rule.EmptyFile(filepath.Join(entry.pkg, "BUILD.bazel"), entry.pkg))
	}
	index.Finish()
	return index
}

func TestResolveBranches(t *testing.T) {
	l := &goLang{}
	index := resolverIndex(l,
		struct{ pkg, name string }{"lib/b", "b"},
		struct{ pkg, name string }{"lib/a", "a"},
	)
	cfg := resolverConfig(t, nil)
	r := rule.NewRule(LibraryKind, "app")
	l.Resolve(cfg, index, nil, r, targetImports{imports: []string{"b", "a"}}, label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//lib/a,//lib/b" {
		t.Errorf("resolved deps = %q", got)
	}

	self := rule.NewRule(LibraryKind, "a")
	l.Resolve(cfg, index, nil, self, targetImports{imports: []string{"a"}}, label.New("", "lib/a", "a"))
	if self.Attr("deps") != nil {
		t.Errorf("self dependency was emitted: %v", self.AttrStrings("deps"))
	}

	otherKind := rule.NewRule("filegroup", "other")
	l.Resolve(cfg, index, nil, otherKind, targetImports{imports: []string{"a"}}, label.New("", "app", "other"))
	if otherKind.Attr("deps") != nil {
		t.Errorf("non-library resolution emitted deps = %v", otherKind.AttrStrings("deps"))
	}

	std := rule.NewRule(LibraryKind, "uses_std")
	l.Resolve(cfg, index, nil, std, targetImports{imports: []string{"fmt"}}, label.New("", "app", "uses_std"))
	if std.Attr("deps") != nil || len(l.errors) != 0 {
		t.Errorf("stdlib resolution = %v, errors = %v", std.AttrStrings("deps"), l.errors)
	}

	hand := rule.NewRule(LibraryKind, "hand")
	hand.SetAttr("deps", []string{":kept"})
	l.Resolve(cfg, index, nil, hand, targetImports{imports: []string{"a"}}, label.New("", "app", "hand"))
	if got := strings.Join(hand.AttrStrings("deps"), ","); got != "//lib/a,:kept" {
		t.Errorf("merged deps = %q", got)
	}

	l.Resolve(cfg, index, nil, rule.NewRule(LibraryKind, "unknown"), targetImports{imports: []string{"unknown"}}, label.New("", "app", "unknown"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "unresolved import") {
		t.Errorf("unresolved errors = %v", l.errors)
	}
	l.Resolve(cfg, index, nil, r, nil, label.New("", "app", "app"))
}

func TestResolveOverride(t *testing.T) {
	l := &goLang{}
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "go go mapped //mapped:dep"}})
	r := rule.NewRule(LibraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, targetImports{imports: []string{"mapped"}}, label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//mapped:dep" {
		t.Errorf("override deps = %q", got)
	}
}

func TestResolveTestBranches(t *testing.T) {
	l := &goLang{}
	index := resolverIndex(l,
		struct{ pkg, name string }{"lib/sib", "sib"},
		struct{ pkg, name string }{"pkg/demo", "demo"},
	)
	cfg := resolverConfig(t, nil)

	sib := rule.NewRule(TestKind, "demo_test")
	l.Resolve(cfg, index, nil, sib, testTargetImports{imports: []string{"sib"}}, label.New("", "pkg/demo", "demo_test"))
	if got := strings.Join(sib.AttrStrings("deps"), ","); got != "//lib/sib" {
		t.Errorf("test sibling deps = %q, want //lib/sib", got)
	}

	self := rule.NewRule(TestKind, "demo_test")
	l.Resolve(cfg, index, nil, self, testTargetImports{imports: []string{"demo"}}, label.New("", "pkg/demo", "demo_test"))
	if self.Attr("deps") != nil {
		t.Errorf("same-package test dep was emitted: %v", self.AttrStrings("deps"))
	}

	cross := rule.NewRule(TestKind, "demo_test")
	l.Resolve(cfg, index, nil, cross, targetImports{imports: []string{"sib"}}, label.New("", "pkg/demo", "demo_test"))
	if cross.Attr("deps") != nil {
		t.Errorf("wrong import type resolved onto test: %v", cross.AttrStrings("deps"))
	}
	libCross := rule.NewRule(LibraryKind, "demo")
	l.Resolve(cfg, index, nil, libCross, testTargetImports{imports: []string{"sib"}}, label.New("", "pkg/demo", "demo"))
	if libCross.Attr("deps") != nil {
		t.Errorf("test import type resolved onto library: %v", libCross.AttrStrings("deps"))
	}

	std := rule.NewRule(TestKind, "uses_std_test")
	l.Resolve(cfg, index, nil, std, testTargetImports{imports: []string{"fmt"}}, label.New("", "pkg/demo", "uses_std_test"))
	if std.Attr("deps") != nil || len(l.errors) != 0 {
		t.Errorf("stdlib test resolution = %v, errors = %v", std.AttrStrings("deps"), l.errors)
	}
	l.Resolve(cfg, index, nil, rule.NewRule(TestKind, "unknown_test"), testTargetImports{imports: []string{"unknown"}}, label.New("", "pkg/demo", "unknown_test"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "unresolved import") {
		t.Errorf("unresolved test errors = %v", l.errors)
	}
}

func TestResolveIgnore(t *testing.T) {
	l := &goLang{}
	l.Before(context.Background())
	cfg := resolverConfig(t, nil)
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import go dropped\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "app", file)
	r := rule.NewRule(LibraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, targetImports{imports: []string{"dropped"}}, label.New("", "app", "app"))
	if r.Attr("deps") != nil {
		t.Errorf("ignored import emitted deps = %v", r.AttrStrings("deps"))
	}
	l.AfterResolvingDeps(context.Background())
	if len(l.errors) != 0 {
		t.Errorf("ignore errors = %v", l.errors)
	}
}

func TestResolveStaleIgnoreFails(t *testing.T) {
	l := &goLang{}
	l.Before(context.Background())
	cfg := resolverConfig(t, nil)
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import go unused\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "app", file)
	defer func() {
		r := recover()
		if r == nil {
			t.Fatal("stale ignore did not panic")
		}
		if msg, ok := r.(string); !ok || !strings.Contains(msg, "stale # gazelle:dx_ignore_import") {
			t.Fatalf("panic = %v, want stale ignore failure", r)
		}
	}()
	l.AfterResolvingDeps(context.Background())
}

func TestResolveAmbiguousFails(t *testing.T) {
	l := &goLang{}
	index := resolverIndex(l,
		struct{ pkg, name string }{"lib/one", "dup"},
		struct{ pkg, name string }{"lib/two", "dup"},
	)
	cfg := resolverConfig(t, nil)
	l.Resolve(cfg, index, nil, rule.NewRule(LibraryKind, "app"), targetImports{imports: []string{"dup"}}, label.New("", "app", "app"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "ambiguous import") {
		t.Errorf("ambiguous errors = %v", l.errors)
	}
}

func TestGenerateCgoStaysHandwritten(t *testing.T) {
	regular := []string{"cgo.go", "helper.go"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/cgo.go":    "package demo\n\nimport \"C\"\n",
		"pkg/demo/helper.go": "package demo\n",
	}, regular)
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules for cgo sources, want 0 with a recorded failure", len(result.Gen))
	}
}

func TestGenerateCgoTestStaysHandwritten(t *testing.T) {
	regular := []string{"demo.go", "cgo_test.go"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/demo.go":     "package demo\n",
		"pkg/demo/cgo_test.go": "package demo_test\n\nimport \"C\"\n",
	}, regular)
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules for cgo test sources, want 0 with a recorded failure", len(result.Gen))
	}
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
