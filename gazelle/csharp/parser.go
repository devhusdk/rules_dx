package csharp

import (
	"sort"
	"strings"
	"sync"

	"github.com/devhusdk/rules_dx/gazelle/common/treesitter"
	ts "github.com/odvcencio/gotreesitter"
	"github.com/odvcencio/gotreesitter/grammars/c_sharp"
)

var grammar = sync.OnceValue(func() *ts.Language { return c_sharp.Language() })

func ParseImports(content []byte) []string {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	set := make(map[string]struct{})
	tree.Walk(tree.Root, func(n *treesitter.Node) bool {
		switch tree.Type(n) {
		case "using_directive":
			var dotted string
			var static, alias bool
			for _, c := range n.Children() {
				switch tree.Type(c) {
				case "static":
					static = true
				case "=":
					alias = true
					dotted = ""
				case "identifier", "qualified_name":
					if alias || dotted == "" {
						dotted = tree.Dotted(c)
					}
				}
			}
			if dotted != "" {
				if static || alias {
					dotted = foldTypeTarget(dotted)
				}
				set[dotted] = struct{}{}
			}
			return false
		case "block", "class_declaration", "struct_declaration", "method_declaration":
			return false
		}
		return true
	})
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}

func ParsePackage(content []byte) (string, error) {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	var names []string
	collectNamespaces(tree, tree.Root, "", &names)
	if len(names) > 1 {
		return "", errDuplicatePackage(names[0])
	}
	if len(names) == 0 {
		return "", nil
	}
	return names[0], nil
}

func collectNamespaces(tree *treesitter.Tree, n *treesitter.Node, prefix string, names *[]string) {
	if n == nil {
		return
	}
	for _, c := range n.Children() {
		switch tree.Type(c) {
		case "namespace_declaration", "file_scoped_namespace_declaration":
			full := prefix
			for _, k := range c.Children() {
				if kind := tree.Type(k); kind == "identifier" || kind == "qualified_name" {
					full = tree.Dotted(k)
					if prefix != "" {
						full = prefix + "." + full
					}
					break
				}
			}
			before := len(*names)
			collectNamespaces(tree, c, full, names)
			if len(*names) == before && full != "" {
				*names = append(*names, full)
			}
		case "block", "class_declaration", "struct_declaration", "method_declaration":
		default:
			collectNamespaces(tree, c, prefix, names)
		}
	}
}

type duplicatePackageError struct{ first string }

func (e *duplicatePackageError) Error() string {
	return "csharp: duplicate namespace declaration " + e.first
}

func errDuplicatePackage(first string) error { return &duplicatePackageError{first: first} }

func DefinesMain(content []byte) bool {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	found := false
	tree.Walk(tree.Root, func(n *treesitter.Node) bool {
		if found {
			return false
		}
		switch tree.Type(n) {
		case "global_statement":
			found = tree.Text(n) != ";"
			return false
		case "method_declaration":
			var name string
			var static bool
			for _, c := range n.Children() {
				switch tree.Type(c) {
				case "modifier":
					if tree.Text(c) == "static" {
						static = true
					}
				case "identifier":
					name = tree.Text(c)
				}
			}
			found = static && name == "Main"
			return false
		case "block":
			return false
		}
		return true
	})
	return found
}

func foldTypeTarget(dotted string) string {
	if IsStdLib(dotted) {
		return dotted
	}
	if i := strings.LastIndexByte(dotted, '.'); i >= 0 && IsStdLib(dotted[:i]) {
		return dotted[:i]
	}
	return dotted
}

func namespaceCandidates(name string) []string {
	i := strings.LastIndexByte(name, '.')
	if i <= 0 {
		return nil
	}
	return []string{name[:i]}
}
