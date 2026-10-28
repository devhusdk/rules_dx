package ruby

import (
	"path"
	"sort"
	"strings"
	"sync"

	"github.com/devhusdk/rules_dx/gazelle/common/treesitter"
	ts "github.com/odvcencio/gotreesitter"
	"github.com/odvcencio/gotreesitter/grammars/ruby"
)

var grammar = sync.OnceValue(func() *ts.Language { return ruby.Language() })

// ParseImports returns the require specs of content. rel is the repo-relative
// directory of the file; require_relative specs resolve against it into
// "//<repo path>" keys (see RelativeKey).
func ParseImports(content []byte, rel string) []string {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	set := make(map[string]struct{})
	tree.Walk(tree.Root, func(n *treesitter.Node) bool {
		kind := tree.Type(n)
		if kind != "call" && kind != "method_call" {
			return true
		}
		name, req, ok := requireCall(tree, n)
		if !ok {
			return true
		}
		if name == "require_relative" {
			if id := relativeImport(rel, req); id != "" {
				set[id] = struct{}{}
			}
		} else if !strings.HasPrefix(req, ".") {
			if id := normalizeImport(req); id != "" {
				set[id] = struct{}{}
			}
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

// requireCall matches a receiver-less require/require_relative call with a
// single literal string argument.
func requireCall(tree *treesitter.Tree, n *treesitter.Node) (name, spec string, ok bool) {
	kids := n.Children()
	if len(kids) < 2 || tree.Type(kids[0]) != "identifier" {
		return "", "", false
	}
	name = tree.Text(kids[0])
	if name != "require" && name != "require_relative" {
		return "", "", false
	}
	var args *treesitter.Node
	for _, c := range kids[1:] {
		if tree.Type(c) == "argument_list" {
			args = c
		}
	}
	if args == nil {
		return "", "", false
	}
	for _, a := range args.Children() {
		if tree.Type(a) != "string" {
			continue
		}
		var parts []string
		for _, c := range a.Children() {
			switch tree.Type(c) {
			case "string_content":
				parts = append(parts, tree.Text(c))
			case "\"", "'":
			default:
				return "", "", false
			}
		}
		spec = strings.TrimSpace(strings.Join(parts, ""))
		return name, spec, spec != ""
	}
	return "", "", false
}

// relativeImport resolves a require_relative spec to its RelativeKey. It
// returns "" for absolute specs, specs escaping the repo, and specs in the
// same directory: generation is SingleDir, so same-dir files share one library.
func relativeImport(rel, spec string) string {
	if strings.HasPrefix(spec, "/") {
		return ""
	}
	id := path.Clean(path.Join(rel, strings.TrimSuffix(spec, ".rb")))
	if id == "." || id == ".." || strings.HasPrefix(id, "../") || path.Dir(id) == path.Clean(rel) {
		return ""
	}
	return "//" + id
}

func ParsePackage(content []byte) (string, error) {
	return "", nil
}

func normalizeImport(req string) string {
	req = strings.TrimSpace(req)
	req = strings.TrimSuffix(req, ".rb")
	return strings.TrimSuffix(req, ".so")
}

// DefinesMain reports whether content has a `__FILE__ == $0` (or
// $PROGRAM_NAME) guard.
func DefinesMain(content []byte) bool {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	found := false
	tree.Walk(tree.Root, func(n *treesitter.Node) bool {
		if found {
			return false
		}
		if tree.Type(n) != "binary" {
			return true
		}
		var parts []string
		for _, c := range n.Children() {
			parts = append(parts, tree.Text(c))
		}
		found = len(parts) == 3 && parts[0] == "__FILE__" && parts[1] == "==" &&
			(parts[2] == "$0" || parts[2] == "$PROGRAM_NAME")
		return !found
	})
	return found
}
