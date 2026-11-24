package kotlin

import (
	"sort"
	"strings"
	"sync"

	"github.com/devhusdk/rules_dx/gazelle/common/treesitter"
	ts "github.com/odvcencio/gotreesitter"
	"github.com/odvcencio/gotreesitter/grammars/kotlin"
)

var grammar = sync.OnceValue(func() *ts.Language { return kotlin.Language() })

func topLevel(tree *treesitter.Tree, kind string) []*treesitter.Node {
	var out []*treesitter.Node
	if tree.Root == nil {
		return nil
	}
	for _, c := range tree.Root.Children() {
		switch tree.Type(c) {
		case kind:
			out = append(out, c)
		case "import_list":
			for _, h := range c.Children() {
				if tree.Type(h) == kind {
					out = append(out, h)
				}
			}
		}
	}
	return out
}

func dotted(tree *treesitter.Tree, n *treesitter.Node) string {
	var segs []string
	for _, c := range n.Children() {
		if tree.Type(c) == "simple_identifier" {
			segs = append(segs, strings.Trim(tree.Dotted(c), "`"))
		}
	}
	return strings.Join(segs, ".")
}

func identifier(tree *treesitter.Tree, n *treesitter.Node) (string, bool) {
	for _, c := range n.Children() {
		if tree.Type(c) == "identifier" {
			return dotted(tree, c), true
		}
	}
	return "", false
}

func ParseImports(content []byte) []string {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	set := make(map[string]struct{})
	for _, h := range topLevel(tree, "import_header") {
		name, ok := identifier(tree, h)
		if !ok {
			continue
		}
		onDemand := false
		for _, c := range h.Children() {
			if tree.Type(c) == "wildcard_import" {
				onDemand = true
			}
		}
		if id := normalizeImport(name, onDemand); id != "" {
			set[id] = struct{}{}
		}
	}
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
	if tree.Root == nil {
		return "", nil
	}
	for _, h := range topLevel(tree, "package_header") {
		if name, ok := identifier(tree, h); ok {
			names = append(names, name)
		}
	}
	for _, c := range tree.Root.Children() {
		if tree.Type(c) != "package_header" && startsWithPackage(tree, c) {
			names = append(names, "")
		}
	}
	if len(names) > 1 {
		return "", errDuplicatePackage(names[0])
	}
	if len(names) == 0 {
		return "", nil
	}
	return names[0], nil
}

type duplicatePackageError struct{ first string }

func (e *duplicatePackageError) Error() string {
	return "kotlin: duplicate package clause " + e.first
}

func errDuplicatePackage(first string) error { return &duplicatePackageError{first: first} }

func startsWithPackage(tree *treesitter.Tree, n *treesitter.Node) bool {
	for n.ChildCount() > 0 {
		n = n.Child(0)
	}
	return tree.Text(n) == "package"
}

func DefinesMain(content []byte) bool {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	var visit func(n *treesitter.Node, top, inObject bool) bool
	visit = func(n *treesitter.Node, top, inObject bool) bool {
		switch tree.Type(n) {
		case "ERROR":
			return false
		case "function_declaration":
			if isMain(tree, n) && (top || inObject && hasJvmStatic(tree, n)) {
				return true
			}
		case "object_declaration", "companion_object":
			inObject = true
		}
		childTop := tree.Type(n) == "source_file"
		for _, c := range n.Children() {
			if visit(c, childTop, inObject) {
				return true
			}
		}
		return false
	}
	return visit(tree.Root, false, false)
}

func isMain(tree *treesitter.Tree, fn *treesitter.Node) bool {
	name := false
	for _, c := range fn.Children() {
		switch tree.Type(c) {
		case "receiver_type":
			return false
		case "simple_identifier":
			name = name || tree.Text(c) == "main"
		}
	}
	return name
}

func hasJvmStatic(tree *treesitter.Tree, fn *treesitter.Node) bool {
	for _, c := range fn.Children() {
		if tree.Type(c) != "modifiers" {
			continue
		}
		for _, a := range c.Children() {
			if tree.Type(a) != "annotation" {
				continue
			}
			last := ""
			tree.Walk(a, func(n *treesitter.Node) bool {
				if tree.Type(n) == "type_identifier" {
					last = tree.Text(n)
				}
				return true
			})
			if last == "JvmStatic" {
				return true
			}
		}
	}
	return false
}

func normalizeImport(dotted string, onDemand bool) string {
	dotted = strings.TrimSpace(dotted)
	if dotted == "" || !onDemand || IsStdLib(dotted) {
		return dotted
	}
	return dotted + ".*"
}
