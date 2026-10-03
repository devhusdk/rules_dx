package fsharp

import (
	"context"
	"flag"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/merger"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
	bzl "github.com/bazelbuild/buildtools/build"
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
	lang := &fsharpLang{}
	lang.Configure(cfg, "parent", &rule.File{Directives: []rule.Directive{
		{Key: "other"},
		{Key: "dx_ignore_import", Value: "fsharp fsharp Widget"},
		{Key: "dx_ignore_import", Value: "fsharp"},
	}})
	if _, ok := cfg.Exts[languageName].(*common.IgnoreConfig); !ok {
		t.Fatalf("ext = %T, want *common.IgnoreConfig", cfg.Exts[languageName])
	}
	entry := common.MatchingIgnore(cfg, languageName, "Widget")
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
	regular := []string{"Demo.fs", "Helper.fs", "notes.txt", "DemoTest.fs"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/Demo.fs":     "namespace Demo\n\nopen Acme.Widget\n\ntype Demo = class end\n",
		"pkg/demo/Helper.fs":   "namespace Demo\n\nopen System.Collections.Generic\n\ntype Helper = class end\n",
		"pkg/demo/notes.txt":   "not a source\n",
		"pkg/demo/DemoTest.fs": "namespace Demo\n\nopen System\n\ntype DemoTest = class end\n",
	}, regular)
	if len(result.Gen) != 1 || len(result.Imports) != 1 {
		t.Fatalf("generated %d rules and %d import sets, want 1 each", len(result.Gen), len(result.Imports))
	}
	lib := result.Gen[0]
	if lib.Kind() != LibraryKind || lib.Name() != "demo" {
		t.Fatalf("library = %s(%s), want fsharp_library(demo)", lib.Kind(), lib.Name())
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "Demo.fs,Helper.fs" {
		t.Errorf("library srcs = %q, want Demo.fs,Helper.fs", got)
	}
	got := result.Imports[0].(targetImports)
	if strings.Join(got.imports, ",") != "Widget" {
		t.Errorf("library imports = %+v, want [Widget]", got.imports)
	}
}

func TestGenerateDependencyOrder(t *testing.T) {
	regular := []string{"Demo.fs", "Helper.fs"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/Demo.fs":   "namespace Demo\n\nopen Helper\n\ntype Demo = class end\n",
		"pkg/demo/Helper.fs": "namespace Demo\n\nopen System.Collections.Generic\n\ntype Helper = class end\n",
	}, regular)
	if len(result.Gen) != 1 {
		t.Fatalf("generated %d rules, want 1", len(result.Gen))
	}
	if got := strings.Join(result.Gen[0].AttrStrings("srcs"), ","); got != "Helper.fs,Demo.fs" {
		t.Errorf("library srcs = %q, want Helper.fs,Demo.fs (dependency first)", got)
	}
}

func TestGenerateChainOrder(t *testing.T) {
	regular := []string{"A.fs", "B.fs", "C.fs"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/A.fs": "namespace Demo\n\nopen B\n\ntype A = class end\n",
		"pkg/demo/B.fs": "namespace Demo\n\nopen C\n\ntype B = class end\n",
		"pkg/demo/C.fs": "namespace Demo\n\ntype C = class end\n",
	}, regular)
	if len(result.Gen) != 1 {
		t.Fatalf("generated %d rules, want 1", len(result.Gen))
	}
	if got := strings.Join(result.Gen[0].AttrStrings("srcs"), ","); got != "C.fs,B.fs,A.fs" {
		t.Errorf("library srcs = %q, want C.fs,B.fs,A.fs (chain dependency order)", got)
	}
}

func TestGenerateCycleFails(t *testing.T) {
	regular := []string{"A.fs", "B.fs"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/A.fs": "namespace Demo\n\nopen B\n\ntype A = class end\n",
		"pkg/demo/B.fs": "namespace Demo\n\nopen A\n\ntype B = class end\n",
	}, regular)
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules for a cycle, want 0 with a recorded failure", len(result.Gen))
	}
}

func TestOrderSourcesByDependencyUnit(t *testing.T) {
	ordered, err := orderSourcesByDependency(
		[]string{"Demo.fs", "Helper.fs"},
		map[string][]byte{
			"Demo.fs":   []byte("namespace Demo\n\nopen Helper\n"),
			"Helper.fs": []byte("namespace Demo\n"),
		},
	)
	if err != nil || strings.Join(ordered, ",") != "Helper.fs,Demo.fs" {
		t.Errorf("orderSourcesByDependency = %v, %v; want Helper.fs,Demo.fs", ordered, err)
	}
	if _, err := orderSourcesByDependency(
		[]string{"A.fs", "B.fs"},
		map[string][]byte{
			"A.fs": []byte("namespace Demo\n\nopen B\n"),
			"B.fs": []byte("namespace Demo\n\nopen A\n"),
		},
	); err == nil {
		t.Error("orderSourcesByDependency succeeded on a cycle, want failure")
	}
}

func TestDoNotSortPreservesCompileOrder(t *testing.T) {
	f := rule.EmptyFile("BUILD.bazel", "pkg/demo")
	r := rule.NewRule(LibraryKind, "demo")
	r.SetSortedAttrs([]string{"deps"})
	r.SetAttr("srcs", rule.UnsortedStrings([]string{"C.fs", "B.fs", "A.fs"}))
	if comments := r.AttrComments("srcs"); comments != nil {
		comments.Before = append(comments.Before, bzl.Comment{Token: "# do not sort: F# compile order, dependencies first"})
	}
	r.Insert(f)
	out := string(f.Format())
	if !strings.Contains(out, "do not sort") {
		t.Errorf("formatted output missing do-not-sort comment:\n%s", out)
	}
	cIdx := strings.Index(out, "\"C.fs\"")
	bIdx := strings.Index(out, "\"B.fs\"")
	aIdx := strings.Index(out, "\"A.fs\"")
	if cIdx < 0 || bIdx < 0 || aIdx < 0 || !(cIdx < bIdx && bIdx < aIdx) {
		t.Errorf("formatted order wrong, want C,B,A with do-not-sort in:\n%s", out)
	}
}

func TestGenerateTestSourcesExcluded(t *testing.T) {
	regular := []string{"Demo.fs", "DemoTest.fs"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/Demo.fs":     "namespace Demo\n\ntype Demo = class end\n",
		"pkg/demo/DemoTest.fs": "namespace Demo\n\nopen Acme.OnlyByTest\n\ntype DemoTest = class end\n",
	}, regular)
	if len(result.Gen) != 1 || len(result.Imports) != 1 {
		t.Fatalf("generated %d rules and %d import sets, want 1 each", len(result.Gen), len(result.Imports))
	}
	if got := strings.Join(result.Gen[0].AttrStrings("srcs"), ","); got != "Demo.fs" {
		t.Errorf("library srcs = %q, want Demo.fs", got)
	}
	if got := result.Imports[0].(targetImports); len(got.imports) != 0 {
		t.Errorf("library imports = %+v, want empty", got.imports)
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
	regular := []string{"Demo.fs", "Main.fs"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/Demo.fs": "namespace Demo\n\ntype Demo = class end\n",
		"pkg/demo/Main.fs": "namespace Demo\n\nmodule Main =\n  [<EntryPoint>]\n  let main _ = 0\n",
	}, regular)
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules for a main mix, want 0 with a recorded failure", len(result.Gen))
	}
}

func TestGenerateMixedPackagesFail(t *testing.T) {
	regular := []string{"A.fs", "B.fs"}
	result := generateFixture(t, map[string]string{
		"pkg/demo/A.fs": "namespace Alpha\n\ntype A = class end\n",
		"pkg/demo/B.fs": "namespace Beta\n\ntype B = class end\n",
	}, regular)
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules for mixed packages, want 0 with a recorded failure", len(result.Gen))
	}
}

func TestGenerateKindMismatchFails(t *testing.T) {
	root := t.TempDir()
	for name, content := range map[string]string{
		"pkg/demo/Demo.fs": "namespace Demo\n\ntype Demo = class end\n",
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
		RegularFiles: []string{"Demo.fs"},
		File:         existing,
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules over a kind mismatch, want 0 with a recorded failure", len(result.Gen))
	}
}

func TestImportsIndexNonTestSources(t *testing.T) {
	lang := NewLanguage()
	lib := rule.NewRule(LibraryKind, "demo")
	lib.SetAttr("srcs", []string{"Demo.fs", "Helper.fs", "DemoTest.fs", "notes.txt"})
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

func TestAfterResolvingDepsReportsFailures(t *testing.T) {
	l := &fsharpLang{}
	l.Before(context.Background())
	l.fail("fsharp: boom")
	defer func() {
		r := recover()
		if r == nil {
			t.Fatal("AfterResolvingDeps did not panic with recorded errors")
		}
		if msg, ok := r.(string); !ok || !strings.Contains(msg, "FSharp generation failed") {
			t.Fatalf("panic = %v, want FSharp generation failure", r)
		}
	}()
	l.AfterResolvingDeps(context.Background())
}

func TestAfterResolvingDepsClean(t *testing.T) {
	l := &fsharpLang{}
	l.Before(context.Background())
	l.AfterResolvingDeps(context.Background())
	if len(l.errors) != 0 {
		t.Errorf("errors = %v", l.errors)
	}
}

func TestLanguageMetadata(t *testing.T) {
	l := &fsharpLang{}
	if l.Name() != "fsharp" || len(l.Kinds()) != 1 || l.CheckFlags(flag.NewFlagSet("test", flag.ContinueOnError), config.New()) != nil {
		t.Fatal("invalid language metadata")
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
	if len(loads) != 1 || loads[0].Name != "@renamed_dx//fsharp/rules:defs.bzl" || strings.Join(loads[0].Symbols, ",") != "fsharp_library" {
		t.Errorf("apparent loads = %+v", loads)
	}
	if defaults := l.Loads(); len(defaults) != 1 || defaults[0].Name != "@rules_dx//fsharp/rules:defs.bzl" {
		t.Errorf("default loads = %+v", defaults)
	}
	defaultApparent := l.ApparentLoads(func(string) string { return "" })
	if defaultApparent[0].Name != "@rules_dx//fsharp/rules:defs.bzl" {
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

func resolverIndex(lang *fsharpLang, entries ...struct {
	pkg  string
	name string
}) *resolve.RuleIndex {
	index := resolve.NewRuleIndex(func(r *rule.Rule, _ string) resolve.Resolver {
		if _, ok := fsharpKinds[r.Kind()]; ok {
			return lang
		}
		return nil
	})
	for _, entry := range entries {
		r := rule.NewRule(LibraryKind, entry.name)
		r.SetAttr("srcs", []string{entry.name + ".fs"})
		index.AddRule(config.New(), r, rule.EmptyFile(filepath.Join(entry.pkg, "BUILD.bazel"), entry.pkg))
	}
	index.Finish()
	return index
}

func TestResolveBranches(t *testing.T) {
	l := &fsharpLang{}
	index := resolverIndex(l,
		struct{ pkg, name string }{"lib/b", "B"},
		struct{ pkg, name string }{"lib/a", "A"},
	)
	cfg := resolverConfig(t, nil)
	r := rule.NewRule(LibraryKind, "app")
	l.Resolve(cfg, index, nil, r, targetImports{imports: []string{"B", "A"}}, label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//lib/a:A,//lib/b:B" {
		t.Errorf("resolved deps = %q", got)
	}

	otherKind := rule.NewRule("filegroup", "other")
	l.Resolve(cfg, index, nil, otherKind, targetImports{imports: []string{"A"}}, label.New("", "app", "other"))
	if otherKind.Attr("deps") != nil {
		t.Errorf("non-library resolution emitted deps = %v", otherKind.AttrStrings("deps"))
	}

	std := rule.NewRule(LibraryKind, "uses_std")
	l.Resolve(cfg, index, nil, std, targetImports{imports: []string{"System.Collections.Generic"}}, label.New("", "app", "uses_std"))
	if std.Attr("deps") != nil || len(l.errors) != 0 {
		t.Errorf("stdlib resolution = %v, errors = %v", std.AttrStrings("deps"), l.errors)
	}

	hand := rule.NewRule(LibraryKind, "hand")
	hand.SetAttr("deps", []string{":kept"})
	l.Resolve(cfg, index, nil, hand, targetImports{imports: []string{"A"}}, label.New("", "app", "hand"))
	if got := strings.Join(hand.AttrStrings("deps"), ","); got != "//lib/a:A,:kept" {
		t.Errorf("merged deps = %q", got)
	}

	l.Resolve(cfg, index, nil, rule.NewRule(LibraryKind, "unknown"), targetImports{imports: []string{"Unknown"}}, label.New("", "app", "unknown"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "unresolved import") {
		t.Errorf("unresolved errors = %v", l.errors)
	}
	l.Resolve(cfg, index, nil, r, nil, label.New("", "app", "app"))
}

func TestResolveOverride(t *testing.T) {
	l := &fsharpLang{}
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "fsharp fsharp Mapped //mapped:dep"}})
	r := rule.NewRule(LibraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, targetImports{imports: []string{"Mapped"}}, label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//mapped:dep" {
		t.Errorf("override deps = %q", got)
	}
}

func TestResolveIgnore(t *testing.T) {
	l := &fsharpLang{}
	l.Before(context.Background())
	cfg := resolverConfig(t, nil)
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import fsharp Dropped\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "app", file)
	r := rule.NewRule(LibraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, targetImports{imports: []string{"Dropped"}}, label.New("", "app", "app"))
	if r.Attr("deps") != nil {
		t.Errorf("ignored import emitted deps = %v", r.AttrStrings("deps"))
	}
	l.AfterResolvingDeps(context.Background())
	if len(l.errors) != 0 {
		t.Errorf("ignore errors = %v", l.errors)
	}
}

func TestResolveStaleIgnoreFails(t *testing.T) {
	l := &fsharpLang{}
	l.Before(context.Background())
	cfg := resolverConfig(t, nil)
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import fsharp Unused\n"))
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
	l := &fsharpLang{}
	index := resolverIndex(l,
		struct{ pkg, name string }{"lib/one", "Dup"},
		struct{ pkg, name string }{"lib/two", "Dup"},
	)
	cfg := resolverConfig(t, nil)
	l.Resolve(cfg, index, nil, rule.NewRule(LibraryKind, "app"), targetImports{imports: []string{"Dup"}}, label.New("", "app", "app"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "ambiguous import") {
		t.Errorf("ambiguous errors = %v", l.errors)
	}
}

func TestAspectHintsSurviveRegen(t *testing.T) {
	root := t.TempDir()
	for name, content := range map[string]string{
		"pkg/demo/Demo.fs":   "module Demo\n\nlet current () = ()\n",
		"pkg/demo/Helper.fs": "module Demo\n\nlet helper () = ()\n",
	} {
		writeFixture(t, root, name, content)
	}
	build := `load("@rules_dx//fsharp/rules:defs.bzl", "fsharp_library")

fsharp_library(
    name = "demo",
    srcs = [
        "Demo.fs",
        "Helper.fs",
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
		RegularFiles: []string{"Demo.fs", "Helper.fs"},
		File:         file,
	})
	if len(result.Gen) != 1 {
		t.Fatalf("generated %d rules, want 1", len(result.Gen))
	}
	merger.MergeFile(file, result.Empty, result.Gen, merger.PreResolve, fsharpKinds, nil)
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
