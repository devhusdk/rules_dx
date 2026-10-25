package jvmsyntax

import (
	"fmt"
	"sort"
	"strings"
	"sync"

	ts "github.com/odvcencio/gotreesitter"
)

type Grammar struct {
	name string
	load func() *ts.Language
	mu   sync.Mutex
	lang *ts.Language
	p    *ts.Parser
}

func NewGrammar(name string, load func() *ts.Language) *Grammar {
	return &Grammar{name: name, load: load}
}

type Tree struct {
	lang *ts.Language
	src  []byte
	Root *ts.Node
}

func (g *Grammar) Parse(content []byte) (*Tree, error) {
	g.mu.Lock()
	defer g.mu.Unlock()
	if g.p == nil {
		g.lang = g.load()
		g.p = ts.NewParser(g.lang)
	}
	tree, err := g.p.Parse(content)
	if err != nil {
		return nil, fmt.Errorf("%s: parse: %w", g.name, err)
	}
	return &Tree{lang: g.lang, src: content, Root: tree.RootNode()}, nil
}

func (t *Tree) Type(n *ts.Node) string { return n.Type(t.lang) }

func (t *Tree) Text(n *ts.Node) string { return n.Text(t.src) }

func (t *Tree) Field(n *ts.Node, name string) *ts.Node { return n.ChildByFieldName(name, t.lang) }

func (t *Tree) FieldOf(n *ts.Node, i int) string { return n.FieldNameForChild(i, t.lang) }

func (t *Tree) Children(n *ts.Node) []*ts.Node {
	out := make([]*ts.Node, 0, n.ChildCount())
	for i := 0; i < n.ChildCount(); i++ {
		out = append(out, n.Child(i))
	}
	return out
}

func (t *Tree) ChildOfType(n *ts.Node, typ string) *ts.Node {
	for i := 0; i < n.ChildCount(); i++ {
		if c := n.Child(i); t.Type(c) == typ {
			return c
		}
	}
	return nil
}

func (t *Tree) HasToken(n *ts.Node, token string) bool {
	for i := 0; i < n.ChildCount(); i++ {
		if c := n.Child(i); !c.IsNamed() && t.Type(c) == token {
			return true
		}
	}
	return false
}

func (t *Tree) OfType(types map[string]bool) func(*ts.Node) bool {
	return func(n *ts.Node) bool { return types[t.Type(n)] }
}

func (t *Tree) Walk(n *ts.Node, visit func(*ts.Node) bool) {
	if !visit(n) {
		return
	}
	for i := 0; i < n.ChildCount(); i++ {
		t.Walk(n.Child(i), visit)
	}
}

func (t *Tree) Leaves(n *ts.Node, types ...string) []string {
	var out []string
	t.Walk(n, func(c *ts.Node) bool {
		for _, typ := range types {
			if t.Type(c) == typ {
				out = append(out, Unquote(t.Text(c)))
				return false
			}
		}
		return true
	})
	return out
}

func (t *Tree) HeaderError(n *ts.Node, stop func(*ts.Node) bool, header map[string]bool) error {
	for i := 0; i < n.ChildCount(); i++ {
		c := n.Child(i)
		if !c.IsError() && stop(c) {
			return nil
		}
		if c.IsError() || c.HasErrorOrMissing() {
			return syntaxError(t.firstError(c))
		}
		if header != nil && c.IsNamed() && !c.IsExtra() && !header[t.Type(c)] {
			return syntaxError(c)
		}
	}
	return nil
}

func syntaxError(n *ts.Node) error {
	p := n.StartPoint()
	return fmt.Errorf("syntax error at line %d column %d", p.Row+1, p.Column+1)
}

func (t *Tree) firstError(n *ts.Node) *ts.Node {
	if n.IsError() || n.IsMissing() {
		return n
	}
	for i := 0; i < n.ChildCount(); i++ {
		if c := n.Child(i); c.HasErrorOrMissing() || c.IsError() || c.IsMissing() {
			return t.firstError(c)
		}
	}
	return n
}

func Unquote(name string) string { return strings.Trim(strings.TrimSpace(name), "`") }

func Join(parts []string) string { return strings.Join(parts, ".") }

func Qualify(pkg, name string) string {
	if pkg == "" {
		return name
	}
	return pkg + "." + name
}

func Sorted(set map[string]bool) []string {
	out := make([]string, 0, len(set))
	for name := range set {
		if name != "" {
			out = append(out, name)
		}
	}
	sort.Strings(out)
	return out
}

func Candidates(name string) []string {
	var out []string
	for {
		i := strings.LastIndexByte(name, '.')
		if i < 0 {
			return out
		}
		name = name[:i]
		out = append(out, name)
	}
}
