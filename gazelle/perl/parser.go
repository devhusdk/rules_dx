package perl

import (
	"sort"
	"strings"
	"sync"

	"github.com/devhusdk/rules_dx/gazelle/common/treesitter"
	ts "github.com/odvcencio/gotreesitter"
	"github.com/odvcencio/gotreesitter/grammars/perl"
)

var grammar = sync.OnceValue(func() *ts.Language { return perl.Language() })

// ParseImports returns the sorted, deduplicated module names content loads
// with use, no, require or the parent and base pragmas. Loads guarded by eval,
// version requirements, file requires and text inside POD, heredocs, strings
// and the data section are skipped. Imports before a syntax error are kept.
func ParseImports(content []byte) []string {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	set := make(map[string]struct{})
	if tree.Root != nil {
		tree.Walk(tree.Root, func(n *treesitter.Node) bool {
			switch tree.Type(n) {
			case "eval_expression":
				return false
			case "use_statement":
				collectUse(tree, n, set)
				return false
			case "require_expression":
				for _, c := range n.Children() {
					if tree.Type(c) == "bareword" {
						addModule(set, tree.Text(c))
					}
				}
			}
			return true
		})
	}
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}

func collectUse(tree *treesitter.Tree, n *treesitter.Node, set map[string]struct{}) {
	var module string
	var args []*treesitter.Node
	for _, c := range n.Children() {
		switch tree.Type(c) {
		case "package":
			if module == "" {
				module = tree.Text(c)
			}
		case "use", "no", ";", "number", "version":
		default:
			args = append(args, c)
		}
	}
	if module == "" {
		return
	}
	addModule(set, module)
	if (module != "parent" && module != "base") || strings.Contains(tree.Text(n), "-norequire") {
		return
	}
	for _, a := range args {
		tree.Walk(a, func(c *treesitter.Node) bool {
			if tree.Type(c) == "string_content" {
				for _, name := range strings.Fields(tree.Text(c)) {
					addModule(set, name)
				}
			}
			return true
		})
	}
}

func addModule(set map[string]struct{}, name string) {
	if isModuleName(name) {
		set[name] = struct{}{}
	}
}

func isModuleName(name string) bool {
	if name == "" {
		return false
	}
	for _, part := range strings.Split(name, "::") {
		if part == "" {
			return false
		}
		for i, r := range part {
			switch {
			case r == '_' || r >= 'a' && r <= 'z' || r >= 'A' && r <= 'Z' || r >= 0x80:
			case r >= '0' && r <= '9' && i > 0:
			default:
				return false
			}
		}
	}
	return true
}
