package scala

import (
	"bytes"
	"sort"
	"strings"
	"sync"

	"github.com/devhusdk/rules_dx/gazelle/common/treesitter"
	ts "github.com/odvcencio/gotreesitter"
	"github.com/odvcencio/gotreesitter/grammars/scala"
)

var grammar = sync.OnceValue(func() *ts.Language { return scala.Language() })

// named returns the dot-joined text of the named, non-comment children of n, or
// the text of n itself when it is a leaf. Dots are not nodes in this grammar.
func named(tree *treesitter.Tree, n *treesitter.Node) string {
	if n.ChildCount() == 0 {
		return strings.Trim(tree.Text(n), "`")
	}
	var segs []string
	for _, c := range n.Children() {
		if !c.IsNamed() || strings.Contains(tree.Type(c), "comment") {
			continue
		}
		segs = append(segs, named(tree, c))
	}
	return strings.Join(segs, ".")
}

// ParseImports returns the sorted, deduplicated import identities of content,
// including imports local to blocks. Imports before a syntax error are kept.
func ParseImports(content []byte) []string {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	set := make(map[string]struct{})
	add := func(id string) {
		if id != "" {
			set[id] = struct{}{}
		}
	}
	tree.Walk(tree.Root, func(n *treesitter.Node) bool {
		if tree.Type(n) != "import_declaration" {
			return true
		}
		if n.HasErrorOrMissing() || truncated(tree, n, content) {
			// A truncated or malformed import names nothing reliable.
			return false
		}
		var group []*treesitter.Node
		flush := func() {
			importerIDs(tree, group, add)
			group = nil
		}
		for _, c := range n.Children() {
			switch tree.Type(c) {
			case ",":
				flush()
			case "identifier", "namespace_selectors", "namespace_wildcard", "as_renamed_identifier", "arrow_renamed_identifier", "type_identifier", "generic_type":
				group = append(group, c)
			}
		}
		flush()
		return false
	})
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}

// truncated reports whether an error node directly follows the import on the
// same line, as in import e. cut off at the end of the file, or the import ends
// in a dangling dot. An error on a later line leaves the import intact.
func truncated(tree *treesitter.Tree, n *treesitter.Node, content []byte) bool {
	if kids := n.Children(); len(kids) > 0 && tree.Type(kids[len(kids)-1]) == "." {
		return true
	}
	next := n.NextSibling()
	if next == nil || !next.IsError() {
		return false
	}
	start, end := n.EndByte(), next.StartByte()
	if end < start || end > uint32(len(content)) {
		return false
	}
	return end-start <= 1 && !bytes.ContainsAny(content[start:end], "\r\n")
}

// importerIDs maps one comma-separated importer (a path of identifiers plus an
// optional selector, wildcard or rename) to import identities.
func importerIDs(tree *treesitter.Tree, group []*treesitter.Node, add func(string)) {
	var path []string
	var tail *treesitter.Node
	for _, c := range group {
		if tree.Type(c) == "identifier" {
			path = append(path, strings.Trim(tree.Text(c), "`"))
		} else {
			tail = c
		}
	}
	pkg := strings.Join(path, ".")
	if tail == nil {
		add(normalizeImport(pkg, false))
		return
	}
	switch tree.Type(tail) {
	case "namespace_wildcard", "type_identifier", "generic_type":
		// _, *, given and given T all import from the package on demand.
		add(normalizeImport(pkg, true))
	case "as_renamed_identifier", "arrow_renamed_identifier":
		renamed(tree, pkg, tail, add)
	case "namespace_selectors":
		for _, sel := range tail.Children() {
			switch tree.Type(sel) {
			case "identifier":
				add(normalizeImport(joinDot(pkg, strings.Trim(tree.Text(sel), "`")), false))
			case "as_renamed_identifier", "arrow_renamed_identifier":
				renamed(tree, pkg, sel, add)
			case "namespace_wildcard", "type_identifier", "generic_type":
				add(normalizeImport(pkg, true))
			}
		}
	}
}

// renamed handles X => Y and X as Y. A rename to _ hides X and imports nothing.
func renamed(tree *treesitter.Tree, pkg string, n *treesitter.Node, add func(string)) {
	var first string
	for _, c := range n.Children() {
		switch tree.Type(c) {
		case "identifier":
			if first == "" {
				first = strings.Trim(tree.Text(c), "`")
			}
		case "wildcard":
			return
		}
	}
	add(normalizeImport(joinDot(pkg, first), false))
}

func joinDot(a, b string) string {
	if a == "" {
		return b
	}
	return a + "." + b
}

// ParsePackage joins chained and nested package clauses (package a / package b,
// package a { package b {, package a; package b) with dots. A clause that
// follows other code is a separate package and is an error.
func ParsePackage(content []byte) (string, error) {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	if tree.Root == nil {
		return "", nil
	}
	total := 0
	var all []*treesitter.Node
	tree.Walk(tree.Root, func(n *treesitter.Node) bool {
		if tree.Type(n) == "package_clause" {
			total++
			all = append(all, n)
		}
		return true
	})
	if total == 0 {
		return "", nil
	}
	var parts []string
	consumed := map[*treesitter.Node]bool{}
	scope := tree.Root.Children()
	for len(scope) > 0 {
		c := scope[0]
		scope = scope[1:]
		if k := tree.Type(c); strings.Contains(k, "comment") || !c.IsNamed() {
			continue
		}
		if tree.Type(c) != "package_clause" {
			break
		}
		consumed[c] = true
		parts = append(parts, clauseName(tree, c))
		if body := child(tree, c, "template_body"); body != nil {
			scope = body.Children()
		}
	}
	name := strings.Join(parts, ".")
	for _, c := range all {
		if !consumed[c] {
			return "", errSeparatePackage(name, clauseName(tree, c))
		}
	}
	return name, nil
}

func child(tree *treesitter.Tree, n *treesitter.Node, kind string) *treesitter.Node {
	for _, c := range n.Children() {
		if tree.Type(c) == kind {
			return c
		}
	}
	return nil
}

func clauseName(tree *treesitter.Tree, clause *treesitter.Node) string {
	if id := child(tree, clause, "package_identifier"); id != nil {
		return named(tree, id)
	}
	return ""
}

type separatePackageError struct{ first, second string }

func (e *separatePackageError) Error() string {
	return "scala: separate package clauses " + e.first + ", " + e.second
}

func errSeparatePackage(first, second string) error {
	return &separatePackageError{first: first, second: second}
}

// DefinesMain reports whether content declares a def main, an @main def, or an
// object extending App.
func DefinesMain(content []byte) bool {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	found := false
	tree.Walk(tree.Root, func(n *treesitter.Node) bool {
		if found {
			return false
		}
		switch tree.Type(n) {
		case "function_definition", "function_declaration":
			for _, c := range n.Children() {
				switch tree.Type(c) {
				case "identifier":
					if tree.Text(c) == "main" {
						found = true
					}
				case "annotation":
					if t := child(tree, c, "type_identifier"); t != nil && tree.Text(t) == "main" {
						found = true
					}
				}
			}
		case "extends_clause":
			for _, c := range n.Children() {
				if k := tree.Type(c); k == "type_identifier" || k == "stable_type_identifier" {
					switch named(tree, c) {
					case "App", "scala.App", "_root_.scala.App":
						found = true
					}
					break
				}
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

