package ocaml

import (
	"testing"

	"github.com/bazel-contrib/bazel-gazelle/v2/rule"
	"github.com/bazelbuild/bazel-gazelle/language"
)

func TestGenerateModules(t *testing.T) {
	lang := NewLanguage()
	res := lang.GenerateRules(language.GenerateArgs{
		Rel:          "pkg",
		RegularFiles: []string{"b.ml", "a.ml", "a.mli", "orphan.mli", "notes.txt"},
	})
	if len(res.Gen) != 2 {
		t.Fatalf("got %d rules, want 2", len(res.Gen))
	}
	a, b := res.Gen[0], res.Gen[1]
	if a.Name() != "a" || a.AttrString("struct") != "a.ml" || a.AttrString("sig") != "a.mli" {
		t.Errorf("bad a rule: %v", a.Name())
	}
	if b.Name() != "b" || b.AttrString("sig") != "" {
		t.Errorf("bad b rule: %v", b.Name())
	}
	if len(res.Imports) != 2 || res.Imports[0] != nil {
		t.Errorf("imports must stay empty: %v", res.Imports)
	}
}

func TestNoSourcesNoRules(t *testing.T) {
	res := NewLanguage().GenerateRules(language.GenerateArgs{Rel: "pkg", RegularFiles: []string{"x.txt"}})
	if len(res.Gen) != 0 {
		t.Fatalf("got %d rules", len(res.Gen))
	}
}

func ocamlFile(t *testing.T, build string) *rule.File {
	t.Helper()
	f, err := rule.LoadData("BUILD.bazel", "pkg", []byte(build))
	if err != nil {
		t.Fatal(err)
	}
	return f
}

func emptyNames(res language.GenerateResult) []string {
	var names []string
	for _, r := range res.Empty {
		names = append(names, r.Name())
	}
	return names
}

func TestGenerateSkipsSourcesClaimedByOtherKinds(t *testing.T) {
	file := ocamlFile(t, `ocaml_binary(name = "fuzz_bin", main = "fuzz.ml")`)
	res := NewLanguage().GenerateRules(language.GenerateArgs{Rel: "pkg", File: file, RegularFiles: []string{"fuzz.ml", "lib.ml"}})
	if len(res.Gen) != 1 || res.Gen[0].Name() != "lib" {
		t.Fatalf("gen = %v, want only lib", res.Gen)
	}
}

func TestGenerateKeepsModulesWithGeneratedStruct(t *testing.T) {
	file := ocamlFile(t, `genrule(name = "gen", outs = ["made.ml"])
ocaml_module(name = "made", struct = "made.ml")
ocaml_module(name = "ext", struct = "@dep//:ext.ml")
ocaml_module(name = "sub", struct = "sub/x.ml")
ocaml_module(name = "gone", struct = "gone.ml")`)
	res := NewLanguage().GenerateRules(language.GenerateArgs{Rel: "pkg", File: file, RegularFiles: []string{"x.txt"}})
	if got := emptyNames(res); len(got) != 1 || got[0] != "gone" {
		t.Fatalf("stale = %v, want [gone]", got)
	}
}
