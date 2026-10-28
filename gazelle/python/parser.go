package python

import (
	"sort"
	"strings"
	"sync"

	"github.com/devhusdk/rules_dx/gazelle/common/treesitter"
	ts "github.com/odvcencio/gotreesitter"
	"github.com/odvcencio/gotreesitter/grammars/python"
)

var grammar = sync.OnceValue(func() *ts.Language { return python.Language() })

// ParseImports returns the sorted, deduplicated top-level module names
// imported by content, including import_module("x") calls. Relative imports
// are skipped.
func ParseImports(content []byte) []string {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	set := make(map[string]struct{})
	add := func(root string) {
		root = strings.TrimSpace(root)
		if idx := strings.IndexByte(root, '.'); idx >= 0 {
			root = strings.TrimSpace(root[:idx])
		}
		if root == "" {
			return
		}
		set[root] = struct{}{}
	}
	if tree.Root != nil {
		tree.Walk(tree.Root, func(n *ts.Node) bool {
			switch tree.Type(n) {
			case "import_statement":
				for _, c := range n.Children() {
					switch tree.Type(c) {
					case "dotted_name":
						add(tree.Dotted(c))
					case "aliased_import":
						if name := c.ChildByFieldName("name", grammar()); name != nil {
							add(tree.Dotted(name))
						}
					}
				}
				return false
			case "import_from_statement":
				if mod := n.ChildByFieldName("module_name", grammar()); mod != nil && tree.Type(mod) == "dotted_name" {
					add(tree.Dotted(mod))
				}
				return false
			case "call":
				addDynamic(tree, n, add)
			}
			return true
		})
	}
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	if len(out) == 0 {
		return nil
	}
	return out
}

func addDynamic(tree *treesitter.Tree, call *ts.Node, add func(string)) {
	fn := call.ChildByFieldName("function", grammar())
	if fn == nil {
		return
	}
	switch tree.Type(fn) {
	case "identifier":
		if tree.Text(fn) != "import_module" {
			return
		}
	case "attribute":
		attr := fn.ChildByFieldName("attribute", grammar())
		if attr == nil || tree.Text(attr) != "import_module" {
			return
		}
	default:
		return
	}
	args := call.ChildByFieldName("arguments", grammar())
	if args == nil {
		return
	}
	for _, a := range args.Children() {
		if !a.IsNamed() {
			continue
		}
		if tree.Type(a) != "string" {
			return
		}
		s := tree.Text(a)
		s = strings.TrimLeft(s, "rRbBuUfF")
		if len(s) >= 6 && (strings.HasPrefix(s, `"""`) || strings.HasPrefix(s, `'''`)) && strings.HasSuffix(s, s[:3]) {
			s = s[3 : len(s)-3]
		} else if len(s) >= 2 && (s[0] == '"' || s[0] == '\'') && s[len(s)-1] == s[0] {
			s = s[1 : len(s)-1]
		} else {
			return
		}
		if strings.HasPrefix(s, ".") {
			return
		}
		add(s)
		return
	}
}
