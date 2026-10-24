package scala

import (
	"fmt"
	"os"

	"github.com/devhusdk/rules_dx/gazelle/common"
	"github.com/devhusdk/rules_dx/gazelle/jvmsyntax"

	ts "github.com/odvcencio/gotreesitter"
	scalagrammar "github.com/odvcencio/gotreesitter/grammars/scala"
)

var grammar = jvmsyntax.NewGrammar("scala", scalagrammar.Language)

var declarations = map[string]bool{
	"class_definition":     true,
	"object_definition":    true,
	"trait_definition":     true,
	"enum_definition":      true,
	"function_definition":  true,
	"function_declaration": true,
	"val_definition":       true,
	"val_declaration":      true,
	"var_definition":       true,
	"var_declaration":      true,
	"type_definition":      true,
	"given_definition":     true,
	"package_object":       true,
	"extension_definition": true,
}

var header = map[string]bool{
	"comment":            true,
	"block_comment":      true,
	"package_clause":     true,
	"import_declaration": true,
	"export_declaration": true,
}

func ParseSource(content []byte) (common.SourceFacts, error) {
	var facts common.SourceFacts
	tree, err := grammar.Parse(content)
	if err != nil {
		return facts, err
	}
	pkg, body, err := packageOf(tree)
	if err != nil {
		return facts, err
	}
	facts.Package = pkg
	imports := make(map[string]bool)
	tree.Walk(tree.Root, func(n *ts.Node) bool {
		if tree.Type(n) != "import_declaration" {
			return true
		}
		for _, name := range importIdentities(tree, n) {
			imports[name] = true
		}
		return false
	})
	for _, c := range tree.Children(body) {
		if !declarations[tree.Type(c)] {
			continue
		}
		for _, name := range declaredNames(tree, c) {
			facts.Declared = append(facts.Declared, jvmsyntax.Qualify(pkg, name))
		}
	}
	facts.Imports = jvmsyntax.Sorted(imports)
	facts.Main = definesMain(tree)
	return facts, nil
}

func packageOf(tree *jvmsyntax.Tree) (string, *ts.Node, error) {
	stop := func(n *ts.Node) bool {
		typ := tree.Type(n)
		return declarations[typ] || (typ == "package_clause" && tree.Field(n, "body") != nil)
	}
	var parts []string
	scope := tree.Root
	for {
		if err := tree.HeaderError(scope, stop, header); err != nil {
			return "", nil, err
		}
		var nested *ts.Node
		var nestedName string
		for _, c := range tree.Children(scope) {
			if tree.Type(c) != "package_clause" {
				continue
			}
			if nested != nil {
				return "", nil, fmt.Errorf("scala: duplicate package clause %s", nestedName)
			}
			parts = append(parts, packageName(tree, c))
			if body := tree.Field(c, "body"); body != nil {
				nested = body
				nestedName = jvmsyntax.Join(parts)
			}
		}
		if nested == nil {
			return jvmsyntax.Join(parts), scope, nil
		}
		scope = nested
	}
}

func packageName(tree *jvmsyntax.Tree, n *ts.Node) string {
	name := tree.Field(n, "name")
	if name == nil {
		return ""
	}
	return jvmsyntax.Join(tree.Leaves(name, "identifier"))
}

func importIdentities(tree *jvmsyntax.Tree, n *ts.Node) []string {
	var out []string
	var path []string
	flush := func() {
		if len(path) > 0 {
			out = append(out, jvmsyntax.Join(trimRoot(path)))
		}
		path = nil
	}
	for i, c := range tree.Children(n) {
		switch typ := tree.Type(c); {
		case tree.FieldOf(n, i) == "path" && typ == "identifier":
			path = append(path, jvmsyntax.Unquote(tree.Text(c)))
		case typ == ",":
			flush()
		case typ == "namespace_wildcard":
			out = append(out, jvmsyntax.Join(trimRoot(path)))
			path = nil
		case typ == "namespace_selectors":
			out = append(out, selectorIdentities(tree, c, trimRoot(path))...)
			path = nil
		case typ == "as_renamed_identifier" || typ == "arrow_renamed_identifier":
			out = append(out, renamedIdentity(tree, c, trimRoot(path)))
			path = nil
		}
	}
	flush()
	return out
}

func selectorIdentities(tree *jvmsyntax.Tree, n *ts.Node, base []string) []string {
	var out []string
	for _, c := range tree.Children(n) {
		switch tree.Type(c) {
		case "identifier":
			out = append(out, jvmsyntax.Join(append(append([]string{}, base...), jvmsyntax.Unquote(tree.Text(c)))))
		case "namespace_wildcard":
			out = append(out, jvmsyntax.Join(base))
		case "as_renamed_identifier", "arrow_renamed_identifier":
			out = append(out, renamedIdentity(tree, c, base))
		}
	}
	return out
}

func renamedIdentity(tree *jvmsyntax.Tree, n *ts.Node, base []string) string {
	name := tree.Field(n, "name")
	if name == nil {
		return jvmsyntax.Join(base)
	}
	return jvmsyntax.Join(append(append([]string{}, base...), jvmsyntax.Unquote(tree.Text(name))))
}

func trimRoot(path []string) []string {
	if len(path) > 0 && path[0] == "_root_" {
		return path[1:]
	}
	return path
}

func declaredNames(tree *jvmsyntax.Tree, n *ts.Node) []string {
	switch tree.Type(n) {
	case "val_definition", "val_declaration", "var_definition", "var_declaration":
		if pattern := tree.Field(n, "pattern"); pattern != nil {
			return tree.Leaves(pattern, "identifier")
		}
		if name := tree.Field(n, "name"); name != nil {
			return []string{jvmsyntax.Unquote(tree.Text(name))}
		}
	case "extension_definition":
		return nil
	default:
		if name := tree.Field(n, "name"); name != nil {
			return []string{jvmsyntax.Unquote(tree.Text(name))}
		}
	}
	return nil
}

func definesMain(tree *jvmsyntax.Tree) bool {
	found := false
	tree.Walk(tree.Root, func(n *ts.Node) bool {
		if found {
			return false
		}
		if tree.Type(n) == "function_definition" {
			if name := tree.Field(n, "name"); name != nil && jvmsyntax.Unquote(tree.Text(name)) == "main" {
				found = true
			}
		}
		return true
	})
	return found
}

func qualifiedIDs(path string) []string {
	content, err := os.ReadFile(path)
	if err != nil {
		return nil
	}
	facts, err := ParseSource(content)
	if err != nil {
		return nil
	}
	return facts.Declared
}
