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

// ParseImports returns the sorted, deduplicated using targets of content.
// Static and alias usings fold type targets to their namespace.
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
					// For an alias the first identifier is the alias name.
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

// ParsePackage returns the namespace name, or an error when the file declares
// more than one namespace.
func ParsePackage(content []byte) (string, error) {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	var names []string
	tree.Walk(tree.Root, func(n *treesitter.Node) bool {
		switch tree.Type(n) {
		case "namespace_declaration", "file_scoped_namespace_declaration":
			for _, c := range n.Children() {
				if k := tree.Type(c); k == "identifier" || k == "qualified_name" {
					names = append(names, tree.Dotted(c))
					break
				}
			}
		case "block", "class_declaration", "struct_declaration", "method_declaration":
			return false
		}
		return true
	})
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
	return "csharp: duplicate namespace declaration " + e.first
}

func errDuplicatePackage(first string) error { return &duplicatePackageError{first: first} }

// DefinesMain reports whether content has a static Main method or top-level
// statements.
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
			// A stray ";" left by error recovery is not a statement.
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

// foldTypeTarget maps the target of a static or alias using to its namespace.
// Those forms may name a type, so a target that is not a platform namespace
// but whose parent is gets folded into the parent. A plain using always names
// a namespace and keeps exact matching.
func foldTypeTarget(dotted string) string {
	if IsStdLib(dotted) {
		return dotted
	}
	if i := strings.LastIndexByte(dotted, '.'); i >= 0 && IsStdLib(dotted[:i]) {
		return dotted[:i]
	}
	return dotted
}

// namespaceCandidates returns the parent namespace of an unmatched using
// target, so a static or alias using of a local type reaches the library
// that declares the type's namespace. It climbs one level only.
func namespaceCandidates(name string) []string {
	i := strings.LastIndexByte(name, '.')
	if i <= 0 {
		return nil
	}
	return []string{name[:i]}
}
