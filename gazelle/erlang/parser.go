package erlang

import (
	"sort"
	"strings"
	"sync"

	"github.com/devhusdk/rules_dx/gazelle/common/treesitter"
	ts "github.com/odvcencio/gotreesitter"
	"github.com/odvcencio/gotreesitter/grammars/erlang"
)

var grammar = sync.OnceValue(func() *ts.Language { return erlang.Language() })

// Refs holds what one Erlang file references.
type Refs struct {
	// Modules are modules named by remote calls, fun M:F/A, -behaviour,
	// -import and -parse_transform. Variables and macros never appear.
	Modules []string
	// Apps are the first path segment of each -include_lib.
	Apps []string
	// NifNames are the base names of literal paths given to erlang:load_nif.
	NifNames []string
}

// ParseRefs reads the references of one .erl or .hrl file. Text inside
// comments and strings never counts, and references before a syntax error are
// kept.
func ParseRefs(content []byte) Refs {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	modules, apps, nifs := set{}, set{}, set{}
	if tree.Root != nil {
		tree.Walk(tree.Root, func(n *treesitter.Node) bool {
			switch tree.Type(n) {
			case "behaviour_attribute", "import_attribute":
				if a := firstAtom(tree, n); a != "" {
					modules.add(a)
				}
			case "compile_options_attribute":
				tree.Walk(n, func(c *treesitter.Node) bool {
					if tree.Type(c) != "tuple" {
						return true
					}
					var atoms []string
					for _, k := range c.Children() {
						if tree.Type(k) == "atom" {
							atoms = append(atoms, atomText(tree.Text(k)))
						}
					}
					if len(atoms) == 2 && atoms[0] == "parse_transform" {
						modules.add(atoms[1])
					}
					return true
				})
			case "pp_include_lib":
				for _, c := range n.Children() {
					if tree.Type(c) == "string" {
						lit := strings.Trim(tree.Text(c), `"`)
						if app, _, ok := strings.Cut(lit, "/"); ok {
							apps.add(app)
						}
					}
				}
			case "remote":
				remote(tree, n, modules, nifs)
			case "external_fun":
				for _, c := range n.Children() {
					if tree.Type(c) == "module" {
						modules.add(firstAtom(tree, c))
					}
				}
			}
			return true
		})
	}
	return Refs{Modules: modules.sorted(), Apps: apps.sorted(), NifNames: nifs.sorted()}
}

func remote(tree *treesitter.Tree, n *treesitter.Node, modules, nifs set) {
	module, fun := "", ""
	var args *treesitter.Node
	for _, c := range n.Children() {
		switch tree.Type(c) {
		case "remote_module":
			module = firstAtom(tree, c)
		case "call":
			for _, k := range c.Children() {
				switch tree.Type(k) {
				case "atom":
					fun = atomText(tree.Text(k))
				case "expr_args":
					args = k
				}
			}
		}
	}
	if module == "" {
		return
	}
	modules.add(module)
	if module == "erlang" && fun == "load_nif" && args != nil {
		last := ""
		for _, c := range args.Children() {
			if tree.Type(c) == "," {
				break
			}
			tree.Walk(c, func(k *treesitter.Node) bool {
				if tree.Type(k) == "string" {
					last = strings.Trim(tree.Text(k), `"`)
				}
				return true
			})
		}
		if i := strings.LastIndexByte(last, '/'); i >= 0 {
			last = last[i+1:]
		}
		nifs.add(last)
	}
}

// firstAtom returns the first direct atom child of n, or "" when it is not a
// plain module name.
func firstAtom(tree *treesitter.Tree, n *treesitter.Node) string {
	for _, c := range n.Children() {
		if tree.Type(c) == "atom" {
			return atomText(tree.Text(c))
		}
	}
	return ""
}

func atomText(s string) string { return strings.Trim(s, "'") }

type set map[string]struct{}

func (s set) add(v string) {
	if atomPattern.MatchString(v) {
		s[v] = struct{}{}
	}
}

func (s set) sorted() []string {
	out := make([]string, 0, len(s))
	for v := range s {
		out = append(out, v)
	}
	sort.Strings(out)
	return out
}
