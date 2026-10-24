package zig

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

func generate(t *testing.T, lang language.Language, files map[string]string) language.GenerateResult {
	t.Helper()
	root := t.TempDir()
	dir := filepath.Join(root, "pkg", "greet")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	var regular []string
	for name, content := range files {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
		regular = append(regular, name)
	}
	return lang.GenerateRules(language.GenerateArgs{
		Config:       generateConfig(t, root),
		Dir:          dir,
		Rel:          "pkg/greet",
		RegularFiles: regular,
	})
}

func failed(lang language.Language) bool { return lang.(*zigLang).Failed() }

func TestGenerateLibraryWithRootAndSources(t *testing.T) {
	lang := NewLanguage()
	result := generate(t, lang, map[string]string{
		"greet.zig":      "const util = @import(\"util.zig\");\nconst fmt = @import(\"fmt_lib\");\n",
		"util.zig":       "const std = @import(\"std\");\n",
		"greet_test.zig": "const t = @import(\"only_by_test\");\n",
		"build.zig":      "const b = @import(\"never_parsed\");\n",
		"notes.txt":      "not a source\n",
	})
	if failed(lang) || len(result.Gen) != 1 {
		t.Fatalf("generated %d rules (failed=%v), want 1", len(result.Gen), failed(lang))
	}
	lib := result.Gen[0]
	if lib.Kind() != LibraryKind || lib.Name() != "greet" || lib.AttrString("main") != "greet.zig" {
		t.Fatalf("library = %s(%s, main=%s)", lib.Kind(), lib.Name(), lib.AttrString("main"))
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "util.zig" {
		t.Errorf("srcs = %q, want util.zig", got)
	}
	if got := result.Imports[0].(common.ImportSet).Names; strings.Join(got, ",") != "fmt_lib" {
		t.Errorf("imports = %v, want [fmt_lib]", got)
	}
}

func TestGenerateMainFails(t *testing.T) {
	lang := NewLanguage()
	result := generate(t, lang, map[string]string{
		"greet.zig": "pub fn main() void {}\n",
	})
	if len(result.Gen) != 0 || !failed(lang) {
		t.Fatalf("generated %d rules for a main file, want 0 with a recorded failure", len(result.Gen))
	}
}

func TestGenerateWithoutRootFails(t *testing.T) {
	lang := NewLanguage()
	result := generate(t, lang, map[string]string{
		"a.zig": "const x = 1;\n",
		"b.zig": "const y = 2;\n",
	})
	if len(result.Gen) != 0 || !failed(lang) {
		t.Fatalf("generated %d rules without a root, want 0 with a recorded failure", len(result.Gen))
	}
}

func TestImportsUseImportNameOrTargetName(t *testing.T) {
	lang := NewLanguage()
	lib := rule.NewRule(LibraryKind, "greet")
	if got := lang.Imports(nil, lib, nil); len(got) != 1 || got[0].Imp != "greet" || got[0].Lang != languageName {
		t.Errorf("imports = %+v, want greet", got)
	}
	lib.SetAttr("import_name", "hello")
	if got := lang.Imports(nil, lib, nil); len(got) != 1 || got[0].Imp != "hello" {
		t.Errorf("imports = %+v, want hello", got)
	}
	if got := lang.Imports(nil, rule.NewRule("filegroup", "greet"), nil); got != nil {
		t.Errorf("other-kind imports = %+v, want nil", got)
	}
}

func resolveWith(t *testing.T, libs map[string]*rule.Rule, imports []string) (*rule.Rule, language.Language) {
	t.Helper()
	lang := NewLanguage()
	lang.(*zigLang).Before(context.Background())
	cfg := config.New()
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	resolver.Configure(cfg, "", &rule.File{})
	index := resolve.NewRuleIndex(func(*rule.Rule, string) resolve.Resolver { return lang })
	for pkg, lib := range libs {
		f := rule.EmptyFile(filepath.Join(pkg, "BUILD.bazel"), pkg)
		lib.Insert(f)
		index.AddRule(cfg, lib, f)
	}
	index.Finish()
	r := rule.NewRule(LibraryKind, "app")
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet(imports), label.New("", "app", "app"))
	return r, lang
}

func TestResolveFindsLibraryByImportName(t *testing.T) {
	renamed := rule.NewRule(LibraryKind, "greet_lib")
	renamed.SetAttr("main", "greet.zig")
	renamed.SetAttr("import_name", "greet")
	r, lang := resolveWith(t, map[string]*rule.Rule{"pkg/greet": renamed}, []string{"greet", "std"})
	if failed(lang) {
		t.Fatal("resolution failed")
	}
	if got := strings.Join(r.AttrStrings("deps"), ","); got != "//pkg/greet:greet_lib" {
		t.Errorf("deps = %q, want //pkg/greet:greet_lib", got)
	}
}

func TestResolveReportsUnknownModules(t *testing.T) {
	_, lang := resolveWith(t, nil, []string{"missing"})
	if !failed(lang) {
		t.Fatal("an unknown module resolved")
	}
}

func TestLanguageMetadata(t *testing.T) {
	lang := NewLanguage().(*zigLang)
	if lang.Name() != languageName || len(lang.Kinds()) != 1 {
		t.Fatalf("metadata = %s / %v", lang.Name(), lang.Kinds())
	}
	if loads := lang.Loads(); len(loads) != 1 || loads[0].Name != "@rules_dx//zig/rules:defs.bzl" {
		t.Errorf("loads = %+v", loads)
	}
}

func TestAfterResolvingDepsReportsFailures(t *testing.T) {
	_, lang := resolveWith(t, nil, []string{"missing"})
	defer func() {
		msg, ok := recover().(string)
		if !ok || !strings.Contains(msg, "Zig generation failed:") || !strings.Contains(msg, `unresolved import "missing"`) {
			t.Fatalf("panic = %q, want the Zig failure header and the unresolved import", msg)
		}
	}()
	lang.(*zigLang).AfterResolvingDeps(context.Background())
}

func generateConfig(t *testing.T, root string) *config.Config {
	t.Helper()
	cfg := config.New()
	cfg.RepoRoot = root
	resolver := &resolve.Configurer{}
	resolver.RegisterFlags(nil, "update", cfg)
	if err := resolver.CheckFlags(nil, cfg); err != nil {
		t.Fatal(err)
	}
	resolver.Configure(cfg, "", nil)
	return cfg
}
