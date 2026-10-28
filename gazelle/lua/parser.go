package lua

import (
	"sort"
	"strings"
	"sync"

	"github.com/devhusdk/rules_dx/gazelle/common/treesitter"
	ts "github.com/odvcencio/gotreesitter"
	"github.com/odvcencio/gotreesitter/grammars/lua"
)

var grammar = sync.OnceValue(func() *ts.Language { return lua.Language() })

// ParseImports returns the sorted, deduplicated module names content loads
// with a plain require call whose only argument is a string literal. Optional
// loads under pcall or xpcall, dynamic names, aliased require and text inside
// comments or strings are skipped. Imports before a syntax error are kept.
func ParseImports(content []byte) []string {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	set := make(map[string]struct{})
	if tree.Root != nil {
		tree.Walk(tree.Root, func(n *treesitter.Node) bool {
			if tree.Type(n) != "function_call" {
				return true
			}
			kids := n.Children()
			if len(kids) < 2 || tree.Type(kids[0]) != "identifier" {
				return true
			}
			switch tree.Text(kids[0]) {
			case "pcall", "xpcall":
				return false
			case "require":
				if name, ok := literalArg(tree, kids[len(kids)-1]); ok {
					set[name] = struct{}{}
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

// literalArg returns the module name of an arguments node holding exactly one
// escape-free string literal.
func literalArg(tree *treesitter.Tree, args *treesitter.Node) (string, bool) {
	if tree.Type(args) != "arguments" {
		return "", false
	}
	var str *treesitter.Node
	for _, c := range args.Children() {
		switch tree.Type(c) {
		case "(", ")":
		case "string":
			if str != nil {
				return "", false
			}
			str = c
		default:
			return "", false
		}
	}
	if str == nil {
		return "", false
	}
	var content *treesitter.Node
	for _, c := range str.Children() {
		switch tree.Type(c) {
		case "string_content":
			if content != nil {
				return "", false
			}
			content = c
		case "escape_sequence":
			return "", false
		}
	}
	if content == nil {
		return "", false
	}
	name := tree.Text(content)
	if !isModuleName(name) {
		return "", false
	}
	return name, true
}

func isModuleName(name string) bool {
	if name == "" || strings.ContainsAny(name, "/\\ \t\r\n") {
		return false
	}
	for _, seg := range strings.Split(name, ".") {
		if seg == "" {
			return false
		}
	}
	return true
}
