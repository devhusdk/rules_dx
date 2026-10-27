package java

import (
	"sort"
	"strings"
	"sync"

	"github.com/devhusdk/rules_dx/gazelle/common/treesitter"
	ts "github.com/odvcencio/gotreesitter"
	"github.com/odvcencio/gotreesitter/grammars/java"
)

var grammar = sync.OnceValue(func() *ts.Language { return java.Language() })

// ParseImports returns the sorted, deduplicated import identities of content.
// Imports before a syntax error are kept.
func ParseImports(content []byte) []string {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	set := make(map[string]struct{})
	if tree.Root != nil {
		for _, decl := range tree.Root.Children() {
			if tree.Type(decl) != "import_declaration" {
				continue
			}
			if isModuleImport(tree.Text(decl)) {
				continue
			}
			var dotted string
			var static, onDemand bool
			for _, c := range decl.Children() {
				switch tree.Type(c) {
				case "identifier", "scoped_identifier":
					dotted = tree.Dotted(c)
				case "asterisk":
					onDemand = true
				case "static":
					static = true
				}
			}
			if id := importIdentity(dotted, static, onDemand); id != "" {
				set[id] = struct{}{}
			}
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
	if tree.Root != nil {
		for _, decl := range tree.Root.Children() {
			if tree.Type(decl) != "package_declaration" {
				continue
			}
			for _, c := range decl.Children() {
				if k := tree.Type(c); k == "identifier" || k == "scoped_identifier" {
					names = append(names, tree.Dotted(c))
				}
			}
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
	return "java: duplicate package clause " + e.first
}

func errDuplicatePackage(first string) error { return &duplicatePackageError{first: first} }

// DefinesMain reports whether content declares a static void main method.
func DefinesMain(content []byte) bool {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	found := false
	tree.Walk(tree.Root, func(n *treesitter.Node) bool {
		if found {
			return false
		}
		if tree.Type(n) != "method_declaration" {
			return true
		}
		var name string
		var static, void bool
		for _, c := range n.Children() {
			switch tree.Type(c) {
			case "identifier":
				name = tree.Text(c)
			case "void_type":
				void = true
			case "modifiers":
				for _, m := range c.Children() {
					if tree.Type(m) == "static" {
						static = true
					}
				}
			}
		}
		found = name == "main" && static && void
		return !found
	})
	return found
}

func importIdentity(dotted string, static, onDemand bool) string {
	dotted = strings.TrimSpace(dotted)
	if !static {
		if onDemand && !IsStdLib(dotted) {
			return dotted + ".*"
		}
		return dotted
	}
	if IsStdLib(dotted) {
		return dotted
	}
	if onDemand {
		return dotted
	}
	i := strings.LastIndexByte(dotted, '.')
	if i < 0 {
		return ""
	}
	return dotted[:i]
}

// isModuleImport reports whether an import declaration is the Java 25 module
// import form (import module java.base;), which names a module, not a class.
// A package that happens to be called module (import module.Foo;) is not one.
func isModuleImport(decl string) bool {
	rest := strings.TrimLeft(strings.TrimPrefix(decl, "import"), " \t\r\n")
	if !strings.HasPrefix(rest, "module") {
		return false
	}
	rest = rest[len("module"):]
	trimmed := strings.TrimLeft(rest, " \t\r\n")
	return len(trimmed) < len(rest) && !strings.HasPrefix(trimmed, ".")
}
