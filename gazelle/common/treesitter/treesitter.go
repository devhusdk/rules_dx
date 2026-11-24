package treesitter

import (
	"strings"

	ts "github.com/odvcencio/gotreesitter"
)

type Node = ts.Node

type Tree struct {
	tree *ts.Tree
	lang *ts.Language
	src  []byte

	Root     *Node
	HasError bool
}

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

func (t *Tree) Release() {
	if t.tree != nil {
		t.tree.Release()
		t.tree = nil
	}
}

func (t *Tree) Type(n *Node) string { return n.Type(t.lang) }

func (t *Tree) Text(n *Node) string { return Text(n, t.src) }

func (t *Tree) Dotted(n *Node) string { return Dotted(n, t.src, t.lang) }

func (t *Tree) Walk(n *Node, fn func(*Node) bool) { Walk(n, fn) }

func Walk(n *Node, fn func(*Node) bool) {
	if n == nil || !fn(n) {
		return
	}
	for _, c := range n.Children() {
		Walk(c, fn)
	}
}

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
