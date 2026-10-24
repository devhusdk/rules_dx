package jvmsyntax

import (
	"reflect"
	"testing"

	ts "github.com/odvcencio/gotreesitter"
	javagrammar "github.com/odvcencio/gotreesitter/grammars/java"
)

var java = NewGrammar("java", javagrammar.Language)

func mustParse(t *testing.T, src string) *Tree {
	t.Helper()
	tree, err := java.Parse([]byte(src))
	if err != nil {
		t.Fatalf("Parse: %v", err)
	}
	return tree
}

func TestCandidatesStripTrailingSegments(t *testing.T) {
	got := Candidates("com.example.Outer.Inner")
	if want := []string{"com.example.Outer", "com.example"}; !reflect.DeepEqual(got, want) {
		t.Errorf("Candidates = %v, want %v", got, want)
	}
	if got := Candidates("Widget"); got != nil {
		t.Errorf("Candidates(Widget) = %v, want none", got)
	}
	if got := Candidates("example.Widget"); got != nil {
		t.Errorf("Candidates(example.Widget) = %v, want none", got)
	}
}

func TestNameHelpers(t *testing.T) {
	if got := Qualify("com.example", "Widget"); got != "com.example.Widget" {
		t.Errorf("Qualify = %q", got)
	}
	if got := Qualify("", "Widget"); got != "Widget" {
		t.Errorf("Qualify default package = %q", got)
	}
	if got := Unquote(" `Quoted` "); got != "Quoted" {
		t.Errorf("Unquote = %q", got)
	}
	if got := Join([]string{"a", "b", "c"}); got != "a.b.c" {
		t.Errorf("Join = %q", got)
	}
	if got := Sorted(map[string]bool{"b": true, "": true, "a": true}); !reflect.DeepEqual(got, []string{"a", "b"}) {
		t.Errorf("Sorted = %v", got)
	}
}

func TestTreeQueries(t *testing.T) {
	tree := mustParse(t, "package com.example;\n\nimport static com.example.Tools.run;\n\nclass Widget {}\n")
	pkg := tree.ChildOfType(tree.Root, "package_declaration")
	if pkg == nil {
		t.Fatal("missing package_declaration")
	}
	if got := tree.Leaves(pkg, "scoped_identifier", "identifier"); !reflect.DeepEqual(got, []string{"com.example"}) {
		t.Errorf("Leaves = %v", got)
	}
	imp := tree.ChildOfType(tree.Root, "import_declaration")
	if !tree.HasToken(imp, "static") || tree.HasToken(pkg, "static") {
		t.Error("HasToken mismatch")
	}
	if tree.ChildOfType(tree.Root, "enum_declaration") != nil {
		t.Error("ChildOfType found a missing node")
	}
	class := tree.ChildOfType(tree.Root, "class_declaration")
	name := tree.Field(class, "name")
	if name == nil || tree.Text(name) != "Widget" {
		t.Fatalf("class name = %v", name)
	}
	fields := map[string]bool{}
	for i := range tree.Children(class) {
		fields[tree.FieldOf(class, i)] = true
	}
	if !fields["name"] || !fields["body"] {
		t.Errorf("fields = %v", fields)
	}
	count := 0
	tree.Walk(tree.Root, func(n *ts.Node) bool {
		count++
		return tree.Type(n) != "class_declaration"
	})
	if count == 0 {
		t.Error("Walk visited nothing")
	}
}

func TestHeaderError(t *testing.T) {
	stop := func(tree *Tree) func(*ts.Node) bool {
		return tree.OfType(map[string]bool{"class_declaration": true})
	}
	clean := mustParse(t, "package demo;\n\nimport a.B;\n\nclass Demo { void f() { int x = ; } }\n")
	if err := clean.HeaderError(clean.Root, stop(clean)); err != nil {
		t.Errorf("body error failed the header: %v", err)
	}
	broken := mustParse(t, "package demo;\n\nimport a.;\n\nclass Demo {}\n")
	err := broken.HeaderError(broken.Root, stop(broken))
	if err == nil || err.Error() != "syntax error at line 3 column 10" {
		t.Errorf("err = %v, want line 3 column 10", err)
	}
	empty := mustParse(t, "")
	if err := empty.HeaderError(empty.Root, stop(empty)); err != nil {
		t.Errorf("empty source: %v", err)
	}
}
