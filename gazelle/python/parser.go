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
// imported by content, including import_module("x") calls and module-level
// pytest_plugins entries. Relative imports
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
		addPytestPlugins(tree, add)
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

// pytestBuiltinPlugins mirrors builtin_plugins in _pytest/config/__init__.py.
// python_test already provides pytest, so these aliases need no dep.
var pytestBuiltinPlugins = map[string]bool{
	"mark": true, "main": true, "runner": true, "fixtures": true, "helpconfig": true,
	"python": true, "terminal": true, "debugging": true, "unittest": true, "capture": true,
	"skipping": true, "legacypath": true, "tmpdir": true, "monkeypatch": true, "recwarn": true,
	"pastebin": true, "assertion": true, "junitxml": true, "doctest": true, "cacheprovider": true,
	"setuponly": true, "setupplan": true, "stepwise": true, "unraisableexception": true,
	"threadexception": true, "warnings": true, "logging": true, "reports": true,
	"faulthandler": true, "subtests": true, "pytester": true, "pytester_assertions": true,
	"terminalprogress": true, "nose": true, "freeze_support": true,
}

// addPytestPlugins adds the static entries of module-level pytest_plugins
// assignments. Function- and class-local assignments are skipped.
func addPytestPlugins(tree *treesitter.Tree, add func(string)) {
	addName := func(name string) {
		name = strings.TrimSpace(name)
		if name == "" || strings.HasPrefix(name, ".") || pytestBuiltinPlugins[name] {
			return
		}
		add(name)
	}
	tree.Walk(tree.Root, func(n *ts.Node) bool {
		switch tree.Type(n) {
		case "function_definition", "class_definition", "lambda":
			return false
		case "assignment":
		default:
			return true
		}
		left := n.ChildByFieldName("left", grammar())
		if left == nil || tree.Type(left) != "identifier" || tree.Text(left) != "pytest_plugins" {
			return true
		}
		right := n.ChildByFieldName("right", grammar())
		for right != nil && tree.Type(right) == "assignment" {
			right = right.ChildByFieldName("right", grammar())
		}
		right = unparen(tree, right)
		if right == nil || right.HasError() {
			return true
		}
		switch tree.Type(right) {
		case "list", "tuple", "expression_list":
			for _, c := range right.Children() {
				if !c.IsNamed() {
					continue
				}
				if v, ok := pythonString(tree, c); ok {
					addName(v)
				}
			}
		default:
			if v, ok := pythonString(tree, right); ok {
				for _, name := range strings.Split(v, ",") {
					addName(name)
				}
			}
		}
		return true
	})
}

func unparen(tree *treesitter.Tree, n *ts.Node) *ts.Node {
	for n != nil && tree.Type(n) == "parenthesized_expression" {
		var inner *ts.Node
		for _, c := range n.Children() {
			if c.IsNamed() && tree.Type(c) != "comment" {
				inner = c
				break
			}
		}
		n = inner
	}
	return n
}

// pythonString returns the value of a plain, raw, or unicode string literal or
// an implicit concatenation of them. Bytes, f-strings, and escapes are rejected.
func pythonString(tree *treesitter.Tree, n *ts.Node) (string, bool) {
	n = unparen(tree, n)
	if n == nil || n.HasError() {
		return "", false
	}
	switch tree.Type(n) {
	case "concatenated_string":
		var b strings.Builder
		for _, c := range n.Children() {
			if !c.IsNamed() || tree.Type(c) == "comment" {
				continue
			}
			part, ok := pythonString(tree, c)
			if !ok {
				return "", false
			}
			b.WriteString(part)
		}
		return b.String(), true
	case "string":
	default:
		return "", false
	}
	s := tree.Text(n)
	quote := strings.IndexAny(s, `"'`)
	if quote < 0 {
		return "", false
	}
	prefix := strings.ToLower(s[:quote])
	if prefix != "" && prefix != "r" && prefix != "u" {
		return "", false
	}
	s = s[quote:]
	switch {
	case len(s) >= 6 && (strings.HasPrefix(s, `"""`) || strings.HasPrefix(s, `'''`)) && strings.HasSuffix(s, s[:3]):
		s = s[3 : len(s)-3]
	case len(s) >= 2 && s[len(s)-1] == s[0]:
		s = s[1 : len(s)-1]
	default:
		return "", false
	}
	if prefix != "r" && strings.Contains(s, `\`) {
		return "", false
	}
	return s, true
}
