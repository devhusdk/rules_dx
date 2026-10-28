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

// topLevel returns the top-level nodes of the given type, looking inside
// import_list for import headers.
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

// dotted joins the simple identifier segments of an identifier node with dots,
// dropping comments and the backticks around quoted segments.
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

// ParseImports returns the sorted, deduplicated import identities of content.
// Imports before a syntax error are kept. Several imports on one line are all
// recognized.
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

// ParsePackage returns the package name, or an error when the file declares
// more than one package.
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
	// The grammar allows one package header; a second clause is not a node of
	// its own and parses as an expression that starts with the identifier package.
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

// DefinesMain reports whether content declares a function named main at any depth.
func DefinesMain(content []byte) bool {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	found := false
	tree.Walk(tree.Root, func(n *treesitter.Node) bool {
		if found {
			return false
		}
		if tree.Type(n) != "function_declaration" {
			return true
		}
		for _, c := range n.Children() {
			if tree.Type(c) == "simple_identifier" {
				found = tree.Text(c) == "main"
				break
			}
		}
		return !found
	})
	return found
}

func normalizeImport(dotted string, onDemand bool) string {
	dotted = strings.TrimSpace(dotted)
	if dotted == "" || !onDemand || IsStdLib(dotted) {
		return dotted
	}
	return dotted + ".*"
}
