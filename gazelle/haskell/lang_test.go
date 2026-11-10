package haskell

import (
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"testing"

	"github.com/bazelbuild/bazel-gazelle/config"
	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
)

func TestGenerateLibrary(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, "pkg", "greet")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	files := map[string]string{
		"Greet.hs":     "module Greet where\nimport Data.List\nimport Util\nimport Data.Aeson\n",
		"Util.hs":      "module Util where\n",
		"GreetSpec.hs": "module GreetSpec where\n",
	}
	var regular []string
	for name, content := range files {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
		regular = append(regular, name)
	}
	lang := NewLanguage()
	res := lang.GenerateRules(language.GenerateArgs{
		Config: &config.Config{RepoRoot: root}, Dir: dir, Rel: "pkg/greet", RegularFiles: regular,
	})
	if len(res.Gen) != 1 || res.Gen[0].Kind() != LibraryKind || res.Gen[0].Name() != "greet" {
		t.Fatalf("unexpected rules: %v", res.Gen)
	}
	if got, want := res.Gen[0].AttrStrings("srcs"), []string{"Greet.hs", "Util.hs"}; !reflect.DeepEqual(got, want) {
		t.Fatalf("srcs = %v, want %v", got, want)
	}
}

func TestMainModuleFails(t *testing.T) {
	root := t.TempDir()
	dir := filepath.Join(root, "app")
	if err := os.MkdirAll(dir, 0o755); err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(dir, "Main.hs"), []byte("module Main where\n"), 0o644); err != nil {
		t.Fatal(err)
	}
	lang := NewLanguage()
	lang.GenerateRules(language.GenerateArgs{
		Config: &config.Config{RepoRoot: root}, Dir: dir, Rel: "app", RegularFiles: []string{"Main.hs"},
	})
	if !lang.(*haskellLang).Failed() {
		t.Fatal("expected failure for a Main module")
	}
}

func TestImportsUseDeclaredModule(t *testing.T) {
	dir := t.TempDir()
	if err := os.MkdirAll(filepath.Join(dir, "Lib"), 0o755); err != nil {
		t.Fatal(err)
	}
	for name, content := range map[string]string{
		"Lib/Utils.hs": "module Lib.Utils where\n",
		"Lib/Bare.hs":  "x = 1\n",
	} {
		if err := os.WriteFile(filepath.Join(dir, name), []byte(content), 0o644); err != nil {
			t.Fatal(err)
		}
	}
	f := rule.EmptyFile(filepath.Join(dir, "BUILD.bazel"), "")
	r := rule.NewRule(LibraryKind, "lib")
	r.SetAttr("srcs", []string{"Lib/Utils.hs", "Lib/Bare.hs"})
	var got []string
	for _, spec := range NewLanguage().Imports(nil, r, f) {
		got = append(got, spec.Imp)
	}
	sort.Strings(got)
	if want := []string{"Bare", "Lib.Utils"}; !reflect.DeepEqual(got, want) {
		t.Errorf("imports = %v, want %v", got, want)
	}
}
