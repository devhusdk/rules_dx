package common

import (
	"context"
	"errors"
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
	clsLanguage = "cls"
	clsKind     = "cls_library"
)

func clsParsePackage(content []byte) (string, error) {
	for _, line := range strings.Split(string(content), "\n") {
		if pkg, ok := strings.CutPrefix(line, "package "); ok {
			return strings.TrimSpace(pkg), nil
		}
	}
	return "", nil
}

func clsParseImports(content []byte) []string {
	var out []string
	for _, line := range strings.Split(string(content), "\n") {
		if imp, ok := strings.CutPrefix(line, "import "); ok {
			out = append(out, strings.TrimSpace(imp))
		}
	}
	return out
}

func clsSpec() ClassLangSpec {
	return ClassLangSpec{
		LanguageName:   clsLanguage,
		LibraryKind:    clsKind,
		MainRule:       "cls_binary",
		DefsPackage:    "cls/rules:defs.bzl",
		UnresolvedHint: "add a local cls_library",
		Exts:           []string{".cls"},
		TestSuffixes:   []string{"Test"},
		ClassIDs:       true,
		PackageIDs:     true,
		ParsePackage:   clsParsePackage,
		DefinesMain:    func(content []byte) bool { return strings.Contains(string(content), "main()") },
		ParseImports:   clsParseImports,
		IsStdLib:       func(name string) bool { return strings.HasPrefix(name, "std.") },
	}
}

func clsGenerate(t *testing.T, lang *SingleLang, files map[string]string) language.GenerateResult {
	t.Helper()
	root := t.TempDir()
	var regular []string
	for name, content := range files {
		dotFile(t, root, "pkg/demo/"+name, content)
		regular = append(regular, name)
	}
	slices.Sort(regular)
	return lang.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, "pkg", "demo"),
		Rel:          "pkg/demo",
		RegularFiles: regular,
	})
}

func clsImports(t *testing.T, lang *SingleLang, files map[string]string) []string {
	t.Helper()
	root := t.TempDir()
	r := rule.NewRule(clsKind, "demo")
	var srcs []string
	for name, content := range files {
		dotFile(t, root, "pkg/demo/"+name, content)
		srcs = append(srcs, name)
	}
	slices.Sort(srcs)
	r.SetAttr("srcs", srcs)
	var got []string
	for _, spec := range lang.Imports(config.New(), r, &rule.File{Path: filepath.Join(root, "pkg", "demo", "BUILD.bazel"), Pkg: "pkg/demo"}) {
		if spec.Lang != clsLanguage {
			t.Fatalf("import lang = %q", spec.Lang)
		}
		got = append(got, spec.Imp)
	}
	return got
}

func TestClassLangMetadata(t *testing.T) {
	l := NewClassLang(clsSpec())
	if l.Name() != clsLanguage {
		t.Fatalf("name = %q", l.Name())
	}
	if _, ok := l.Kinds()[clsKind]; !ok || len(l.Kinds()) != 1 {
		t.Fatalf("kinds = %v", l.Kinds())
	}
	loads := l.Loads()
	if len(loads) != 1 || loads[0].Name != "@rules_dx//cls/rules:defs.bzl" || !reflect.DeepEqual(loads[0].Symbols, []string{clsKind}) {
		t.Fatalf("loads = %+v", loads)
	}
}

func TestClassLangGenerateSkipsTestSources(t *testing.T) {
	l := NewClassLang(clsSpec())
	l.Before(context.Background())
	files := map[string]string{
		"Demo.cls":     "package demo\nimport other.Thing\nimport std.io\n",
		"DemoTest.cls": "package demo\nimport test.Only\n",
		"notes.txt":    "",
	}
	result := clsGenerate(t, l, files)
	if l.Failed() || len(result.Gen) != 1 {
		t.Fatalf("failed = %v gen = %d", l.Failed(), len(result.Gen))
	}
	if got := result.Gen[0].AttrStrings("srcs"); !reflect.DeepEqual(got, []string{"Demo.cls"}) {
		t.Fatalf("srcs = %v", got)
	}
	if result.Gen[0].Kind() != clsKind || result.Gen[0].Name() != "demo" {
		t.Fatalf("rule = %s(%s)", result.Gen[0].Kind(), result.Gen[0].Name())
	}
	if got, _ := UnwrapImportSet(result.Imports[0]); !reflect.DeepEqual(got, []string{"other.Thing"}) {
		t.Fatalf("imports = %v", got)
	}
	if got := clsImports(t, l, files); !reflect.DeepEqual(got, []string{"Demo", "demo", "demo.Demo"}) {
		t.Fatalf("index = %v", got)
	}
}

func TestClassLangIDFlags(t *testing.T) {
	files := map[string]string{"Demo.cls": "package demo\n"}
	for _, tc := range []struct {
		class, pkg bool
		want       []string
	}{
		{true, true, []string{"Demo", "demo", "demo.Demo"}},
		{true, false, []string{"Demo"}},
		{false, true, []string{"demo", "demo.Demo"}},
		{false, false, nil},
	} {
		spec := clsSpec()
		spec.ClassIDs, spec.PackageIDs = tc.class, tc.pkg
		if got := clsImports(t, NewClassLang(spec), files); !reflect.DeepEqual(got, tc.want) {
			t.Errorf("class=%v package=%v: index = %v, want %v", tc.class, tc.pkg, got, tc.want)
		}
	}
}

func TestClassLangMainRefusal(t *testing.T) {
	l := NewClassLang(clsSpec())
	l.Before(context.Background())
	result := clsGenerate(t, l, map[string]string{"Main.cls": "package demo\nmain()\n"})
	if len(result.Gen) != 0 || !l.Failed() {
		t.Fatalf("gen = %d failed = %v", len(result.Gen), l.Failed())
	}
	l.AfterResolvingDeps(context.Background())
	msgs := Messages(Errors.Take())
	if len(msgs) != 1 || !strings.Contains(msgs[0], "cls: pkg/demo: Main.cls defines main; thin cls_binary entries stay handwritten, so split main-bearing sources") {
		t.Fatalf("errors = %v", msgs)
	}
}

func TestClassLangOrderAndMakeRule(t *testing.T) {
	files := map[string]string{"A.cls": "package demo\n", "B.cls": "package demo\n"}
	spec := clsSpec()
	spec.OrderSrcs = func(_ string, sources []string, _ map[string][]byte) ([]string, error) {
		out := slices.Clone(sources)
		slices.Reverse(out)
		return out, nil
	}
	spec.MakeRule = func(name string, sources []string, _ []string) *rule.Rule {
		r := rule.NewRule(clsKind, name)
		r.SetAttr("srcs", rule.UnsortedStrings(sources))
		r.SetAttr("tags", []string{"custom"})
		return r
	}
	l := NewClassLang(spec)
	l.Before(context.Background())
	result := clsGenerate(t, l, files)
	if len(result.Gen) != 1 || !reflect.DeepEqual(result.Gen[0].AttrStrings("srcs"), []string{"B.cls", "A.cls"}) || result.Gen[0].Attr("tags") == nil {
		t.Fatalf("custom rule = %+v", result.Gen)
	}
	spec.OrderSrcs = func(string, []string, map[string][]byte) ([]string, error) { return nil, errors.New("no order") }
	l = NewClassLang(spec)
	l.Before(context.Background())
	if result := clsGenerate(t, l, files); len(result.Gen) != 0 || !l.Failed() {
		t.Fatalf("order error ignored: gen = %d", len(result.Gen))
	}
	Errors.Reset()
	l = NewClassLang(clsSpec())
	l.Before(context.Background())
	result = clsGenerate(t, l, files)
	if len(result.Gen) != 1 || !reflect.DeepEqual(result.Gen[0].AttrStrings("srcs"), []string{"A.cls", "B.cls"}) || result.Gen[0].Attr("tags") != nil {
		t.Fatalf("default rule = %+v", result.Gen)
	}
}

func TestClassLangCandidates(t *testing.T) {
	spec := clsSpec()
	spec.Candidates = func(name string) []string {
		if i := strings.LastIndexByte(name, '.'); i >= 0 {
			return []string{name[:i]}
		}
		return nil
	}
	l := NewClassLang(spec)
	l.Before(context.Background())
	index := resolve.NewRuleIndex(func(r *rule.Rule, _ string) resolve.Resolver {
		if r.Kind() == clsKind {
			return l
		}
		return nil
	})
	root := t.TempDir()
	dotFile(t, root, "lib/Thing.cls", "package lib\n")
	lib := rule.NewRule(clsKind, "lib")
	lib.SetAttr("srcs", []string{"Thing.cls"})
	index.AddRule(config.New(), lib, &rule.File{Path: filepath.Join(root, "lib", "BUILD.bazel"), Pkg: "lib"})
	index.Finish()
	cfg := config.New()
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	r := rule.NewRule(clsKind, "app")
	l.Resolve(cfg, index, nil, r, WrapImportSet([]string{"lib.Missing"}), label.New("", "app", "app"))
	if l.Failed() || !reflect.DeepEqual(r.AttrStrings("deps"), []string{"//lib"}) {
		t.Fatalf("deps = %v failed = %v", r.AttrStrings("deps"), l.Failed())
	}
}

func TestClassLangSpecRoundTrip(t *testing.T) {
	files := map[string]string{"Demo.cls": "package demo\nimport other.Thing\n"}
	direct := NewClassLang(clsSpec())
	viaSpec := NewSingleLang(clsSpec().Spec())
	if !reflect.DeepEqual(clsImports(t, direct, files), clsImports(t, viaSpec, files)) {
		t.Fatal("Spec() index differs from NewClassLang")
	}
	direct.Before(context.Background())
	a := clsGenerate(t, direct, files)
	viaSpec.Before(context.Background())
	b := clsGenerate(t, viaSpec, files)
	if len(a.Gen) != 1 || len(b.Gen) != 1 || !reflect.DeepEqual(a.Gen[0].AttrStrings("srcs"), b.Gen[0].AttrStrings("srcs")) || !reflect.DeepEqual(a.Imports, b.Imports) {
		t.Fatalf("Spec() generation differs: %+v vs %+v", a, b)
	}
}
