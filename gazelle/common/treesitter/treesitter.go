// Package treesitter wraps the pure-Go tree-sitter runtime (gotreesitter) with a
// small, language-agnostic API for the gazelle import parsers. It holds no
// language-specific logic; each language package supplies its own grammar.
package treesitter

import (
	"strings"

	ts "github.com/odvcencio/gotreesitter"
)

// Node is a syntax tree node.
type Node = ts.Node

// Tree is a parsed source file. Release it when done with its nodes.
type Tree struct {
	tree *ts.Tree
	lang *ts.Language
	src  []byte

	// Root is the root node, or nil when the parse produced no tree.
	Root *Node
	// HasError reports whether the tree contains ERROR or missing nodes. The
	// tree is still usable: error recovery keeps the well-formed parts.
	HasError bool
}

// Parse parses content with lang. A fresh parser is created per call, so Parse
// is safe for concurrent use.
func Parse(lang *ts.Language, content []byte) *Tree {
	t := &Tree{lang: lang, src: content}
	tree, err := ts.NewParser(lang).Parse(content)
	if err != nil || tree == nil {
		t.HasError = true
		return t
	}
	t.tree = tree
	t.Root = tree.RootNode()
	if t.Root == nil {
		t.HasError = true
		return t
	}
	t.HasError = t.Root.HasError()
	return t
}

// Release frees the tree's memory. Nodes must not be used afterwards.
func (t *Tree) Release() {
	if t.tree != nil {
		t.tree.Release()
		t.tree = nil
	}
}

// Type returns the grammar node type of n.
func (t *Tree) Type(n *Node) string { return n.Type(t.lang) }

// Text returns the source text covered by n.
func (t *Tree) Text(n *Node) string { return Text(n, t.src) }

// Dotted joins the leaf tokens under n, dropping comments.
func (t *Tree) Dotted(n *Node) string { return Dotted(n, t.src, t.lang) }

// Walk visits n and its descendants depth first in source order. When fn
// returns false the children of that node are skipped.
func (t *Tree) Walk(n *Node, fn func(*Node) bool) { Walk(n, fn) }

// Walk visits n and its descendants depth first in source order. When fn
// returns false the children of that node are skipped.
func Walk(n *Node, fn func(*Node) bool) {
	if n == nil || !fn(n) {
		return
	}
	for _, c := range n.Children() {
		Walk(c, fn)
	}
}

// Text returns the source text covered by n.
func Text(n *Node, content []byte) string {
	if n == nil {
		return ""
	}
	s, e := int(n.StartByte()), int(n.EndByte())
	if s < 0 || e > len(content) || s > e {
		return ""
	}
	return string(content[s:e])
}

// Dotted joins the leaf tokens under n without whitespace, dropping comments.
// For a qualified identifier such as a./*x*/B it returns "a.B".
func Dotted(n *Node, content []byte, lang *ts.Language) string {
	var b strings.Builder
	Walk(n, func(c *Node) bool {
		if strings.Contains(c.Type(lang), "comment") {
			return false
		}
		if c.ChildCount() == 0 {
			b.WriteString(Text(c, content))
		}
		return true
	})
	return b.String()
}
