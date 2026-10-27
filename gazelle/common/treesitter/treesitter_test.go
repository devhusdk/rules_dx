package treesitter

import (
	"strings"
	"testing"

	"github.com/odvcencio/gotreesitter/grammars/java"
)

const src = "package a.b;\nimport c./*x*/D;\nimport e.F;\nclass G {}\n"

func TestWalkOrderAndText(t *testing.T) {
	tree := Parse(java.Language(), []byte(src))
	defer tree.Release()
	if tree.HasError {
		t.Fatal("unexpected parse error")
	}
	var kinds []string
	tree.Walk(tree.Root, func(n *Node) bool {
		switch k := tree.Type(n); k {
		case "package_declaration", "import_declaration", "class_declaration":
			kinds = append(kinds, k)
			return false
		}
		return true
	})
	want := "package_declaration import_declaration import_declaration class_declaration"
	if got := strings.Join(kinds, " "); got != want {
		t.Fatalf("walk order = %q, want %q", got, want)
	}
	first := tree.Root.Child(0)
	if got := tree.Text(first); got != "package a.b;" {
		t.Fatalf("Text = %q", got)
	}
}

func TestDottedDropsCommentsAndWhitespace(t *testing.T) {
	tree := Parse(java.Language(), []byte("import c . /*x*/ D\n// y\n .E;\n"))
	defer tree.Release()
	var got string
	tree.Walk(tree.Root, func(n *Node) bool {
		if tree.Type(n) == "scoped_identifier" {
			got = tree.Dotted(n)
			return false
		}
		return true
	})
	if got != "c.D.E" {
		t.Fatalf("Dotted = %q, want c.D.E", got)
	}
}

func TestParseMalformedKeepsTree(t *testing.T) {
	tree := Parse(java.Language(), []byte("import a.B;\nclass X { String s = \"oops\n"))
	defer tree.Release()
	if tree.Root == nil || !tree.HasError {
		t.Fatalf("want a tree flagged with errors, root=%v err=%v", tree.Root, tree.HasError)
	}
}
