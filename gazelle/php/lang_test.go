package php

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

func writeFixture(t *testing.T, root string, files map[string]string) {
	t.Helper()
	for name, content := range files {
		file := filepath.Join(root, filepath.FromSlash(name))
		if err := os.MkdirAll(filepath.Dir(file), 0o755); err != nil {
			t.Fatal(err)
		}
		if err := os.WriteFile(file, []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}
}

func generate(t *testing.T, rel string, files map[string]string, regular []string) language.GenerateResult {
	t.Helper()
	root := t.TempDir()
	writeFixture(t, root, files)
	return NewLanguage().GenerateRules(language.GenerateArgs{
		Config:       &config.Config{RepoRoot: root},
		Dir:          filepath.Join(root, filepath.FromSlash(rel)),
		Rel:          rel,
		RegularFiles: regular,
	})
}

func TestGenerateLibraryAndTests(t *testing.T) {
	result := generate(t, "pkg/demo", map[string]string{
		"pkg/demo/Demo.php":                 "<?php\nnamespace Demo;\nclass Demo extends Sub\\Thing {}\n",
		"pkg/demo/src/Sub/Thing.php":        "<?php\nnamespace Demo\\Sub;\nuse Vendor\\Lib;\nclass Thing extends \\Exception { function f() { new Lib(); } }\n",
		"pkg/demo/tests/SubThingTest.php":   "<?php\nuse PHPUnit\\Framework\\TestCase;\nuse Demo\\Sub\\Thing;\nclass SubThingTest extends TestCase { function f() { new Thing(); } }\n",
		"pkg/demo/src/Nested/BUILD.bazel":   "",
		"pkg/demo/src/Nested/Own.php":       "<?php\nclass Own {}\n",
		"pkg/demo/bin/run.php":              "<?php\nnew Demo\\Demo();\n",
		"pkg/demo/tests/helpers/Helper.php": "<?php\nclass Helper {}\n",
	}, []string{"Demo.php", "notes.txt"})
	if len(result.Gen) != 3 || len(result.Imports) != 3 {
		t.Fatalf("generated %d rules and %d import sets, want 3 each", len(result.Gen), len(result.Imports))
	}
	lib, support, test := result.Gen[0], result.Gen[1], result.Gen[2]
	if support.Name() != "demo_test_support" || support.AttrBool("testonly") != true ||
		strings.Join(support.AttrStrings("srcs"), ",") != "tests/helpers/Helper.php" {
		t.Errorf("support = %s srcs %v testonly %v", support.Name(), support.AttrStrings("srcs"), support.AttrBool("testonly"))
	}
	if lib.Kind() != LibraryKind || lib.Name() != "demo" {
		t.Errorf("library = %s(%s), want php_library(demo)", lib.Kind(), lib.Name())
	}
	if got := strings.Join(lib.AttrStrings("srcs"), ","); got != "Demo.php,src/Sub/Thing.php" {
		t.Errorf("library srcs = %q", got)
	}
	if test.Kind() != TestKind || test.Name() != "sub_thing_test" {
		t.Errorf("test = %s(%s), want php_test(sub_thing_test)", test.Kind(), test.Name())
	}
	libImports := result.Imports[0].(common.ImportSet).Names
	if !reflect.DeepEqual(libImports, []string{"Demo\\Sub\\Thing", "Exception", "Vendor\\Lib"}) {
		t.Errorf("library imports = %v", libImports)
	}
	testImports := result.Imports[2].(common.ImportSet).Names
	if !reflect.DeepEqual(testImports, []string{"Demo\\Sub\\Thing"}) {
		t.Errorf("test imports = %v, want PHPUnit dropped", testImports)
	}
}

func TestGenerateKeepsAutoloadFilesOutOfSrcs(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, map[string]string{
		"pkg/BUILD.bazel": "php_library(name = \"pkg\", srcs = [], autoload_files = [\"src/functions.php\"])\n",
		"pkg/src/A.php":         "<?php\nclass A {}\n",
		"pkg/src/functions.php": "<?php\nnamespace N;\nfunction f() { new Dep(); }\n",
	})
	f, err := rule.LoadFile(filepath.Join(root, "pkg", "BUILD.bazel"), "pkg")
	if err != nil {
		t.Fatal(err)
	}
	result := NewLanguage().GenerateRules(language.GenerateArgs{
		Config: &config.Config{RepoRoot: root}, Dir: filepath.Join(root, "pkg"), Rel: "pkg", File: f, RegularFiles: []string{"BUILD.bazel"},
	})
	if len(result.Gen) != 1 {
		t.Fatalf("generated %d rules, want 1", len(result.Gen))
	}
	if got := result.Gen[0].AttrStrings("srcs"); !reflect.DeepEqual(got, []string{"src/A.php"}) {
		t.Errorf("srcs = %v, want only src/A.php", got)
	}
	if got := result.Imports[0].(common.ImportSet).Names; !reflect.DeepEqual(got, []string{"N\\Dep"}) {
		t.Errorf("imports = %v, want the autoload file's references", got)
	}
}

func TestGenerateSkipsDirectoriesOwnedByParent(t *testing.T) {
	files := map[string]string{
		"pkg/src/Acme/Widget.php": "<?php\nclass Widget {}\n",
		"pkg/tests/XTest.php":     "<?php\n",
	}
	for _, rel := range []string{"pkg/src", "pkg/src/Acme", "pkg/tests"} {
		result := generate(t, rel, files, []string{"Widget.php"})
		if len(result.Gen) != 0 || len(result.Empty) != 0 {
			t.Errorf("%s generated %d rules, want none", rel, len(result.Gen))
		}
	}
}

func TestGenerateNestedPackageOwnsItsSources(t *testing.T) {
	result := generate(t, "pkg/src/Acme", map[string]string{
		"pkg/src/Acme/BUILD.bazel": "",
		"pkg/src/Acme/Widget.php":  "<?php\nclass Widget {}\n",
	}, []string{"BUILD.bazel", "Widget.php"})
	if len(result.Gen) != 1 || result.Gen[0].Name() != "Acme" {
		t.Fatalf("generated %v, want php_library(Acme)", result.Gen)
	}
}

func TestGenerateNeverEmitsBinaries(t *testing.T) {
	result := generate(t, "pkg/tool", map[string]string{
		"pkg/tool/bin/run.php": "<?php\necho 1;\n",
	}, []string{})
	if len(result.Gen) != 0 {
		t.Errorf("generated %d rules for a script-only directory, want none", len(result.Gen))
	}
}

func TestGenerateRejectsDuplicateTestNames(t *testing.T) {
	lang := NewLanguage().(*phpLang)
	root := t.TempDir()
	writeFixture(t, root, map[string]string{
		"pkg/tests/AATest.php":     "<?php\n",
		"pkg/tests/sub/AATest.php": "<?php\n",
	})
	lang.GenerateRules(language.GenerateArgs{Config: &config.Config{RepoRoot: root}, Dir: filepath.Join(root, "pkg"), Rel: "pkg"})
	if !lang.Failed() {
		t.Error("duplicate test names were accepted")
	}
}

func TestImportsIndexDeclaredNames(t *testing.T) {
	root := t.TempDir()
	writeFixture(t, root, map[string]string{
		"pkg/src/Thing.php":  "<?php\nnamespace Acme;\nclass Thing {}\ninterface Face {}\n",
		"pkg/functions.php": "<?php\nnamespace Acme;\nfunction helper() {}\n",
	})
	lib := rule.NewRule(LibraryKind, "pkg")
	lib.SetAttr("srcs", []string{"src/Thing.php", "notes.txt"})
	lib.SetAttr("autoload_files", []string{"functions.php"})
	f := &rule.File{Path: filepath.Join(root, "pkg", "BUILD.bazel"), Pkg: "pkg"}
	var got []string
	for _, spec := range NewLanguage().Imports(&config.Config{}, lib, f) {
		got = append(got, spec.Imp)
	}
	if !reflect.DeepEqual(got, []string{"Acme\\Face", "Acme\\Thing", "Acme\\helper"}) {
		t.Errorf("imports = %v", got)
	}
	if specs := NewLanguage().Imports(&config.Config{}, rule.NewRule(TestKind, "x"), f); specs != nil {
		t.Errorf("test imports = %v, want nil", specs)
	}
}

func resolveImports(t *testing.T, names []string) (*phpLang, *rule.Rule) {
	t.Helper()
	lang := NewLanguage().(*phpLang)
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
	lang.Resolve(cfg, index, nil, r, common.WrapImportSet(names), label.New("", "app", "app"))
	return lang, r
}

func TestResolveSkipsBuiltins(t *testing.T) {
	lang, r := resolveImports(t, []string{"Exception", "ArrayObject", "PHP_EOL"})
	if lang.Failed() {
		t.Fatalf("builtin imports were reported: %v", lang.Errors())
	}
	if deps := r.AttrStrings("deps"); len(deps) != 0 {
		t.Errorf("deps = %v, want none", deps)
	}
}

func TestResolveDoesNotTreatNamespacedNamesAsBuiltins(t *testing.T) {
	lang, _ := resolveImports(t, []string{"App\\Exception"})
	if !lang.Failed() {
		t.Error("App\\Exception resolved as a builtin")
	}
}

func TestResolveReportsVendorImportWithHint(t *testing.T) {
	lang, _ := resolveImports(t, []string{"Vendor\\Lib\\Thing"})
	errs := lang.Errors()
	if len(errs) != 1 || !strings.Contains(errs[0], "Vendor\\Lib\\Thing") || !strings.Contains(errs[0], "gazelle:resolve") {
		t.Errorf("errors = %v, want one unresolved Vendor\\Lib\\Thing with a resolve hint", errs)
	}
}
