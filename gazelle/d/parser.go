package d

import (
	"fmt"
	"regexp"
	"sort"
	"strings"
	"sync"

	"github.com/devhusdk/rules_dx/gazelle/common"
	"github.com/devhusdk/rules_dx/gazelle/common/treesitter"
	ts "github.com/odvcencio/gotreesitter"
	"github.com/odvcencio/gotreesitter/grammars/d"
)

var grammar = sync.OnceValue(func() *ts.Language { return d.Language() })

var (
	makeDepLine   = regexp.MustCompile(`^[^\s:;(){}"'#]+\.\w+:(\s|$)`)
	dtraceProvide = regexp.MustCompile(`(?m)^provider\s+\w+\s*\{`)
)

// IsForeign reports whether a .d file is not D source: a Make dependency
// file or a DTrace script. Only these explicit facts skip a file; any other
// file that fails to parse is an error.
func IsForeign(content []byte) bool {
	content = common.StripBOM(content)
	text := string(content)
	first, _, _ := strings.Cut(text, "\n")
	first = strings.TrimRight(first, "\r")
	if strings.HasPrefix(first, "#!") && strings.Contains(first, "dtrace") {
		return true
	}
	if strings.Contains(text, "#pragma D ") || strings.HasPrefix(text, "#pragma D") {
		return true
	}
	if dtraceProvide.MatchString(text) {
		return true
	}
	return !strings.HasPrefix(first, "//") && !strings.HasPrefix(first, "/*") && !strings.HasPrefix(first, "/+") && makeDepLine.MatchString(first)
}

// Info holds the facts read from one D source file.
type Info struct {
	// Module is the declared module name, or "" without a declaration.
	Module string
	// Imports lists the imported modules, sorted and deduplicated.
	Imports []string
	// StringImports lists the literal import("file") arguments.
	StringImports []string
	// StringImportErr is set when an import(expr) argument is not a literal.
	StringImportErr error
	// DefinesMain reports a module-scope main function.
	DefinesMain bool
	// HasUnittest reports a unittest block.
	HasUnittest bool
}

// Analyze parses content. A syntax error fails with the line of the first
// error node, so malformed input is never taken for a clean file.
func Analyze(content []byte) (Info, error) {
	content = common.StripBOM(content)
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	if tree.Root == nil {
		return Info{}, fmt.Errorf("syntax error: no parse tree")
	}
	if tree.HasError {
		return Info{}, fmt.Errorf("syntax error at line %d", firstErrorLine(tree))
	}
	var info Info
	imports := make(map[string]bool)
	strs := make(map[string]bool)
	tree.Walk(tree.Root, func(n *treesitter.Node) bool {
		switch tree.Type(n) {
		case "token_string", "mixin_expression":
			return false
		case "module_declaration":
			if info.Module == "" {
				info.Module = fqn(tree, n)
			}
			return false
		case "import_declaration":
			for _, c := range n.Children() {
				if tree.Type(c) == "imported" {
					if name := importedName(tree, c); name != "" {
						imports[name] = true
					}
				}
			}
			return false
		case "import_expression":
			lit, ok := stringLiteral(tree, n)
			switch {
			case ok:
				strs[lit] = true
			case info.StringImportErr == nil:
				info.StringImportErr = fmt.Errorf("import(%s) at line %d is not a string literal", argText(tree, n), n.StartPoint().Row+1)
			}
			return false
		case "unittest_declaration":
			info.HasUnittest = true
		}
		return true
	})
	info.Imports = sortedKeys(imports)
	info.StringImports = sortedKeys(strs)
	info.DefinesMain = definesMain(tree, tree.Root)
	return info, nil
}

func sortedKeys(set map[string]bool) []string {
	out := make([]string, 0, len(set))
	for k := range set {
		out = append(out, k)
	}
	sort.Strings(out)
	return out
}

func firstErrorLine(tree *treesitter.Tree) int {
	line := 0
	tree.Walk(tree.Root, func(n *treesitter.Node) bool {
		if line != 0 {
			return false
		}
		if n.IsError() || n.IsMissing() {
			line = int(n.StartPoint().Row) + 1
			return false
		}
		return true
	})
	if line == 0 {
		line = 1
	}
	return line
}

func fqn(tree *treesitter.Tree, n *treesitter.Node) string {
	for _, c := range n.Children() {
		if tree.Type(c) == "module_fqn" {
			return tree.Dotted(c)
		}
	}
	return ""
}

func importedName(tree *treesitter.Tree, n *treesitter.Node) string {
	if name := fqn(tree, n); name != "" {
		return name
	}
	text := tree.Dotted(n)
	if _, after, ok := strings.Cut(text, "="); ok {
		text = after
	}
	return text
}

func argText(tree *treesitter.Tree, n *treesitter.Node) string {
	text := tree.Text(n)
	text = strings.TrimPrefix(text, "import")
	text = strings.TrimSpace(text)
	return strings.TrimSuffix(strings.TrimPrefix(text, "("), ")")
}

// stringLiteral returns the value of a plain string literal argument of an
// import expression. Escapes, concatenation and suffixes are not literal.
func stringLiteral(tree *treesitter.Tree, n *treesitter.Node) (string, bool) {
	arg := strings.TrimSpace(argText(tree, n))
	if len(arg) < 2 || strings.ContainsAny(arg, "\\\n") {
		return "", false
	}
	var body string
	switch {
	case arg[0] == '"' && arg[len(arg)-1] == '"':
		body = arg[1 : len(arg)-1]
		if strings.Contains(body, `"`) {
			return "", false
		}
	case strings.HasPrefix(arg, `r"`) && arg[len(arg)-1] == '"':
		body = arg[2 : len(arg)-1]
		if strings.Contains(body, `"`) {
			return "", false
		}
	case arg[0] == '`' && arg[len(arg)-1] == '`':
		body = arg[1 : len(arg)-1]
		if strings.Contains(body, "`") {
			return "", false
		}
	default:
		return "", false
	}
	return body, body != ""
}

// skipScope holds the nodes whose contents are not module scope.
var skipScope = map[string]bool{
	"aggregate_body":       true,
	"function_body":        true,
	"unittest_declaration": true,
	"mixin_expression":     true,
	"mixin_declaration":    true,
	"template_declaration": true,
	"token_string":         true,
}

func definesMain(tree *treesitter.Tree, root *treesitter.Node) bool {
	found := false
	tree.Walk(root, func(n *treesitter.Node) bool {
		if found {
			return false
		}
		kind := tree.Type(n)
		if skipScope[kind] {
			return false
		}
		if kind == "function_declaration" {
			for _, c := range n.Children() {
				if tree.Type(c) == "identifier" {
					found = tree.Text(c) == "main"
					break
				}
			}
			return false
		}
		return true
	})
	return found
}

// ParseModule returns the declared module name, or "" when the file has no
// module declaration or does not parse.
func ParseModule(content []byte) string {
	info, _ := Analyze(content)
	return info.Module
}

// ParseImports returns the sorted imported modules of content.
func ParseImports(content []byte) []string {
	info, _ := Analyze(content)
	return info.Imports
}

// ParseStringImports returns the literal import("file") arguments, and an
// error when any import expression is not a literal.
func ParseStringImports(content []byte) ([]string, error) {
	info, err := Analyze(content)
	if err != nil {
		return nil, err
	}
	return info.StringImports, info.StringImportErr
}

// DefinesMain reports a module-scope main function. Aggregate members and
// function bodies never count.
func DefinesMain(content []byte) bool {
	info, _ := Analyze(content)
	return info.DefinesMain
}

// HasUnittest reports whether content has a unittest block.
func HasUnittest(content []byte) bool {
	info, _ := Analyze(content)
	return info.HasUnittest
}
