package common

import (
	"context"
	"flag"
	"os"
	"path"
	"path/filepath"
	"strings"
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

const (
	dotLanguage = "dot"
	dotKind     = "dot_library"
)

func isDotStdLib(name string) bool { return strings.HasPrefix(name, "dot.") }

func dotSpec() SingleLangSpec {
	return SingleLangSpec{
		LanguageName:   dotLanguage,
		DisplayName:    "Dot",
		LibraryKind:    dotKind,
		Kinds:          map[string]rule.KindInfo{dotKind: LibraryKindInfo()},
		DefsPackage:    "dot/rules:defs.bzl",
		ImportAttr:     "srcs",
		Identity:       func(name string) string { return strings.TrimSuffix(path.Base(name), ".dot") },
		IsImportSource: func(name string) bool { return HasExt(name, []string{".dot"}) },
		IsTestSource:   func(name string) bool { return strings.HasSuffix(path.Base(name), "_test.dot") },
		IsStdLib:       isDotStdLib,
		UnresolvedHint: "add a local dot_library or an exact # gazelle:resolve mapping",
		Generate: func(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep Reporter) language.GenerateResult {
			return GenerateSingleDir(args, kinds, SingleDirSpec{
				LanguageName: dotLanguage,
				LibraryKind:  dotKind,
				IsSource:     func(name string) bool { return HasExt(name, []string{".dot"}) },
				DirName:      DirTargetName,
				DefinesMain:  func([]byte) bool { return false },
				ParseImports: func([]byte) []string { return nil },
				IsStdLib:     isDotStdLib,
				Wrap:         WrapImportSet,
			}, rep)
		},
	}
}

func newDotLang() *SingleLang { return NewSingleLang(dotSpec()) }

func dotFile(t *testing.T, root, name, content string) {
	t.Helper()
	file := filepath.Join(root, filepath.FromSlash(name))
	if err := os.MkdirAll(filepath.Dir(file), 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(file, []byte(content), 0o644); err != nil {
		t.Fatal(err)
	}
}

func TestConfigureCollectsIgnoreWitness(t *testing.T) {
	cfg := config.New()
	if got := UsedIgnores(cfg, dotLanguage); got != nil {
		t.Fatalf("absent config: %v", got)
	}
	cfg.Exts[dotLanguage] = "wrong type"
	if got := UsedIgnores(cfg, dotLanguage); got != nil {
		t.Fatalf("foreign config: %v", got)
	}
	delete(cfg.Exts, dotLanguage)
	l := newDotLang()
	f := &rule.File{Directives: []rule.Directive{{Key: "other", Value: "ignored"}, {Key: "dx_ignore_import", Value: "dot dot Widget"}}}
	l.Configure(cfg, "parent", f)
	l.Configure(cfg, "parent/child", nil)
	entry := l.matchingIgnore(cfg, "Widget")
	if entry == nil || entry.Path != "parent" {
		t.Fatalf("inherited ignore: %+v", entry)
	}
	entry.Used = true
	conf := cfg.Exts[dotLanguage].(*IgnoreConfig)
	conf.Ignores = append(conf.Ignores, nil, entry, &IgnoreEntry{Value: "Unused", Path: "parent"})
	got := UsedIgnores(cfg, dotLanguage)
	if len(got) != 1 || got[0] != [2]string{"parent", "Widget"} {
		t.Fatalf("used ignores: %v", got)
	}
	l.Configure(cfg, "parent", &rule.File{Directives: []rule.Directive{{Key: "dx_ignore_import", Value: "dot"}}})
	if len(l.errors) != 1 {
		t.Fatalf("malformed ignore accepted: %v", l.errors)
	}
}

func TestConfigureAcceptsBothIgnoreSpellings(t *testing.T) {
	cfg := config.New()
	l := newDotLang()
	l.Configure(cfg, "pkg", &rule.File{Directives: []rule.Directive{
		{Key: "dx_ignore_import", Value: "dot Short"},
		{Key: "dx_ignore_import", Value: "dot dot Long"},
		{Key: "dx_ignore_import", Value: "other dot Foreign"},
		{Key: "dx_ignore_import", Value: ""},
		{Key: "dx_ignore_import", Value: "other"},
	}})
	if len(l.errors) != 0 {
		t.Fatalf("errors = %v", l.errors)
	}
	conf := cfg.Exts[dotLanguage].(*IgnoreConfig)
	if len(conf.Ignores) != 2 || conf.Ignores[0].Value != "Short" || conf.Ignores[1].Value != "Long" {
		t.Fatalf("ignores = %+v, want Short and Long", conf.Ignores)
	}
	for _, value := range []string{"Short", "Long"} {
		if l.matchingIgnore(cfg, value) == nil {
			t.Errorf("missing ignore for %q", value)
		}
	}
	if l.matchingIgnore(cfg, "Foreign") != nil {
		t.Error("an ignore naming another language was accepted")
	}
}

func TestConfigureRejectsAMislabelledLongSpelling(t *testing.T) {
	cfg := config.New()
	l := newDotLang()
	l.Configure(cfg, "pkg", &rule.File{Directives: []rule.Directive{{Key: "dx_ignore_import", Value: "dot other Widget"}}})
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "malformed # gazelle:dx_ignore_import") {
		t.Fatalf("errors = %v, want one malformed ignore", l.errors)
	}
	if got := len(cfg.Exts[dotLanguage].(*IgnoreConfig).Ignores); got != 0 {
		t.Fatalf("ignores = %d, want 0", got)
	}
}

func TestConfigurePicksTheNearestIgnore(t *testing.T) {
	cfg := config.New()
	l := newDotLang()
	l.Configure(cfg, "pkg", &rule.File{Directives: []rule.Directive{{Key: "dx_ignore_import", Value: "dot Widget"}}})
	l.Configure(cfg, "pkg/sub", &rule.File{Directives: []rule.Directive{{Key: "dx_ignore_import", Value: "dot Widget"}}})
	entry := l.matchingIgnore(cfg, "Widget")
	if entry == nil || entry.Path != "pkg/sub" {
		t.Fatalf("nearest ignore = %+v, want the pkg/sub one", entry)
	}
}

func TestAfterResolvingDepsPanicsOnRecordedErrors(t *testing.T) {
	l := newDotLang()
	l.Before(context.Background())
	l.fail("dot: boom")
	defer func() {
		r := recover()
		if r == nil {
			t.Fatal("AfterResolvingDeps did not panic with recorded errors")
		}
		if msg, ok := r.(string); !ok || !strings.Contains(msg, "Dot generation failed") {
			t.Fatalf("panic = %v, want Dot generation failure", r)
		}
	}()
	l.AfterResolvingDeps(context.Background())
}

func TestAfterResolvingDepsSortsErrors(t *testing.T) {
	l := newDotLang()
	l.Before(context.Background())
	l.fail("dot: second")
	l.fail("dot: first")
	defer func() {
		r := recover()
		msg, ok := r.(string)
		if !ok {
			t.Fatalf("panic = %v, want a string", r)
		}
		if strings.Index(msg, "first") > strings.Index(msg, "second") {
			t.Fatalf("panic = %q, want sorted errors", msg)
		}
	}()
	l.AfterResolvingDeps(context.Background())
}

func TestAfterResolvingDepsCleanDoesNotPanic(t *testing.T) {
	l := newDotLang()
	l.Before(context.Background())
	l.AfterResolvingDeps(context.Background())
	if len(l.errors) != 0 {
		t.Errorf("errors = %v", l.errors)
	}
}

func TestAfterResolvingDepsReportsStaleIgnore(t *testing.T) {
	l := newDotLang()
	l.Before(context.Background())
	cfg := config.New()
	l.Configure(cfg, "pkg", &rule.File{Directives: []rule.Directive{{Key: "dx_ignore_import", Value: "dot Unused"}}})
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

func TestBeforeClearsErrorsAndIgnores(t *testing.T) {
	l := newDotLang()
	l.Before(context.Background())
	l.fail("dot: boom")
	cfg := config.New()
	l.Configure(cfg, "pkg", &rule.File{Directives: []rule.Directive{{Key: "dx_ignore_import", Value: "dot Widget"}}})
	if !l.Failed() {
		t.Fatal("Failed() = false with a recorded error")
	}
	l.Before(context.Background())
	if l.Failed() || len(l.ignores) != 0 {
		t.Fatalf("Before left %v / %v", l.errors, l.ignores)
	}
}

func TestLanguageMetadata(t *testing.T) {
	l := newDotLang()
	if l.Name() != dotLanguage || len(l.Kinds()) != 1 || l.CheckFlags(flag.NewFlagSet("test", flag.ContinueOnError), config.New()) != nil {
		t.Fatal("invalid language metadata")
	}
	l.RegisterFlags(flag.NewFlagSet("test", flag.ContinueOnError), "update", config.New())
	l.Configure(config.New(), "pkg", nil)
	l.DoneGeneratingRules()
	if got := l.KnownDirectives(); len(got) != 1 || got[0] != "dx_ignore_import" {
		t.Fatalf("directives = %v, want [dx_ignore_import]", got)
	}
	if l.Embeds(nil, label.NoLabel) != nil {
		t.Fatal("invalid embeds")
	}
	loads := l.ApparentLoads(func(name string) string {
		if name == "rules_dx" {
			return "renamed_dx"
		}
		return ""
	})
	if len(loads) != 1 || loads[0].Name != "@renamed_dx//dot/rules:defs.bzl" || strings.Join(loads[0].Symbols, ",") != dotKind {
		t.Errorf("apparent loads = %+v", loads)
	}
	if defaults := l.Loads(); len(defaults) != 1 || defaults[0].Name != "@rules_dx//dot/rules:defs.bzl" {
		t.Errorf("default loads = %+v", defaults)
	}
	defaultApparent := l.ApparentLoads(func(string) string { return "" })
	if defaultApparent[0].Name != "@rules_dx//dot/rules:defs.bzl" {
		t.Errorf("default apparent loads = %+v", defaultApparent)
	}
}

func TestImportsIndexesTheImportAttr(t *testing.T) {
	l := newDotLang()
	lib := rule.NewRule(dotKind, "demo")
	lib.SetAttr("srcs", []string{"Demo.dot", "Helper.dot", "Demo_test.dot", "notes.txt"})
	imports := l.Imports(&config.Config{}, lib, nil)
	if len(imports) != 2 || imports[0].Lang != dotLanguage || imports[0].Imp != "Demo" || imports[1].Imp != "Helper" {
		t.Errorf("imports = %+v, want Demo+Helper identities", imports)
	}
	if got := l.Imports(&config.Config{}, rule.NewRule("filegroup", "demo"), nil); got != nil {
		t.Errorf("other-kind imports = %+v, want nil", got)
	}
	if got := l.Imports(&config.Config{}, rule.NewRule(dotKind, "empty"), nil); got != nil {
		t.Errorf("srcless imports = %+v, want nil", got)
	}
}

func TestWrapAndUnwrapImportSet(t *testing.T) {
	names, ok := UnwrapImportSet(WrapImportSet([]string{"B", "A"}))
	if !ok || strings.Join(names, ",") != "B,A" {
		t.Errorf("round trip = %v, %v", names, ok)
	}
	if names, ok := UnwrapImportSet(WrapImportSet(nil)); !ok || names != nil {
		t.Errorf("empty set = %v, %v", names, ok)
	}
	if names, ok := UnwrapImportSet("other"); ok || names != nil {
		t.Errorf("foreign raw = %v, %v", names, ok)
	}
}

func TestHasExt(t *testing.T) {
	exts := []string{".a", ".b"}
	if !HasExt("pkg/x.a", exts) || !HasExt("pkg/x.b", exts) {
		t.Error("HasExt missed a listed extension")
	}
	if HasExt("pkg/x.c", exts) || HasExt("pkg/x", exts) || HasExt("x", nil) {
		t.Error("HasExt matched an unlisted name")
	}
}

func TestGenerateRulesDelegatesToTheSpec(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, "pkg", "demo")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	l := newDotLang()
	result := l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          dir,
		Rel:          "pkg/demo",
		RegularFiles: []string{"notes.txt"},
	})
	if len(result.Gen) != 0 {
		t.Fatalf("generated %d rules for a non-source directory, want 0", len(result.Gen))
	}
	dotFile(t, root, "pkg/demo/Demo.dot", "nothing to import\n")
	result = l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          dir,
		Rel:          "pkg/demo",
		RegularFiles: []string{"Demo.dot"},
	})
	if len(result.Gen) != 1 || result.Gen[0].Kind() != dotKind || result.Gen[0].Name() != "demo" {
		t.Fatalf("generated %+v, want one dot_library(demo)", result.Gen)
	}
	if names, ok := UnwrapImportSet(result.Imports[0]); !ok || len(names) != 0 {
		t.Errorf("imports = %v, %v, want an empty set", names, ok)
	}
}

func TestFailRecordsErrors(t *testing.T) {
	l := newDotLang()
	l.Before(context.Background())
	if l.Failed() {
		t.Fatal("a fresh language reports failure")
	}
	l.Fail("dot: %d", 7)
	if !l.Failed() {
		t.Fatal("Fail did not record an error")
	}
	if len(l.errors) != 1 || l.errors[0] != "dot: 7" {
		t.Fatalf("errors = %v", l.errors)
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

func resolverIndex(lang *SingleLang, entries ...struct {
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
		r := rule.NewRule(dotKind, entry.name)
		r.SetAttr("srcs", []string{entry.name + ".dot"})
		index.AddRule(config.New(), r, rule.EmptyFile(filepath.Join(entry.pkg, "BUILD.bazel"), entry.pkg))
	}
	index.Finish()
	return index
}

func TestResolveBranches(t *testing.T) {
	l := newDotLang()
	index := resolverIndex(l,
		struct{ pkg, name string }{"lib/b", "B"},
		struct{ pkg, name string }{"lib/a", "A"},
	)
	cfg := resolverConfig(t, nil)
	r := rule.NewRule(dotKind, "app")
	l.Resolve(cfg, index, nil, r, WrapImportSet([]string{"B", "A"}), label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//lib/a:A,//lib/b:B" {
		t.Errorf("resolved deps = %q", got)
	}

	otherKind := rule.NewRule("filegroup", "other")
	l.Resolve(cfg, index, nil, otherKind, WrapImportSet([]string{"A"}), label.New("", "app", "other"))
	if otherKind.Attr("deps") != nil {
		t.Errorf("non-library resolution emitted deps = %v", otherKind.AttrStrings("deps"))
	}

	std := rule.NewRule(dotKind, "uses_std")
	l.Resolve(cfg, index, nil, std, WrapImportSet([]string{"dot.util.List"}), label.New("", "app", "uses_std"))
	if std.Attr("deps") != nil || len(l.errors) != 0 {
		t.Errorf("stdlib resolution = %v, errors = %v", std.AttrStrings("deps"), l.errors)
	}

	hand := rule.NewRule(dotKind, "hand")
	hand.SetAttr("deps", []string{":kept"})
	l.Resolve(cfg, index, nil, hand, WrapImportSet([]string{"A"}), label.New("", "app", "hand"))
	if got := strings.Join(hand.AttrStrings("deps"), ","); got != "//lib/a:A,:kept" {
		t.Errorf("merged deps = %q", got)
	}

	l.Resolve(cfg, index, nil, rule.NewRule(dotKind, "unknown"), WrapImportSet([]string{"Unknown"}), label.New("", "app", "unknown"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "unresolved import") {
		t.Errorf("unresolved errors = %v", l.errors)
	}
	if !strings.Contains(l.errors[0], "add a local dot_library or an exact # gazelle:resolve mapping") {
		t.Errorf("unresolved error = %q, want the spec hint", l.errors[0])
	}

	l.Resolve(cfg, index, nil, r, nil, label.New("", "app", "app"))
	if len(l.errors) != 1 {
		t.Errorf("a foreign raw value was not ignored: %v", l.errors)
	}
}

func TestResolveOverride(t *testing.T) {
	l := newDotLang()
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "dot dot Mapped //mapped:dep"}})
	r := rule.NewRule(dotKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, WrapImportSet([]string{"Mapped"}), label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//mapped:dep" {
		t.Errorf("override deps = %q", got)
	}
}

func TestResolveOverrideAndIgnoreConflict(t *testing.T) {
	l := newDotLang()
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "dot dot Mapped //mapped:dep"}})
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import dot Mapped\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "app", file)
	r := rule.NewRule(dotKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, WrapImportSet([]string{"Mapped"}), label.New("", "app", "app"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "both an exact resolve mapping and ignore") {
		t.Errorf("conflict errors = %v", l.errors)
	}
	if r.Attr("deps") != nil {
		t.Errorf("conflicting import emitted deps = %v", r.AttrStrings("deps"))
	}
}

func TestResolveIgnore(t *testing.T) {
	l := newDotLang()
	l.Before(context.Background())
	cfg := resolverConfig(t, nil)
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import dot Dropped\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "app", file)
	r := rule.NewRule(dotKind, "app")
	l.Resolve(cfg, resolverIndex(l), nil, r, WrapImportSet([]string{"Dropped"}), label.New("", "app", "app"))
	if r.Attr("deps") != nil {
		t.Errorf("ignored import emitted deps = %v", r.AttrStrings("deps"))
	}
	l.AfterResolvingDeps(context.Background())
	if len(l.errors) != 0 {
		t.Errorf("ignore errors = %v", l.errors)
	}
}

func TestResolveSkipsTheFromLabel(t *testing.T) {
	l := newDotLang()
	index := resolverIndex(l, struct{ pkg, name string }{"app", "app"})
	cfg := resolverConfig(t, nil)
	r := rule.NewRule(dotKind, "app")
	l.Resolve(cfg, index, nil, r, WrapImportSet([]string{"app"}), label.New("", "app", "app"))
	if r.Attr("deps") != nil || len(l.errors) != 0 {
		t.Errorf("self import = %v, errors = %v", r.AttrStrings("deps"), l.errors)
	}
}

func TestResolveAmbiguousFails(t *testing.T) {
	l := newDotLang()
	index := resolverIndex(l,
		struct{ pkg, name string }{"lib/one", "Dup"},
		struct{ pkg, name string }{"lib/two", "Dup"},
	)
	cfg := resolverConfig(t, nil)
	l.Resolve(cfg, index, nil, rule.NewRule(dotKind, "app"), WrapImportSet([]string{"Dup"}), label.New("", "app", "app"))
	if len(l.errors) != 1 || !strings.Contains(l.errors[0], "ambiguous import") {
		t.Errorf("ambiguous errors = %v", l.errors)
	}
}
