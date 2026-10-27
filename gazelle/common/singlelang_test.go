package common

import (
	"context"
	"errors"
	"flag"
	"os"
	"path"
	"path/filepath"
	"reflect"
	"slices"
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
	entry := MatchingIgnore(cfg, dotLanguage, "Widget")
	if entry == nil || entry.Path != "parent" {
		t.Fatalf("inherited ignore: %+v", entry)
	}
	entry.MarkUsed("parent/child")
	conf := cfg.Exts[dotLanguage].(*IgnoreConfig)
	conf.Ignores = append(conf.Ignores, nil, entry, &IgnoreEntry{Value: "Unused", Path: "parent"})
	got := UsedIgnores(cfg, dotLanguage)
	if len(got) != 1 || got[0].Path != "parent" || got[0].Value != "Widget" || !reflect.DeepEqual(got[0].UsedBy, []string{"parent/child"}) {
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
		if MatchingIgnore(cfg, dotLanguage, value) == nil {
			t.Errorf("missing ignore for %q", value)
		}
	}
	if MatchingIgnore(cfg, dotLanguage, "Foreign") != nil {
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
	entry := MatchingIgnore(cfg, dotLanguage, "Widget")
	if entry == nil || entry.Path != "pkg/sub" {
		t.Fatalf("nearest ignore = %+v, want the pkg/sub one", entry)
	}
}

func TestAfterResolvingDepsReportsToSink(t *testing.T) {
	l := newDotLang()
	l.Before(context.Background())
	l.fail("dot: boom")
	l.AfterResolvingDeps(context.Background())
	got := Errors.Take()
	want := []GenerationError{{Language: dotLanguage, Message: "dot: boom"}}
	if !reflect.DeepEqual(got, want) {
		t.Fatalf("sink = %#v, want %#v", got, want)
	}
}

func TestAfterResolvingDepsSortsErrors(t *testing.T) {
	l := newDotLang()
	l.Before(context.Background())
	l.fail("dot: second")
	l.fail("dot: first")
	l.AfterResolvingDeps(context.Background())
	got := Errors.Take()
	if len(got) != 2 || got[0].Message != "dot: first" || got[1].Message != "dot: second" {
		t.Fatalf("sink = %#v, want sorted errors", got)
	}
}

func TestAfterResolvingDepsCleanReportsNothing(t *testing.T) {
	l := newDotLang()
	l.Before(context.Background())
	l.AfterResolvingDeps(context.Background())
	if len(l.errors) != 0 {
		t.Errorf("errors = %v", l.errors)
	}
	if got := Errors.Take(); got != nil {
		t.Errorf("sink = %#v, want empty", got)
	}
}

func TestAfterResolvingDepsReportsStaleIgnore(t *testing.T) {
	l := newDotLang()
	l.Before(context.Background())
	cfg := config.New()
	l.Configure(cfg, "pkg", &rule.File{Directives: []rule.Directive{{Key: "dx_ignore_import", Value: "dot Unused"}}})
	l.generated.Add("pkg")
	l.AfterResolvingDeps(context.Background())
	got := Errors.Take()
	if len(got) != 1 || !strings.Contains(got[0].Message, "stale # gazelle:dx_ignore_import") {
		t.Fatalf("sink = %#v, want stale ignore failure", got)
	}
}

func TestAfterResolvingDepsSkipsIgnoresOutsideGeneratedPackages(t *testing.T) {
	l := newDotLang()
	l.Before(context.Background())
	cfg := config.New()
	l.Configure(cfg, "c", &rule.File{Directives: []rule.Directive{{Key: "dx_ignore_import", Value: "dot Unused"}}})
	l.GenerateRules(language.GenerateArgs{Config: cfg, Rel: "a"})
	l.AfterResolvingDeps(context.Background())
	if len(l.errors) != 0 {
		t.Errorf("errors = %v, want the out-of-scope directive skipped", l.errors)
	}
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

func TestImportsWithoutIdentityIndexesQualifiedIDsOnly(t *testing.T) {
	spec := dotSpec()
	spec.Identity = nil
	var paths []string
	spec.QualifiedIDs = func(path string) []string {
		paths = append(paths, path)
		return []string{"Acme.Demo"}
	}
	l := NewSingleLang(spec)
	tmp := t.TempDir()
	lib := rule.NewRule(dotKind, "demo")
	lib.SetAttr("srcs", []string{"Demo.dot", "Helper.dot", "Demo_test.dot"})
	imports := l.Imports(&config.Config{}, lib, &rule.File{Path: filepath.Join(tmp, "BUILD.bazel")})
	if len(imports) != 1 || imports[0].Lang != dotLanguage || imports[0].Imp != "Acme.Demo" {
		t.Errorf("imports = %+v, want only Acme.Demo", imports)
	}
	if want := []string{filepath.Join(tmp, "Demo.dot"), filepath.Join(tmp, "Helper.dot")}; !slices.Equal(paths, want) {
		t.Errorf("qualified paths = %v, want %v", paths, want)
	}
	if got := l.Imports(&config.Config{}, lib, nil); got != nil {
		t.Errorf("nil file imports = %+v, want nil", got)
	}
}

func TestImportsPathIdentityQualifiesWithPackage(t *testing.T) {
	spec := dotSpec()
	spec.PathIdentity = true
	l := NewSingleLang(spec)
	lib := rule.NewRule(dotKind, "util")
	lib.SetAttr("srcs", []string{"util.dot"})
	imports := l.Imports(&config.Config{}, lib, &rule.File{Pkg: "a"})
	if len(imports) != 1 || imports[0].Imp != "a/util" {
		t.Errorf("imports = %+v, want a/util", imports)
	}
}

func TestImportsWithoutATestSourceFilter(t *testing.T) {
	spec := dotSpec()
	spec.IsTestSource = nil
	l := NewSingleLang(spec)
	lib := rule.NewRule(dotKind, "demo")
	lib.SetAttr("srcs", []string{"Demo.dot", "Demo_test.dot", "notes.txt"})
	imports := l.Imports(&config.Config{}, lib, nil)
	if len(imports) != 2 || imports[0].Imp != "Demo" || imports[1].Imp != "Demo_test" {
		t.Errorf("imports = %+v, want Demo and Demo_test", imports)
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

func TestGenerateRootPackage(t *testing.T) {
	root := t.TempDir()
	dotFile(t, root, "Root.dot", "nothing to import\n")
	for _, tc := range []struct{ repoName, want string }{{"demo-repo", "demo_repo"}, {"", "lib"}} {
		result := newDotLang().GenerateRules(language.GenerateArgs{
			Config:       &config.Config{RepoRoot: root, RepoName: tc.repoName},
			Dir:          root,
			Rel:          "",
			RegularFiles: []string{"Root.dot"},
		})
		if len(result.Gen) != 1 || result.Gen[0].Kind() != dotKind || result.Gen[0].Name() != tc.want {
			t.Fatalf("repo %q: generated %+v, want one dot_library(%s)", tc.repoName, result.Gen, tc.want)
		}
	}
}

func extendedDotLang(extend func(r *rule.Rule, files []string, contents map[string][]byte) error) *SingleLang {
	spec := dotSpec()
	spec.Generate = func(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep Reporter) language.GenerateResult {
		return GenerateSingleDir(args, kinds, SingleDirSpec{
			LanguageName: dotLanguage,
			LibraryKind:  dotKind,
			IsSource:     func(name string) bool { return HasExt(name, []string{".dot"}) },
			DirName:      DirTargetName,
			DefinesMain:  func([]byte) bool { return false },
			ParseImports: func([]byte) []string { return nil },
			Extend:       extend,
			Wrap:         WrapImportSet,
		}, rep)
	}
	return NewSingleLang(spec)
}

func generateExtended(t *testing.T, l *SingleLang) language.GenerateResult {
	t.Helper()
	root := t.TempDir()
	dotFile(t, root, "pkg/demo/Demo.dot", "embeds data.txt\n")
	dotFile(t, root, "pkg/demo/data.txt", "payload\n")
	return l.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"Demo.dot", "data.txt"},
	})
}

func TestGenerateExtendSeesFilesAndContents(t *testing.T) {
	var gotFiles []string
	var gotContent string
	l := extendedDotLang(func(r *rule.Rule, files []string, contents map[string][]byte) error {
		gotFiles = files
		gotContent = string(contents["Demo.dot"])
		r.SetAttr("data", []string{"data.txt"})
		return nil
	})
	result := generateExtended(t, l)
	if l.Failed() || len(result.Gen) != 1 {
		t.Fatalf("generated %d rules (failed=%v), want 1", len(result.Gen), l.Failed())
	}
	if got := strings.Join(result.Gen[0].AttrStrings("data"), ","); got != "data.txt" {
		t.Errorf("data = %q, want data.txt", got)
	}
	if strings.Join(gotFiles, ",") != "Demo.dot,data.txt" || gotContent != "embeds data.txt\n" {
		t.Errorf("hook saw files %v and content %q", gotFiles, gotContent)
	}
}

func TestGenerateExtendErrorRecordsAFailure(t *testing.T) {
	l := extendedDotLang(func(*rule.Rule, []string, map[string][]byte) error {
		return errors.New("embeds a missing file")
	})
	result := generateExtended(t, l)
	if !l.Failed() || len(result.Gen) != 0 || len(result.Imports) != 0 {
		t.Fatalf("generated %d rules (failed=%v), want 0 with a recorded failure", len(result.Gen), l.Failed())
	}
	l.AfterResolvingDeps(context.Background())
	msg := strings.Join(Messages(Errors.Take()), "\n")
	if !strings.Contains(msg, "dot: pkg/demo: embeds a missing file") {
		t.Fatalf("errors = %q, want the hook error", msg)
	}
}

func TestGenerateSingleFileRecordsAFailurePerPlan(t *testing.T) {
	cases := []struct {
		name    string
		files   map[string]string
		regular []string
		want    string
	}{
		{"readError", map[string]string{}, []string{"missing.dot"}, "read missing.dot"},
		{"emptyName", map[string]string{"pkg/demo/---.dot": ""}, []string{"---.dot"}, "empty target name"},
		{"collision", map[string]string{
			"pkg/demo/a-b.dot": "",
			"pkg/demo/a_b.dot": "",
		}, []string{"a-b.dot", "a_b.dot"}, "claimed by a-b.dot, a_b.dot"},
	}
	for _, tc := range cases {
		root := t.TempDir()
		for name, content := range tc.files {
			dotFile(t, root, name, content)
		}
		spec := dotSpec()
		spec.Generate = func(args language.GenerateArgs, kinds map[string]rule.KindInfo, rep Reporter) language.GenerateResult {
			return GenerateSingleFile(args, kinds, SingleFileSpec{
				LanguageName: dotLanguage,
				LibraryKind:  dotKind,
				IsSource:     func(name string) bool { return HasExt(name, []string{".dot"}) },
				TargetName: func(name string) (string, error) {
					trimmed := strings.TrimSuffix(path.Base(name), ".dot")
					if trimmed == "---" {
						return "", errors.New("empty target name")
					}
					return strings.ReplaceAll(trimmed, "-", "_"), nil
				},
				ParseImports: func([]byte, string) []string { return nil },
				Wrap:         WrapImportSet,
			}, rep)
		}
		l := NewSingleLang(spec)
		result := l.GenerateRules(language.GenerateArgs{
			Config:       &config.Config{RepoRoot: root},
			Dir:          filepath.Join(root, "pkg", "demo"),
			Rel:          "pkg/demo",
			RegularFiles: tc.regular,
		})
		if len(result.Gen) != 0 {
			t.Errorf("%s: generated %d rules, want none", tc.name, len(result.Gen))
		}
		if len(l.errors) != 1 || !strings.Contains(l.errors[0], tc.want) {
			t.Errorf("%s: errors = %v, want %q", tc.name, l.errors, tc.want)
		}
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

func newJVMDotLang() *SingleLang {
	spec := dotSpec()
	spec.Candidates = JVMCandidates
	return NewSingleLang(spec)
}

func TestResolveJVMCandidates(t *testing.T) {
	l := newJVMDotLang()
	index := resolverIndex(l,
		struct{ pkg, name string }{"lib/x", "com.x"},
		struct{ pkg, name string }{"lib/acme", "com.acme"},
	)
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "dot dot org.y //third_party:y"}})
	for _, tc := range []struct{ imp, want string }{
		{"com.x.runBlocking", "//lib/x:com.x"},
		{"com.x.Outer.Inner", "//lib/x:com.x"},
		{"com.x.*", "//lib/x:com.x"},
		{"org.y.Thing", "//third_party:y"},
		{"org.y.*", "//third_party:y"},
	} {
		r := rule.NewRule(dotKind, "app")
		l.Resolve(cfg, index, nil, r, WrapImportSet([]string{tc.imp}), label.New("", "app", "app"))
		if got := strings.Join(r.AttrStrings("deps"), ","); got != tc.want {
			t.Errorf("%s: deps = %q, want %q", tc.imp, got, tc.want)
		}
	}
	if len(l.errors) != 0 {
		t.Fatalf("errors = %v", l.errors)
	}
	// A candidate walk never climbs into a parent package.
	for _, imp := range []string{"com.acme.lib.Client", "org.y.sub.Thing"} {
		l.Resolve(cfg, index, nil, rule.NewRule(dotKind, "app"), WrapImportSet([]string{imp}), label.New("", "app", "app"))
	}
	if len(l.errors) != 2 || !strings.Contains(l.errors[0], `"com.acme.lib.Client"`) || !strings.Contains(l.errors[1], `"org.y.sub.Thing"`) {
		t.Errorf("parent package errors = %v", l.errors)
	}
}

func TestResolveNoBareNameMatch(t *testing.T) {
	l := newJVMDotLang()
	index := resolverIndex(l, struct{ pkg, name string }{"lib/util", "Util"})
	cfg := resolverConfig(t, []rule.Directive{{Key: "resolve", Value: "dot dot Mapped //mapped:dep"}})
	r := rule.NewRule(dotKind, "app")
	l.Resolve(cfg, index, nil, r, WrapImportSet([]string{"com.x.Util", "com.x.Mapped"}), label.New("", "app", "app"))
	if r.Attr("deps") != nil {
		t.Errorf("bare names bound deps = %v", r.AttrStrings("deps"))
	}
	if len(l.errors) != 2 || !strings.Contains(l.errors[0], `"com.x.Util"`) || !strings.Contains(l.errors[1], `"com.x.Mapped"`) {
		t.Errorf("errors = %v", l.errors)
	}
}

func TestResolveBareNameIgnoreNoLongerMatches(t *testing.T) {
	l := newJVMDotLang()
	l.Before(context.Background())
	cfg := resolverConfig(t, nil)
	file, err := rule.LoadData("BUILD.bazel", "app", []byte("# gazelle:dx_ignore_import dot Dropped\n"))
	if err != nil {
		t.Fatal(err)
	}
	l.Configure(cfg, "app", file)
	l.generated.Add("app")
	l.Resolve(cfg, resolverIndex(l), nil, rule.NewRule(dotKind, "app"), WrapImportSet([]string{"x.y.Dropped"}), label.New("", "app", "app"))
	l.AfterResolvingDeps(context.Background())
	msg := strings.Join(Messages(Errors.Take()), "\n")
	if !strings.Contains(msg, `unresolved import "x.y.Dropped"`) || !strings.Contains(msg, "stale # gazelle:dx_ignore_import dot Dropped") {
		t.Errorf("errors = %q", msg)
	}
}

func TestJVMCandidates(t *testing.T) {
	for _, tc := range []struct {
		in   string
		want []string
	}{
		{"a.b.c.X", []string{"a.b.c"}},
		{"com.a.Outer.Inner", []string{"com.a.Outer", "com.a"}},
		{"com.x.Obj.member", []string{"com.x.Obj", "com.x"}},
		{"com.x.runBlocking", []string{"com.x"}},
		{"a.b.c.*", []string{"a.b.c"}},
		{"p.*", []string{"p"}},
		{"grades.helper", nil},
		{"Widget", nil},
		{".*", nil},
	} {
		if got := JVMCandidates(tc.in); !slices.Equal(got, tc.want) {
			t.Errorf("JVMCandidates(%q) = %v, want %v", tc.in, got, tc.want)
		}
	}
}

func TestJVMQualifiedIDs(t *testing.T) {
	root := t.TempDir()
	dotFile(t, root, "a/Foo.dot", "pkg com.a\n")
	dotFile(t, root, "a/Bare.dot", "\n")
	parse := func(content []byte) (string, error) {
		return strings.TrimSpace(strings.TrimPrefix(string(content), "pkg ")), nil
	}
	identity := func(name string) string { return strings.TrimSuffix(name, ".dot") }
	if got := JVMQualifiedIDs(filepath.Join(root, "a", "Foo.dot"), parse, identity); !slices.Equal(got, []string{"com.a", "com.a.Foo"}) {
		t.Errorf("packaged ids = %v", got)
	}
	if got := JVMQualifiedIDs(filepath.Join(root, "a", "Bare.dot"), parse, identity); got != nil {
		t.Errorf("unpackaged ids = %v", got)
	}
	if got := JVMQualifiedIDs(filepath.Join(root, "a", "Missing.dot"), parse, identity); got != nil {
		t.Errorf("missing ids = %v", got)
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

func TestResolveAmbiguousIncludingTheFromLabelIsSelf(t *testing.T) {
	l := newDotLang()
	index := resolverIndex(l,
		struct{ pkg, name string }{"app", "Dup"},
		struct{ pkg, name string }{"lib/two", "Dup"},
	)
	cfg := resolverConfig(t, nil)
	r := rule.NewRule(dotKind, "Dup")
	l.Resolve(cfg, index, nil, r, WrapImportSet([]string{"Dup"}), label.New("", "app", "Dup"))
	if r.Attr("deps") != nil || len(l.errors) != 0 {
		t.Errorf("self ambiguity = %v, errors = %v", r.AttrStrings("deps"), l.errors)
	}
}

func TestResolveSharedBasenamesStayIndependent(t *testing.T) {
	spec := dotSpec()
	spec.PathIdentity = true
	spec.Identity = func(name string) string { return strings.TrimSuffix(path.Base(name), ".dot") }
	l := NewSingleLang(spec)
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return l })
	for _, pkg := range []string{"a", "b"} {
		r := rule.NewRule(dotKind, "util")
		r.SetAttr("srcs", []string{"util.dot"})
		index.AddRule(config.New(), r, rule.EmptyFile(filepath.Join(pkg, "BUILD.bazel"), pkg))
	}
	index.Finish()
	r := rule.NewRule(dotKind, "app")
	l.Resolve(resolverConfig(t, nil), index, nil, r, WrapImportSet([]string{"a/util", "b/util"}), label.New("", "app", "app"))
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//a:util,//b:util" {
		t.Errorf("deps = %q, want //a:util,//b:util", got)
	}
	if len(l.errors) != 0 {
		t.Errorf("errors = %v", l.errors)
	}
}

func TestOptInFieldsSplitLoadsAndIndexExtraKinds(t *testing.T) {
	spec := dotSpec()
	spec.ExtraKinds = []string{"dot_extra", "dot_gpu"}
	spec.KindDefs = map[string]string{"dot_gpu": "gpu/rules:defs.bzl"}
	spec.IndexKinds = []string{"dot_gpu"}
	spec.IsIndexSource = func(name string) bool { return strings.HasSuffix(name, ".gdot") }
	spec.Directives = []string{"dot_custom"}
	var gotRel, gotValue string
	spec.ConfigureExt = func(_ *config.Config, rel string, f *rule.File, _ Reporter) {
		gotRel = rel
		for _, d := range f.Directives {
			if d.Key == "dot_custom" {
				gotValue = d.Value
			}
		}
	}
	l := NewSingleLang(spec)

	loads := l.Loads()
	if len(loads) != 2 || loads[0].Name != "@rules_dx//dot/rules:defs.bzl" || strings.Join(loads[0].Symbols, ",") != "dot_library,dot_extra" ||
		loads[1].Name != "@rules_dx//gpu/rules:defs.bzl" || strings.Join(loads[1].Symbols, ",") != "dot_gpu" {
		t.Errorf("loads = %+v", loads)
	}
	if got := strings.Join(l.KnownDirectives(), ","); got != "dx_ignore_import,dot_custom" {
		t.Errorf("directives = %q", got)
	}
	l.Configure(config.New(), "pkg", &rule.File{Directives: []rule.Directive{{Key: "dot_custom", Value: "v"}}})
	if gotRel != "pkg" || gotValue != "v" {
		t.Errorf("ConfigureExt saw rel=%q value=%q", gotRel, gotValue)
	}

	gpu := rule.NewRule("dot_gpu", "g")
	gpu.SetAttr("srcs", []string{"a.dot", "b.gdot"})
	if got := l.Imports(&config.Config{}, gpu, nil); len(got) != 1 || got[0].Imp != "b.gdot" {
		t.Errorf("extra kind imports = %+v, want only b.gdot", got)
	}
	extra := rule.NewRule("dot_extra", "e")
	extra.SetAttr("srcs", []string{"a.dot"})
	if got := l.Imports(&config.Config{}, extra, nil); got != nil {
		t.Errorf("unindexed kind imports = %+v", got)
	}
}

func TestGenerateStripsBOM(t *testing.T) {
	root := t.TempDir()
	dotFile(t, root, "pkg/demo/Demo.dot", "\xEF\xBB\xBFimport x")
	var seen []byte
	args := language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: []string{"Demo.dot"},
	}
	isSource := func(name string) bool { return HasExt(name, []string{".dot"}) }
	dirSpec := SingleDirSpec{
		LanguageName: dotLanguage,
		LibraryKind:  dotKind,
		IsSource:     isSource,
		DirName:      DirTargetName,
		DefinesMain:  func([]byte) bool { return false },
		ParseImports: func(content []byte) []string { seen = content; return nil },
		Wrap:         WrapImportSet,
	}
	GenerateSingleDir(args, map[string]rule.KindInfo{dotKind: LibraryKindInfo()}, dirSpec, NewSingleLang(dotSpec()))
	if string(seen) != "import x" {
		t.Errorf("GenerateSingleDir content = %q, want the BOM stripped", seen)
	}
	seen = nil
	fileSpec := SingleFileSpec{
		LanguageName: dotLanguage,
		LibraryKind:  dotKind,
		IsSource:     isSource,
		TargetName:   func(name string) (string, error) { return strings.TrimSuffix(name, ".dot"), nil },
		ParseImports: func(content []byte, _ string) []string { seen = content; return nil },
		Wrap:         WrapImportSet,
	}
	GenerateSingleFile(args, map[string]rule.KindInfo{dotKind: LibraryKindInfo()}, fileSpec, NewSingleLang(dotSpec()))
	if string(seen) != "import x" {
		t.Errorf("GenerateSingleFile content = %q, want the BOM stripped", seen)
	}
}

func TestGenerateRootHint(t *testing.T) {
	root := t.TempDir()
	dotFile(t, root, "Root.dot", "nothing\n")
	dotFile(t, root, "pkg/x/X.dot", "nothing\n")
	cases := []struct {
		hint, rel, want string
	}{
		{"from-manifest", "", "from_manifest"},
		{"---", "", "demo_repo"},
		{"", "", "demo_repo"},
	}
	for _, tc := range cases {
		spec := SingleDirSpec{
			LanguageName: dotLanguage,
			LibraryKind:  dotKind,
			IsSource:     func(name string) bool { return HasExt(name, []string{".dot"}) },
			DirName:      DirTargetName,
			RootHint:     func(language.GenerateArgs) string { return tc.hint },
			DefinesMain:  func([]byte) bool { return false },
			ParseImports: func([]byte) []string { return nil },
			Wrap:         WrapImportSet,
		}
		result := GenerateSingleDir(language.GenerateArgs{
			Config:       &config.Config{RepoRoot: root, RepoName: "demo-repo"},
			Dir:          root,
			RegularFiles: []string{"Root.dot"},
		}, map[string]rule.KindInfo{dotKind: LibraryKindInfo()}, spec, NewSingleLang(dotSpec()))
		if len(result.Gen) != 1 || result.Gen[0].Name() != tc.want {
			t.Fatalf("hint %q: generated %+v, want %s", tc.hint, result.Gen, tc.want)
		}
	}
	spec := SingleDirSpec{
		LanguageName: dotLanguage,
		LibraryKind:  dotKind,
		IsSource:     func(name string) bool { return HasExt(name, []string{".dot"}) },
		DirName:      DirTargetName,
		RootHint:     func(language.GenerateArgs) string { t.Fatal("RootHint called for a subpackage"); return "" },
		DefinesMain:  func([]byte) bool { return false },
		ParseImports: func([]byte) []string { return nil },
		Wrap:         WrapImportSet,
	}
	result := GenerateSingleDir(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "x"),
		Rel:          "pkg/x",
		RegularFiles: []string{"X.dot"},
	}, map[string]rule.KindInfo{dotKind: LibraryKindInfo()}, spec, NewSingleLang(dotSpec()))
	if len(result.Gen) != 1 || result.Gen[0].Name() != "x" {
		t.Fatalf("subpackage: generated %+v, want x", result.Gen)
	}
}

func resolveConfig(t *testing.T, root, directives string) *config.Config {
	t.Helper()
	cfg := config.New()
	cfg.RepoRoot = root
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	f, err := rule.LoadData("BUILD.bazel", "", []byte(directives))
	if err != nil {
		t.Fatal(err)
	}
	resolver.Configure(cfg, "pkg/demo", f)
	return cfg
}

func TestMappedByOverride(t *testing.T) {
	cfg := resolveConfig(t, t.TempDir(), "# gazelle:resolve dot dot.Mapped //third_party:mapped\n# gazelle:resolve dot dot.pkg //third_party:pkg\n")
	candidates := JVMCandidates
	for _, tc := range []struct {
		name string
		cfg  *config.Config
		cand func(string) []string
		want bool
	}{
		{"dot.Mapped", cfg, nil, true},
		{"dot.Other", cfg, nil, false},
		{"dot.pkg.Bare", cfg, nil, false},
		{"dot.pkg.Bare", cfg, candidates, true},
		{"dot.pkg.sub.Bare", cfg, candidates, false},
		{"dot.Mapped", &config.Config{}, nil, false},
		{"dot.Mapped", nil, nil, false},
	} {
		if got := mappedByOverride(tc.cfg, dotLanguage, tc.cand, tc.name); got != tc.want {
			t.Errorf("mappedByOverride(%q) = %v, want %v", tc.name, got, tc.want)
		}
	}
}

func TestGeneratorsKeepMappedStdLibRoots(t *testing.T) {
	root := t.TempDir()
	dotFile(t, root, "pkg/demo/Demo.dot", "x\n")
	parse := func([]byte) []string { return []string{"dot.Mapped", "dot.Plain"} }
	for _, tc := range []struct {
		name       string
		directives string
		want       []string
	}{
		{"mapped", "# gazelle:resolve dot dot.Mapped //third_party:mapped\n", []string{"dot.Mapped"}},
		{"unmapped", "", nil},
	} {
		cfg := resolveConfig(t, root, tc.directives)
		args := language.GenerateArgs{Config: cfg, Dir: filepath.Join(root, "pkg", "demo"), Rel: "pkg/demo", RegularFiles: []string{"Demo.dot"}}
		single := GenerateSingleDir(args, map[string]rule.KindInfo{dotKind: LibraryKindInfo()}, SingleDirSpec{
			LanguageName: dotLanguage, LibraryKind: dotKind, IsSource: isDot2,
			DirName: DirTargetName, DefinesMain: func([]byte) bool { return false },
			ParseImports: parse, IsStdLib: isDotStdLib, Wrap: WrapImportSet,
		}, newDotLang())
		file := GenerateSingleFile(args, map[string]rule.KindInfo{dotKind: LibraryKindInfo()}, SingleFileSpec{
			LanguageName: dotLanguage, LibraryKind: dotKind, IsSource: isDot2,
			TargetName:   func(name string) (string, error) { return strings.TrimSuffix(name, ".dot"), nil },
			ParseImports: func(c []byte, _ string) []string { return parse(c) }, IsStdLib: isDotStdLib, Wrap: WrapImportSet,
		}, newDotLang())
		for kind, res := range map[string]language.GenerateResult{"dir": single, "file": file} {
			if len(res.Imports) != 1 {
				t.Fatalf("%s/%s: imports = %v", tc.name, kind, res.Imports)
			}
			got, _ := UnwrapImportSet(res.Imports[0])
			if strings.Join(got, ",") != strings.Join(tc.want, ",") {
				t.Errorf("%s/%s: imports = %v, want %v", tc.name, kind, got, tc.want)
			}
		}
	}
}

func isDot2(name string) bool { return HasExt(name, []string{".dot"}) }

func TestResolveSingleLetsAMappingBeatStdLib(t *testing.T) {
	for _, tc := range []struct {
		name       string
		directives string
		want       []string
	}{
		{"mapped", "# gazelle:resolve dot dot.Mapped //third_party:mapped\n", []string{"//third_party:mapped"}},
		{"unmapped", "", nil},
	} {
		cfg := resolveConfig(t, t.TempDir(), tc.directives)
		l := newDotLang()
		l.Before(context.Background())
		index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return nil })
		r := rule.NewRule(dotKind, "app")
		l.Resolve(cfg, index, nil, r, WrapImportSet([]string{"dot.Mapped"}), label.New("", "pkg", "app"))
		if l.Failed() {
			t.Errorf("%s: reported %v", tc.name, l.errors)
		}
		got := r.AttrStrings("deps")
		if strings.Join(got, ",") != strings.Join(tc.want, ",") {
			t.Errorf("%s: deps = %v, want %v", tc.name, got, tc.want)
		}
	}
}

func TestMergeStaleOwned(t *testing.T) {
	file, err := rule.LoadData("pkg/BUILD.bazel", "pkg", []byte(`dot_library(name = "kept")

dot_library(name = "stale")

dot_library(name = "handwritten", srcs = ["tool.dot"])

filegroup(name = "other")
`))
	if err != nil {
		t.Fatal(err)
	}
	kinds := map[string]rule.KindInfo{dotKind: LibraryKindInfo()}
	gen := language.GenerateResult{Gen: []*rule.Rule{rule.NewRule(dotKind, "kept")}}
	empties := func(result language.GenerateResult) string {
		var names []string
		for _, r := range result.Empty {
			names = append(names, r.Kind()+":"+r.Name())
		}
		return strings.Join(names, ",")
	}
	owns := func(r *rule.Rule) bool { return len(r.AttrStrings("srcs")) == 0 }
	if got := empties(MergeStaleOwned(file, gen, kinds, owns)); got != "dot_library:stale" {
		t.Errorf("MergeStaleOwned empties = %q, want only the owned stale rule", got)
	}
	want := "dot_library:stale,dot_library:handwritten"
	if got := empties(MergeStaleOwned(file, gen, kinds, nil)); got != want {
		t.Errorf("MergeStaleOwned(nil) empties = %q, want %q", got, want)
	}
	if got := empties(MergeStale(file, gen, kinds)); got != want {
		t.Errorf("MergeStale empties = %q, want %q", got, want)
	}
}
