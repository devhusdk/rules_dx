package fsharp

import (
	"context"
	"flag"
	"os"
	"path/filepath"
	"sort"
	"strings"
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/merger"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
	bzl "github.com/bazelbuild/buildtools/build"
	"github.com/devhusdk/rules_dx/gazelle/common"
)

func newLang() *common.SingleLang {
	l := NewLanguage().(*common.SingleLang)
	l.Before(context.Background())
	return l
}

func takeErrors(l *common.SingleLang) []string {
	l.AfterResolvingDeps(context.Background())
	return common.Messages(common.Errors.Take())
}

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
	lang := newLang()
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
	if errs := takeErrors(lang); len(errs) != 1 {
		t.Fatalf("malformed directive accepted: %v", errs)
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
	got, _ := common.UnwrapImportSet(result.Imports[0])
	if strings.Join(got, ",") != "Widget" {
		t.Errorf("library imports = %+v, want [Widget]", got)
	}
}

func generatedSrcs(t *testing.T, files map[string]string) string {
	t.Helper()
	var regular []string
	for name := range files {
		regular = append(regular, strings.TrimPrefix(name, "pkg/demo/"))
	}
	sort.Strings(regular)
	result := generateFixture(t, files, regular)
	if len(result.Gen) != 1 {
		t.Fatalf("generated %d rules, want 1", len(result.Gen))
	}
	return strings.Join(result.Gen[0].AttrStrings("srcs"), ",")
}

func TestGenerateDependencyOrder(t *testing.T) {
	got := generatedSrcs(t, map[string]string{
		"pkg/demo/Demo.fs":   "module Demo\n\nlet greet name = \"hello \" + name + Helper.suffix\n",
		"pkg/demo/Helper.fs": "module Helper\n\nlet suffix = \"\"\n",
	})
	if got != "Helper.fs,Demo.fs" {
		t.Errorf("library srcs = %q, want Helper.fs,Demo.fs (dependency first)", got)
	}
}

func TestGenerateChainOrder(t *testing.T) {
	got := generatedSrcs(t, map[string]string{
		"pkg/demo/A.fs": "namespace Demo\n\ntype A =\n    static member Value() = B.Value() + \"a\"\n",
		"pkg/demo/B.fs": "namespace Demo\n\ntype B =\n    static member Value() = C.Value() + \"b\"\n",
		"pkg/demo/C.fs": "namespace Demo\n\ntype C =\n    static member Value() = \"c\"\n",
	})
	if got != "C.fs,B.fs,A.fs" {
		t.Errorf("library srcs = %q, want C.fs,B.fs,A.fs (chain dependency order)", got)
	}
}

func TestGenerateCycleFails(t *testing.T) {
	regular := []string{"A.fs", "B.fs"}
	got := refuseFixture(t, map[string]string{
		"pkg/demo/A.fs": "namespace Demo\n\ntype A = { b: B option }\n",
		"pkg/demo/B.fs": "namespace Demo\n\ntype B = { a: A option }\n",
	}, regular)
	wantRefusal(t, got, "fsharp: pkg/demo: cyclic F# compile order among A.fs, B.fs, first edge A.fs -> B.fs (B);")
}

func TestGenerateNestedModule(t *testing.T) {
	got := generatedSrcs(t, map[string]string{
		"pkg/demo/Core.fs": "namespace Demo\n\nmodule Utils =\n    let twice x = x * 2\n\ntype Core = { size: int }\n",
		"pkg/demo/App.fs":  "namespace Demo\n\nmodule App =\n    let run () = Utils.twice 21\n",
	})
	if got != "Core.fs,App.fs" {
		t.Errorf("library srcs = %q, want Core.fs,App.fs", got)
	}
}

func TestGenerateModulePerFile(t *testing.T) {
	got := generatedSrcs(t, map[string]string{
		"pkg/demo/Alpha.fs":  "module Alpha\n\nlet run () = Format.wrap Zeta.value\n",
		"pkg/demo/Zeta.fs":   "module Zeta\n\nlet value = 1\n",
		"pkg/demo/Format.fs": "module Format\n\nlet wrap x = string x\n",
	})
	if got != "Format.fs,Zeta.fs,Alpha.fs" {
		t.Errorf("library srcs = %q, want Format.fs,Zeta.fs,Alpha.fs", got)
	}
}

func TestGenerateDottedModulePerFile(t *testing.T) {
	got := generatedSrcs(t, map[string]string{
		"pkg/demo/App.fs":   "module Demo.App\n\nopen Demo.Types\n\nlet run (o: Order) = o.qty\n",
		"pkg/demo/Model.fs": "module Demo.Types\n\ntype Order = { qty: int }\n",
	})
	if got != "Model.fs,App.fs" {
		t.Errorf("library srcs = %q, want Model.fs,App.fs", got)
	}
}

func TestGenerateUnqualifiedTypeRef(t *testing.T) {
	got := generatedSrcs(t, map[string]string{
		"pkg/demo/A.fs": "namespace Demo\n\ntype A = { b: Item }\n",
		"pkg/demo/B.fs": "namespace Demo\n\ntype Item = { size: int }\n",
	})
	if got != "B.fs,A.fs" {
		t.Errorf("library srcs = %q, want B.fs,A.fs", got)
	}
}

func TestGenerateSignaturePair(t *testing.T) {
	got := generatedSrcs(t, map[string]string{
		"pkg/demo/Demo.fs":    "module Demo\n\nlet greet name = name + Helper.suffix\n",
		"pkg/demo/Helper.fs":  "module Helper\n\nlet suffix = \"!\"\n",
		"pkg/demo/Helper.fsi": "module Helper\n\nval suffix: string\n",
	})
	if got != "Helper.fsi,Helper.fs,Demo.fs" {
		t.Errorf("library srcs = %q, want Helper.fsi,Helper.fs,Demo.fs", got)
	}
}

func TestGenerateLoneSignatureFails(t *testing.T) {
	got := refuseFixture(t, map[string]string{
		"pkg/demo/Demo.fs":    "module Demo\n\nlet x = 1\n",
		"pkg/demo/Helper.fsi": "module Helper\n\nval suffix: string\n",
	}, []string{"Demo.fs", "Helper.fsi"})
	wantRefusal(t, got, "fsharp: pkg/demo: Helper.fsi has no implementation Helper.fs")
}

func TestGenerateNamespaceAndSubmoduleOneGroup(t *testing.T) {
	got := generatedSrcs(t, map[string]string{
		"pkg/demo/Demo.fs":   "namespace Demo\n\ntype Greeter =\n    static member Greet() = Helper.suffix\n",
		"pkg/demo/Helper.fs": "module Demo.Helper\n\nlet suffix = \"!\"\n",
	})
	if got != "Helper.fs,Demo.fs" {
		t.Errorf("library srcs = %q, want Helper.fs,Demo.fs", got)
	}
}

func TestGenerateAmbiguousDeclaredNameIgnored(t *testing.T) {
	got := generatedSrcs(t, map[string]string{
		"pkg/demo/A.fs": "namespace Demo\n\ntype A = { s: Shared }\n",
		"pkg/demo/B.fs": "namespace Demo\n\ntype Shared = int\n",
		"pkg/demo/C.fs": "namespace Demo\n\ntype Shared = string\n",
	})
	if got != "A.fs,B.fs,C.fs" {
		t.Errorf("library srcs = %q, want A.fs,B.fs,C.fs", got)
	}
}

func TestOrderSourcesByDependencyUnit(t *testing.T) {
	ordered, err := orderSourcesByDependency(
		[]string{"Demo.fs", "Helper.fs"},
		map[string][]byte{
			"Demo.fs":   []byte("module Demo\n\nlet x = Helper.suffix\n"),
			"Helper.fs": []byte("module Helper\n\nlet suffix = 1\n"),
		},
	)
	if err != nil || strings.Join(ordered, ",") != "Helper.fs,Demo.fs" {
		t.Errorf("orderSourcesByDependency = %v, %v; want Helper.fs,Demo.fs", ordered, err)
	}
	ordered, err = orderSourcesByDependency(
		[]string{"Demo.fs", "Lib.fs"},
		map[string][]byte{
			"Demo.fs": []byte("namespace Demo\n\nopen Acme.Lib\n"),
			"Lib.fs":  []byte("namespace Acme\n"),
		},
	)
	if err != nil || strings.Join(ordered, ",") != "Lib.fs,Demo.fs" {
		t.Errorf("orderSourcesByDependency open = %v, %v; want Lib.fs,Demo.fs", ordered, err)
	}
	ordered, err = orderSourcesByDependency(
		[]string{"App.fs", "Model.fs", "Root.fs"},
		map[string][]byte{
			"App.fs":   []byte("module Demo.App\n\nopen Demo.Types.Orders\n\nlet run (o: Order) = o.qty\n"),
			"Model.fs": []byte("module Demo.Types\n\nmodule Orders =\n    type Order = { qty: int }\n"),
			"Root.fs":  []byte("module Root\n\nopen Demo.Types\n\nlet zero: Orders.Order = { qty = 0 }\n"),
		},
	)
	if err != nil || strings.Join(ordered, ",") != "Model.fs,App.fs,Root.fs" {
		t.Errorf("orderSourcesByDependency dotted open = %v, %v; want Model.fs,App.fs,Root.fs", ordered, err)
	}
	if _, err := orderSourcesByDependency(
		[]string{"A.fs", "B.fs"},
		map[string][]byte{
			"A.fs": []byte("module A\n\nlet x = B.y\n"),
			"B.fs": []byte("module B\n\nlet y = A.x\n"),
		},
	); err == nil || !strings.Contains(err.Error(), "first edge A.fs -> B.fs (B)") {
		t.Errorf("orderSourcesByDependency cycle = %v, want named edge", err)
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
	if got, _ := common.UnwrapImportSet(result.Imports[0]); len(got) != 0 {
		t.Errorf("library imports = %+v, want empty", got)
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
	got := refuseFixture(t, map[string]string{
		"pkg/demo/Demo.fs": "namespace Demo\n\ntype Demo = class end\n",
		"pkg/demo/Main.fs": "namespace Demo\n\nmodule Main =\n  [<EntryPoint>]\n  let main _ = 0\n",
	}, regular)
	wantRefusal(t, got, "fsharp: pkg/demo: Main.fs defines main;")
	if strings.Contains(got, "duplicate") {
		t.Errorf("refusal = %q, want no duplicate-declaration error", got)
	}
}

func TestGenerateMixedPackagesFail(t *testing.T) {
	regular := []string{"A.fs", "B.fs"}
	got := refuseFixture(t, map[string]string{
		"pkg/demo/A.fs": "namespace Alpha\n\ntype A = class end\n",
		"pkg/demo/B.fs": "namespace Beta\n\ntype B = class end\n",
	}, regular)
	wantRefusal(t, got, "fsharp: pkg/demo: mixed packages Alpha, Beta in one directory;")
	got = refuseFixture(t, map[string]string{
		"pkg/demo/A.fs": "namespace Demo\n\ntype A = class end\n",
		"pkg/demo/B.fs": "module Helper\n\nlet b = 1\n",
	}, regular)
	wantRefusal(t, got, "fsharp: pkg/demo: mixed packages Demo, global in one directory;")
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
	lang := NewLanguage()
	result := lang.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"Demo.fs"},
		File:         existing,
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules over a kind mismatch, want 0 with a recorded failure", len(result.Gen))
	}
	wantRefusal(t, refusal(t, lang), "fsharp: pkg/demo: target name \"demo\" is claimed by generated fsharp_library(pkg/demo) and existing filegroup")
}

func TestImportsIndexNonTestSources(t *testing.T) {
	lang := NewLanguage()
	lib := rule.NewRule(LibraryKind, "demo")
	lib.SetAttr("srcs", []string{"Demo.fs", "Helper.fsi", "Helper.fs", "DemoTest.fs", "notes.txt"})
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
	l := newLang()
	l.Fail("fsharp: boom")
	l.AfterResolvingDeps(context.Background())
	got := common.Errors.Take()
	if len(got) != 1 || got[0].Language != languageName || got[0].Message != "fsharp: boom" {
		t.Fatalf("sink = %#v, want one fsharp failure", got)
	}
}

func TestAfterResolvingDepsClean(t *testing.T) {
	l := newLang()
	l.AfterResolvingDeps(context.Background())
	if l.Failed() {
		t.Errorf("errors = %v", common.Messages(common.Errors.Take()))
	}
}

func TestLanguageMetadata(t *testing.T) {
	l := newLang()
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

func resolverIndex(lang *common.SingleLang, entries ...struct {
	pkg  string
	name string
}) *resolve.RuleIndex {
	index := resolve.NewRuleIndex(func(r *rule.Rule, _ string) resolve.Resolver {
		if _, ok := lang.Kinds()[r.Kind()]; ok {
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
	l := newLang()
	index := resolverIndex(l,
		struct{ pkg, name string }{"lib/b", "B"},
		struct{ pkg, name string }{"lib/a", "A"},
	)
	cfg := resolverConfig(t, nil)
	r := rule.NewRule(LibraryKind, "app")
	l.Resolve(cfg, index, nil, r, common.WrapImportSet([]string{"B", "A"}), label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//lib/a:A,//lib/b:B" {
		t.Errorf("resolved deps = %q", got)
	}

	otherKind := rule.NewRule("filegroup", "other")
	l.Resolve(cfg, index, nil, otherKind, common.WrapImportSet([]string{"A"}), label.New("", "app", "other"))
	if otherKind.Attr("deps") != nil {
		t.Errorf("non-library resolution emitted deps = %v", otherKind.AttrStrings("deps"))
	}

	std := rule.NewRule(LibraryKind, "uses_std")
	l.Resolve(cfg, index, nil, std, common.WrapImportSet([]string{"System.Collections.Generic"}), label.New("", "app", "uses_std"))
	if std.Attr("deps") != nil || l.Failed() {
		t.Errorf("stdlib resolution = %v, failed = %v", std.AttrStrings("deps"), l.Failed())
	}

	hand := rule.NewRule(LibraryKind, "hand")
	hand.SetAttr("deps", []string{":kept"})
	l.Resolve(cfg, index, nil, hand, common.WrapImportSet([]string{"A"}), label.New("", "app", "hand"))
	if got := strings.Join(hand.AttrStrings("deps"), ","); got != "//lib/a:A,:kept" {
		t.Errorf("merged deps = %q", got)
	}

	l.Resolve(cfg, index, nil, rule.NewRule(LibraryKind, "unknown"), common.WrapImportSet([]string{"Unknown"}), label.New("", "app", "unknown"))
	if errs := takeErrors(l); len(errs) != 1 || !strings.Contains(errs[0], "unresolved import") {
		t.Errorf("unresolved errors = %v", errs)
	}
	l.Resolve(cfg, index, nil, r, nil, label.New("", "app", "app"))
}

func TestResolveOverride(t *testing.T) {
	l := newLang()
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "fsharp fsharp Mapped //mapped:dep"}})
	r := rule.NewRule(LibraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, common.WrapImportSet([]string{"Mapped"}), label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//mapped:dep" {
		t.Errorf("override deps = %q", got)
	}
}

func TestResolveIgnore(t *testing.T) {
	l := newLang()
	cfg := resolverConfig(t, nil)
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import fsharp Dropped\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "app", file)
	r := rule.NewRule(LibraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, common.WrapImportSet([]string{"Dropped"}), label.New("", "app", "app"))
	if r.Attr("deps") != nil {
		t.Errorf("ignored import emitted deps = %v", r.AttrStrings("deps"))
	}
	l.AfterResolvingDeps(context.Background())
	if l.Failed() {
		t.Errorf("ignore errors = %v", takeErrors(l))
	}
}

func TestResolveStaleIgnoreFails(t *testing.T) {
	l := newLang()
	cfg := resolverConfig(t, nil)
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import fsharp Unused\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "app", file)
	l.GenerateRules(language.GenerateArgs{Config: cfg, Rel: "app"})
	l.AfterResolvingDeps(context.Background())
	if msg := strings.Join(common.Messages(common.Errors.Take()), "\n"); !strings.Contains(msg, "stale # gazelle:dx_ignore_import") {
		t.Fatalf("errors = %q, want stale ignore failure", msg)
	}
}

func TestResolveMappingIgnoreConflict(t *testing.T) {
	l := newLang()
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "fsharp fsharp Mapped //mapped:dep"}})
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import fsharp Mapped\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "app", file)
	r := rule.NewRule(LibraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, common.WrapImportSet([]string{"Mapped"}), label.New("", "app", "app"))
	if errs := takeErrors(l); len(errs) != 1 || !strings.Contains(errs[0], "both an exact resolve mapping and ignore") {
		t.Errorf("conflict errors = %v", errs)
	}
}

func TestOrderSourcesByDependencyMissingContent(t *testing.T) {
	ordered, err := orderSourcesByDependency([]string{"A.fs"}, map[string][]byte{})
	if err != nil || strings.Join(ordered, ",") != "A.fs" {
		t.Errorf("orderSourcesByDependency = %v, %v; want A.fs", ordered, err)
	}
}

func TestResolveAmbiguousFails(t *testing.T) {
	l := newLang()
	index := resolverIndex(l,
		struct{ pkg, name string }{"lib/one", "Dup"},
		struct{ pkg, name string }{"lib/two", "Dup"},
	)
	cfg := resolverConfig(t, nil)
	l.Resolve(cfg, index, nil, rule.NewRule(LibraryKind, "app"), common.WrapImportSet([]string{"Dup"}), label.New("", "app", "app"))
	if errs := takeErrors(l); len(errs) != 1 || !strings.Contains(errs[0], "ambiguous import") {
		t.Errorf("ambiguous errors = %v", errs)
	}
}

func TestAspectHintsSurviveRegen(t *testing.T) {
	root := t.TempDir()
	for name, content := range map[string]string{
		"pkg/demo/Demo.fs":   "module Demo\n\nlet current () = ()\n",
		"pkg/demo/Helper.fs": "module Helper\n\nlet helper () = ()\n",
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
	files := map[string]string{
		"Alpha.fs": "namespace Demo\n\ntype Alpha = class end\n",
		"Beta.fs":  "namespace Demo\n\ntype Beta = class end\n",
	}
	for _, tc := range []struct{ repoName, want string }{{"demo-repo", "demo_repo"}, {"", "lib"}} {
		result := generateRoot(t, tc.repoName, files)
		if len(result.Gen) != 1 || result.Gen[0].Kind() != LibraryKind || result.Gen[0].Name() != tc.want {
			t.Fatalf("repo %q: generated %+v, want one fsharp_library(%s)", tc.repoName, result.Gen, tc.want)
		}
		if got := strings.Join(result.Gen[0].AttrStrings("srcs"), ","); got != "Alpha.fs,Beta.fs" {
			t.Errorf("repo %q: srcs = %q, want Alpha.fs,Beta.fs", tc.repoName, got)
		}
	}
}

func TestResolveNuGetNamespaceUsesMappingAndFailsWhenUnmapped(t *testing.T) {
	parsed := ParseImports([]byte("open System.CommandLine\n"))
	if len(parsed) != 1 {
		t.Fatalf("ParseImports = %v, want one import", parsed)
	}
	from := label.New("", "app", "app")

	l := newLang()
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "fsharp fsharp " + parsed[0] + " //mapped:dep"}})
	r := rule.NewRule(LibraryKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, common.WrapImportSet(parsed), from)
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//mapped:dep" || l.Failed() {
		t.Fatalf("deps = %q errors = %v", got, takeErrors(l))
	}

	l = newLang()
	cfg = resolverConfig(t, nil)
	l.Resolve(cfg, resolverIndex(l), nil, rule.NewRule(LibraryKind, "app"), common.WrapImportSet(parsed), from)
	if errs := takeErrors(l); len(errs) != 1 || !strings.Contains(errs[0], "unresolved import") {
		t.Fatalf("errors = %v, want unresolved import", errs)
	}
}
