package julia

import (
	"context"
	"os"
	"path/filepath"
	"strings"
	"testing"

	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazel-contrib/bazel-gazelle/v2/label"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazelbuild/bazel-gazelle/resolve"
)

func writeTree(t *testing.T, dir string, files map[string]string) []string {
	t.Helper()
	var regular []string
	for name, content := range files {
		p := filepath.Join(dir, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(p), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(p, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
		if !strings.Contains(name, "/") {
			regular = append(regular, name)
		}
	}
	return regular
}

func generate(t *testing.T, lang language.Language, rel string, file *rule.File, files map[string]string) language.GenerateResult {
	t.Helper()
	root := t.TempDir()
	dir := filepath.Join(root, filepath.FromSlash(rel))
	regular := writeTree(t, dir, files)
	return lang.GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          dir,
		Rel:          rel,
		File:         file,
		RegularFiles: regular,
	})
}

func buildFile(t *testing.T, content string) *rule.File {
	t.Helper()
	f, err := rule.LoadData("pkg/BUILD.bazel", "pkg", []byte(content))
	if err != nil {
		t.Fatal(err)
	}
	return f
}

func errorsText(lang language.Language) string {
	lang.(*juliaLang).AfterResolvingDeps(context.Background())
	return strings.Join(common.Messages(common.Errors.Take()), "\n")
}

func project(name string, deps ...string) string {
	out := "name = \"" + name + "\"\nuuid = \"" + testUUID + "\"\n"
	if len(deps) > 0 {
		out += "[deps]\n"
		for i := 0; i < len(deps); i += 2 {
			out += deps[i] + " = \"" + deps[i+1] + "\"\n"
		}
	}
	return out
}

func summary(result language.GenerateResult) string {
	var out []string
	for _, r := range result.Gen {
		out = append(out, r.Kind()+":"+r.Name()+":"+strings.Join(r.AttrStrings("srcs"), "+"))
	}
	return strings.Join(out, ",")
}

func TestGenerateLibraryAndTest(t *testing.T) {
	lang := NewLanguage()
	result := generate(t, lang, "pkg", nil, map[string]string{
		"Project.toml":     project("Demo", "Foo", testUUID) + "[extras]\nTest = \"" + testStd + "\"\n[targets]\ntest = [\"Test\"]\n",
		"src/Demo.jl":      "module Demo\nend\n",
		"src/sub/x.jl":     "",
		"test/runtests.jl": "",
		"test/b.jl":        "",
	})
	if lang.(*juliaLang).Failed() {
		t.Fatal(errorsText(lang))
	}
	want := "julia_library:Demo:src/Demo.jl+src/sub/x.jl,julia_test:Demo_test:test/b.jl+test/runtests.jl"
	if got := summary(result); got != want {
		t.Fatalf("rules = %s, want %s", got, want)
	}
	if got := strings.Join(result.Gen[0].AttrStrings("data"), ","); got != "Project.toml" {
		t.Errorf("data = %q", got)
	}
	if got := result.Gen[1].AttrString("main"); got != TestEntry {
		t.Errorf("main = %q, want %s", got, TestEntry)
	}
	if got := strings.Join(result.Imports[0].(common.ImportSet).Names, ","); got != "Foo" {
		t.Errorf("library imports = %s", got)
	}
	if got := strings.Join(result.Imports[1].(common.ImportSet).Names, ","); got != "Demo,Test" {
		t.Errorf("test imports = %s", got)
	}
}

func TestGenerateSingleTestHasNoMain(t *testing.T) {
	result := generate(t, NewLanguage(), "pkg", nil, map[string]string{
		"Project.toml":     project("Demo"),
		"src/Demo.jl":      "",
		"test/runtests.jl": "",
	})
	if len(result.Gen) != 2 || result.Gen[1].Attr("main") != nil {
		t.Fatalf("rules = %s, want a test without main", summary(result))
	}
}

func TestGenerateQuietNoOps(t *testing.T) {
	for name, tree := range map[string]map[string]string{
		"environment": {"Project.toml": "[deps]\nFoo = \"" + testUUID + "\"\n", "src/Demo.jl": ""},
		"no src":      {"Project.toml": project("Demo")},
		"no project":  {"src/Demo.jl": ""},
	} {
		lang := NewLanguage()
		if result := generate(t, lang, "pkg", nil, tree); len(result.Gen) != 0 || lang.(*juliaLang).Failed() {
			t.Errorf("%s: gen = %s, errors = %q, want a quiet no-op", name, summary(result), errorsText(lang))
		}
	}
}

func TestGenerateTestonlyFixture(t *testing.T) {
	result := generate(t, NewLanguage(), "tests/fixtures/demo", nil, map[string]string{"Project.toml": project("Demo"), "src/Demo.jl": ""})
	if len(result.Gen) != 1 || result.Gen[0].AttrBool("testonly") != true {
		t.Fatalf("fixture library = %s, want testonly", summary(result))
	}
}

func TestGenerateErrors(t *testing.T) {
	cases := []struct {
		name string
		tree map[string]string
		file string
		want string
	}{
		{"invalid toml", map[string]string{"Project.toml": "name = \n"}, "", "pkg/Project.toml: invalid TOML"},
		{"bad identifier", map[string]string{"Project.toml": "name = \"a-b\"\n"}, "", "not a Julia identifier"},
		{"missing entry", map[string]string{"Project.toml": project("Demo"), "src/Other.jl": ""}, "", "no src/Demo.jl"},
		{"nested build", map[string]string{"Project.toml": project("Demo"), "src/Demo.jl": "", "src/sub/BUILD.bazel": "", "src/sub/x.jl": ""}, "", "src/sub has its own BUILD.bazel"},
		{"foreign stdlib uuid", map[string]string{"Project.toml": project("Demo", "Test", testUUID), "src/Demo.jl": ""}, "", "standard library Test has"},
		{"bad test target", map[string]string{"Project.toml": project("Demo") + "[targets]\ntest = [\"Test\"]\n", "src/Demo.jl": ""}, "", "not in [extras]"},
		{"bad test project", map[string]string{"Project.toml": project("Demo"), "src/Demo.jl": "", "test/runtests.jl": "", "test/Project.toml": "[deps]\nX = 1\n"}, "", "test/Project.toml: [deps] X"},
		{"claim conflict", map[string]string{"Project.toml": project("Demo"), "src/Demo.jl": ""}, `filegroup(name = "Demo")`, "target name"},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			var file *rule.File
			if tc.file != "" {
				file = buildFile(t, tc.file+"\n")
			}
			lang := NewLanguage()
			result := generate(t, lang, "pkg", file, tc.tree)
			if len(result.Gen) != 0 {
				t.Errorf("generated %s, want none", summary(result))
			}
			if msg := errorsText(lang); !strings.Contains(msg, tc.want) || !strings.HasPrefix(msg, "julia: pkg: ") && !strings.Contains(msg, "julia: pkg: ") {
				t.Errorf("errors = %q, want %q", msg, tc.want)
			}
		})
	}
}

func TestGenerateSkipsNestedPackage(t *testing.T) {
	result := generate(t, NewLanguage(), "pkg", nil, map[string]string{
		"Project.toml":          project("Demo"),
		"src/Demo.jl":           "",
		"src/inner/Project.toml": project("Inner"),
		"src/inner/src/Inner.jl": "",
		"src/inner/BUILD.bazel":  "",
	})
	if got := summary(result); got != "julia_library:Demo:src/Demo.jl" {
		t.Fatalf("rules = %s, want the nested package left out", got)
	}
}

func TestGenerateRemovesOnlyOwnedRules(t *testing.T) {
	file := buildFile(t, `julia_library(name = "Old", srcs = ["src/Old.jl"])

julia_library(name = "Gen", srcs = [":gen"])

julia_test(name = "old_test", srcs = ["test/old.jl"])

julia_binary(name = "bin", srcs = ["src/Demo.jl"])
`)
	result := generate(t, NewLanguage(), "pkg", file, map[string]string{"Project.toml": project("Demo"), "src/Demo.jl": ""})
	var empty []string
	for _, r := range result.Empty {
		empty = append(empty, r.Kind()+":"+r.Name())
	}
	if got := strings.Join(empty, ","); got != "julia_library:Old,julia_test:old_test" {
		t.Errorf("removed = %s", got)
	}
}

func TestImportsIndexByRuleName(t *testing.T) {
	lib := rule.NewRule(LibraryKind, "Demo")
	lib.SetAttr("srcs", []string{"src/Demo.jl"})
	specs := NewLanguage().Imports(nil, lib, nil)
	if len(specs) != 1 || specs[0].Imp != "Demo" || specs[0].Lang != "julia" {
		t.Errorf("imports = %v, want Demo", specs)
	}
	if specs := NewLanguage().Imports(nil, rule.NewRule(TestKind, "Demo_test"), nil); len(specs) != 0 {
		t.Errorf("test imports = %v, want none", specs)
	}
}

func resolveDeps(t *testing.T, directives string, libs []string, imports []string) (*rule.Rule, string) {
	t.Helper()
	lang := NewLanguage().(*juliaLang)
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
	resolver.Configure(cfg, "", f)
	lang.Configure(cfg, "", f)
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return lang })
	for _, name := range libs {
		r := rule.NewRule(LibraryKind, name)
		index.AddRule(cfg, r, rule.EmptyFile("lib/BUILD.bazel", "lib"))
	}
	index.Finish()
	test := rule.NewRule(TestKind, "app_test")
	lang.Resolve(cfg, index, nil, test, common.WrapImportSet(imports), label.New("", "app", "app_test"))
	return test, errorsText(lang)
}

func TestResolveOrder(t *testing.T) {
	test, msg := resolveDeps(t, "# gazelle:resolve julia Example @julia_deps//:Example\n# gazelle:resolve julia Test @julia_deps//:Test\n", []string{"Stats"}, []string{"Example", "Stats", "LinearAlgebra", "Test"})
	if got := strings.Join(test.AttrStrings("deps"), ","); got != "//lib:Stats,@julia_deps//:Example,@julia_deps//:Test" || msg != "" {
		t.Fatalf("deps = %s (errors %q)", got, msg)
	}
}

func TestResolveUnresolved(t *testing.T) {
	_, msg := resolveDeps(t, "", nil, []string{"Missing"})
	if !strings.Contains(msg, `unresolved import "Missing"`) || !strings.Contains(msg, "# gazelle:resolve julia <Name> <label>") {
		t.Errorf("errors = %q", msg)
	}
}
