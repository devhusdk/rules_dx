package rust

import (
	"strings"
	"sync"

	ts "github.com/odvcencio/gotreesitter"
	"github.com/odvcencio/gotreesitter/grammars/rust"
)

var parserPool = sync.Pool{New: func() any { return ts.NewParser(rust.Language()) }}

type syntaxTree struct {
	tree *ts.Tree
	lang *ts.Language
	src  []byte
	root syntaxNode
}

type syntaxNode struct {
	t *syntaxTree
	n *ts.Node
}

func parseSyntax(src []byte) (*syntaxTree, error) {
	p := parserPool.Get().(*ts.Parser)
	defer parserPool.Put(p)
	tree, err := p.Parse(src)
	if err != nil {
		return nil, err
	}
	t := &syntaxTree{tree: tree, lang: rust.Language(), src: src}
	t.root = syntaxNode{t: t, n: tree.RootNode()}
	return t, nil
}

func (t *syntaxTree) release() { t.tree.Release() }

func (n syntaxNode) valid() bool { return n.n != nil }

func (n syntaxNode) kind() string { return n.n.Type(n.t.lang) }

func (n syntaxNode) start() int { return int(n.n.StartByte()) }

func (n syntaxNode) end() int { return int(n.n.EndByte()) }

func (n syntaxNode) text() string { return string(n.t.src[n.start():n.end()]) }

func (n syntaxNode) isNamed() bool { return n.n.IsNamed() }

func (n syntaxNode) isExtra() bool { return n.n.IsExtra() }

func (n syntaxNode) children() []syntaxNode {
	raw := n.n.Children()
	out := make([]syntaxNode, len(raw))
	for i, c := range raw {
		out[i] = syntaxNode{t: n.t, n: c}
	}
	return out
}

func (n syntaxNode) field(name string) syntaxNode {
	return syntaxNode{t: n.t, n: n.n.ChildByFieldName(name, n.t.lang)}
}

func (n syntaxNode) position() (line, col int) {
	return lineCol(n.t.src, n.start())
}

func lineCol(src []byte, off int) (line, col int) {
	pre := src[:off]
	line = 1 + strings.Count(string(pre), "\n")
	col = off - (strings.LastIndex(string(pre), "\n") + 1) + 1
	return line, col
}

func (n syntaxNode) findError(anc []syntaxNode, skip func(e syntaxNode, anc []syntaxNode) bool) (syntaxNode, []syntaxNode, bool) {
	if n.n.IsError() || n.n.IsMissing() {
		if skip != nil && skip(n, anc) {
			return syntaxNode{}, nil, false
		}
		return n, anc, true
	}
	if !n.n.HasErrorOrMissing() {
		return syntaxNode{}, nil, false
	}
	anc = append(anc[:len(anc):len(anc)], n)
	for _, c := range n.children() {
		if e, a, ok := c.findError(anc, skip); ok {
			return e, a, true
		}
	}
	return syntaxNode{}, nil, false
}

func (n syntaxNode) firstToken() string {
	toks := n.tokens()
	if len(toks) == 0 {
		return ""
	}
	return toks[0]
}

func (n syntaxNode) isComment() bool {
	k := n.kind()
	return k == "line_comment" || k == "block_comment"
}

func (n syntaxNode) leaves() []syntaxNode {
	if n.isComment() {
		return nil
	}
	kids := n.children()
	if len(kids) == 0 {
		return []syntaxNode{n}
	}
	var out []syntaxNode
	for _, c := range kids {
		out = append(out, c.leaves()...)
	}
	return out
}

func (n syntaxNode) tokens() []string {
	leaves := n.leaves()
	out := make([]string, len(leaves))
	for i, l := range leaves {
		out[i] = l.text()
	}
	return out
}

func (n syntaxNode) joined() string { return strings.Join(n.tokens(), "") }

func (n syntaxNode) containsDecl() bool {
	switch n.kind() {
	case "mod_item", "use_declaration", "extern_crate_declaration":
		return true
	}
	for _, c := range n.children() {
		if c.containsDecl() {
			return true
		}
	}
	return false
}
