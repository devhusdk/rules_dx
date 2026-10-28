package php

import (
	"bytes"
	"sort"
	"strings"
	"sync"

	"github.com/devhusdk/rules_dx/gazelle/common/treesitter"
	ts "github.com/odvcencio/gotreesitter"
	"github.com/odvcencio/gotreesitter/grammars/php"
)

var grammar = sync.OnceValue(func() *ts.Language { return php.Language() })

// reserved are the names that never refer to a user declaration when they
// stand alone in a class position.
var reserved = map[string]bool{
	"self": true, "static": true, "parent": true,
	"array": true, "callable": true, "bool": true, "boolean": true, "int": true, "integer": true,
	"float": true, "double": true, "string": true, "iterable": true, "object": true, "mixed": true,
	"never": true, "void": true, "null": true, "false": true, "true": true,
}

// Parse returns the sorted, deduplicated fully qualified names content
// declares at the top level of a file or namespace body, and the names it
// refers to. Names carry no leading backslash. Classes, interfaces, traits and
// enums are declared by their full name; functions and constants are declared
// by their namespaced name too.
//
// References come from use clauses of every kind and from the class positions
// of the code (extends, implements, trait use, new, static access, instanceof,
// catch, type declarations, attributes) plus every qualified name. They are
// resolved with PHP's lexical rules: the current namespace and the use aliases
// of the file. Unqualified function and constant names are skipped because PHP
// falls back to the global one, so the target is unknowable. Strings,
// heredocs, comments, inline HTML and require/include are never read.
// Declarations before a syntax error are kept.
func Parse(content []byte) (declares, imports []string) {
	content = bytes.TrimPrefix(content, []byte("\xef\xbb\xbf"))
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	p := &parser{tree: tree, decl: map[string]struct{}{}, imps: map[string]struct{}{}}
	p.resetScope("")
	if tree.Root != nil {
		for _, c := range tree.Root.Children() {
			p.statement(c)
		}
	}
	return keys(p.decl), keys(p.imps)
}

type parser struct {
	tree    *treesitter.Tree
	ns      string
	aliases map[string]string
	decl    map[string]struct{}
	imps    map[string]struct{}
}

func keys(m map[string]struct{}) []string {
	out := make([]string, 0, len(m))
	for k := range m {
		out = append(out, k)
	}
	sort.Strings(out)
	return out
}

func (p *parser) resetScope(ns string) {
	p.ns = ns
	p.aliases = map[string]string{}
}

func (p *parser) qualify(name string) string {
	if p.ns == "" {
		return name
	}
	return p.ns + `\` + name
}

// statement handles a node at the top level of a file or namespace body.
func (p *parser) statement(n *treesitter.Node) {
	switch p.tree.Type(n) {
	case "namespace_definition":
		var ns string
		var body *treesitter.Node
		for _, c := range n.Children() {
			switch p.tree.Type(c) {
			case "namespace_name":
				ns = p.tree.Dotted(c)
			case "compound_statement":
				body = c
			}
		}
		if body == nil {
			p.resetScope(ns)
			return
		}
		outer, outerAliases := p.ns, p.aliases
		p.resetScope(ns)
		for _, c := range body.Children() {
			p.statement(c)
		}
		p.ns, p.aliases = outer, outerAliases
	case "class_declaration", "interface_declaration", "trait_declaration", "enum_declaration":
		p.declareNamed(n)
		p.visit(n)
	case "function_definition":
		p.declareNamed(n)
		p.visit(n)
	case "const_declaration":
		for _, c := range n.Children() {
			if p.tree.Type(c) == "const_element" {
				for _, e := range c.Children() {
					if p.tree.Type(e) == "name" {
						p.decl[p.qualify(p.tree.Text(e))] = struct{}{}
						break
					}
				}
			}
		}
		p.visit(n)
	default:
		p.visit(n)
	}
}

func (p *parser) declareNamed(n *treesitter.Node) {
	for _, c := range n.Children() {
		if p.tree.Type(c) == "name" {
			p.decl[p.qualify(p.tree.Text(c))] = struct{}{}
			return
		}
	}
}

// visit finds references below n.
func (p *parser) visit(n *treesitter.Node) {
	p.tree.Walk(n, func(c *treesitter.Node) bool {
		switch p.tree.Type(c) {
		case "namespace_use_declaration":
			p.useDeclaration(c)
			return false
		case "qualified_name", "relative_name":
			p.ref(p.tree.Dotted(c))
			return false
		case "base_clause", "class_interface_clause", "use_declaration", "named_type", "object_creation_expression", "attribute":
			p.nameChildren(c, -1)
		case "scoped_call_expression", "scoped_property_access_expression", "class_constant_access_expression":
			p.nameChildren(c, 1)
		case "binary_expression":
			kids := c.Children()
			for i := 0; i+1 < len(kids); i++ {
				if p.tree.Type(kids[i]) == "instanceof" && p.tree.Type(kids[i+1]) == "name" {
					p.ref(p.tree.Text(kids[i+1]))
				}
			}
		}
		return true
	})
}

// nameChildren resolves the plain name children of n. A limit above zero
// stops after that many children, which keeps a member name such as the
// constant in Foo::BAR out.
func (p *parser) nameChildren(n *treesitter.Node, limit int) {
	for i, c := range n.Children() {
		if limit > 0 && i >= limit {
			return
		}
		if p.tree.Type(c) == "name" {
			p.ref(p.tree.Text(c))
		}
		if limit > 0 {
			return
		}
	}
}

// ref resolves a name written in code and records it.
func (p *parser) ref(raw string) {
	name := strings.TrimSpace(raw)
	switch {
	case name == "":
		return
	case strings.HasPrefix(name, `\`):
		name = name[1:]
	case strings.HasPrefix(name, `namespace\`):
		name = p.qualify(name[len(`namespace\`):])
	default:
		first, rest, _ := strings.Cut(name, `\`)
		if rest == "" && reserved[strings.ToLower(first)] {
			return
		}
		if target, ok := p.aliases[strings.ToLower(first)]; ok {
			// A class use clause is only an import when its alias is used
			// as a whole name; as a namespace prefix it may name no class.
			name = target
			if rest != "" {
				name += `\` + rest
			}
		} else {
			name = p.qualify(name)
		}
	}
	if name != "" {
		p.imps[name] = struct{}{}
	}
}

// useDeclaration records the names of one use statement and the aliases of
// its class clauses.
func (p *parser) useDeclaration(n *treesitter.Node) {
	kind := ""
	prefix := ""
	for _, c := range n.Children() {
		switch t := p.tree.Type(c); t {
		case "function", "const":
			kind = t
		case "namespace_name":
			prefix = strings.TrimPrefix(p.tree.Dotted(c), `\`) + `\`
		case "namespace_use_clause":
			p.useClause(c, kind, "")
		case "namespace_use_group":
			for _, g := range c.Children() {
				if p.tree.Type(g) == "namespace_use_clause" {
					p.useClause(g, kind, prefix)
				}
			}
		}
	}
}

func (p *parser) useClause(n *treesitter.Node, kind, prefix string) {
	var full, alias string
	seenAs := false
	for _, c := range n.Children() {
		switch t := p.tree.Type(c); t {
		case "function", "const":
			kind = t
		case "as":
			seenAs = true
		case "name":
			if seenAs {
				alias = p.tree.Text(c)
			} else {
				full = p.tree.Text(c)
			}
		case "qualified_name", "namespace_name":
			full = p.tree.Dotted(c)
		}
	}
	full = prefix + strings.TrimPrefix(full, `\`)
	if full == "" || full == prefix {
		return
	}
	if kind != "" {
		p.imps[full] = struct{}{}
		return
	}
	if alias == "" {
		alias = full[strings.LastIndex(full, `\`)+1:]
	}
	p.aliases[strings.ToLower(alias)] = full
}
