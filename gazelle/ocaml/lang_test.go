package ocaml

import (
	"testing"

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
