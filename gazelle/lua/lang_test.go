package lua

import (
	"context"
	"os"
	"path/filepath"
	"reflect"
	"strings"
	"testing"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

func generate(t *testing.T, rel, build string, files map[string]string, regular []string) language.GenerateResult {
	t.Helper()
	root := t.TempDir()
	for name, content := range files {
		file := filepath.Join(root, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(file), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(file, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	var f *rule.File
	if build != "" {
		var err error
		if f, err = rule.LoadData(filepath.Join(root, rel, "BUILD.bazel"), rel, []byte(build)); err != nil {
			t.Fatal(err)
		}
	}
	return NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, filepath.FromSlash(rel)),
		Rel:          rel,
		File:         f,
		RegularFiles: regular,
	})
}

func TestGenerateLibraryAndTests(t *testing.T) {
	result := generate(t, "pkg/demo", "", map[string]string{
		"pkg/demo/init.lua":     "require(\"demo.sub\")\n",
		"pkg/demo/sub.lua":      "local s = require(\"string\")\nrequire(\"inspect\")\n",
		"pkg/demo/a.b.lua":      "return {}\n",
		"pkg/demo/sub_test.lua": "require(\"demo.sub\")\n",
	}, []string{"init.lua", "sub.lua", "a.b.lua", "sub_test.lua", "notes.txt"})
	if len(result.Gen) != 2 || len(result.Imports) != 2 {
		t.Fatalf("generated %d rules and %d import sets, want 2 each", len(result.Gen), len(result.Imports))
	}
	lib, test := result.Gen[0], result.Gen[1]
	if lib.Kind() != LibraryKind || lib.Name() != "demo" {
		t.Errorf("library = %s(%s)", lib.Kind(), lib.Name())
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "init.lua,sub.lua" {
		t.Errorf("library srcs = %q, want the dotted stem and the test left out", got)
	}
	if test.Kind() != TestKind || test.Name() != "sub_test" || test.AttrString("main") != "sub_test.lua" {
		t.Errorf("test = %s(%s) main %q", test.Kind(), test.Name(), test.AttrString("main"))
	}
	if got := result.Imports[0].(common.ImportSet).Names; !reflect.DeepEqual(got, []string{"demo.sub", "inspect", "string"}) {
		t.Errorf("library imports = %v", got)
	}
	if lib.AttrString("strip_import_prefix") != "" {
		t.Error("generation must never emit strip_import_prefix")
	}
}

func TestGenerateLeavesBinarySourcesOut(t *testing.T) {
	build := `lua_binary(
    name = "app_bin",
    srcs = ["main.lua", "extra.lua"],
    main = "main.lua",
)
`
	result := generate(t, "app", build, map[string]string{
		"app/hello.lua": "return {}\n",
		"app/main.lua":  "require(\"hello\")\n",
		"app/extra.lua": "return {}\n",
	}, []string{"hello.lua", "main.lua", "extra.lua"})
	if len(result.Gen) != 1 || strings.Join(result.Gen[0].AttrStrings("srcs"), ",") != "hello.lua" {
		t.Fatalf("generated %v, want one library over hello.lua", result.Gen)
	}
	if len(result.Empty) != 0 {
		t.Errorf("empty = %v, want the binary kept", result.Empty)
	}
}

func TestGenerateFixtureIsTestonly(t *testing.T) {
	result := generate(t, "x/testdata/f", "", map[string]string{"x/testdata/f/a.lua": "return 1\n"}, []string{"a.lua"})
	if len(result.Gen) != 1 || result.Gen[0].AttrBool("testonly") != true {
		t.Errorf("fixture library not testonly: %v", result.Gen)
	}
}

func TestImportsHonourPrefixes(t *testing.T) {
	lib := rule.NewRule(LibraryKind, "demo")
	lib.SetAttr("srcs", []string{"src/x/init.lua", "src/y.lua", "src/a.b.lua", "other.txt"})
	f := &rule.File{Pkg: "pkg"}
	names := func(r *rule.Rule) []string {
		var got []string
		for _, spec := range NewLanguage().Imports(&config.Config{}, r, f) {
			got = append(got, spec.Imp)
		}
		return got
	}
	if got := names(lib); !reflect.DeepEqual(got, []string{"pkg.src.x", "pkg.src.y"}) {
		t.Errorf("unprefixed imports = %v", got)
	}
	lib.SetAttr("strip_import_prefix", "src")
	lib.SetAttr("import_prefix", "vendor")
	if got := names(lib); !reflect.DeepEqual(got, []string{"vendor.x", "vendor.y"}) {
		t.Errorf("prefixed imports = %v", got)
	}
	if specs := NewLanguage().Imports(&config.Config{}, rule.NewRule(TestKind, "x"), f); specs != nil {
		t.Errorf("test imports = %v, want nil", specs)
	}
}

func resolveImports(t *testing.T, index *resolve.RuleIndex, names []string) (*luaLang, *rule.Rule) {
	t.Helper()
	lang := NewLanguage().(*luaLang)
	lang.Before(context.Background())
	cfg := config.New()
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	resolver.Configure(cfg, "app", &rule.File{})
	if index == nil {
		index = resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return nil })
	}
	r := rule.NewRule(LibraryKind, "app")
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet(names), label.New("", "app", "app"))
	return lang, r
}

func TestResolveSkipsStdlib(t *testing.T) {
	lang, r := resolveImports(t, nil, []string{"string", "table", "runfiles"})
	if lang.Failed() || len(r.AttrStrings("deps")) != 0 {
		t.Errorf("stdlib reported or added deps: %v %v", lang.Errors(), r.AttrStrings("deps"))
	}
}

func TestResolveReportsUnknownWithHint(t *testing.T) {
	lang, _ := resolveImports(t, nil, []string{"inspect"})
	errs := lang.Errors()
	if len(errs) != 1 || !strings.Contains(errs[0], "inspect") || !strings.Contains(errs[0], "gazelle:resolve") {
		t.Errorf("errors = %v, want one unresolved inspect with a resolve hint", errs)
	}
}

func TestResolveSelfImportAddsNoDep(t *testing.T) {
	lang := NewLanguage()
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return lang })
	self := rule.NewRule(LibraryKind, "app")
	self.SetAttr("srcs", []string{"parse.lua"})
	index.AddRule(&config.Config{}, self, &rule.File{Pkg: "app"})
	index.Finish()
	l, r := resolveImports(t, index, []string{"app.parse"})
	if l.Failed() || len(r.AttrStrings("deps")) != 0 {
		t.Errorf("self import: errors %v deps %v", l.Errors(), r.AttrStrings("deps"))
	}
}
