package kotlin

import (
	"os"

	"github.com/devhusdk/rules_dx/gazelle/common"
	"github.com/devhusdk/rules_dx/gazelle/jvmsyntax"

	ts "github.com/odvcencio/gotreesitter"
	kotlingrammar "github.com/odvcencio/gotreesitter/grammars/kotlin"
)

var grammar = jvmsyntax.NewGrammar("kotlin", kotlingrammar.Language)

var declarations = map[string]bool{
	"class_declaration":    true,
	"object_declaration":   true,
	"function_declaration": true,
	"property_declaration": true,
	"type_alias":           true,
}

func ParseSource(content []byte) (common.SourceFacts, error) {
	var facts common.SourceFacts
	tree, err := grammar.Parse(content)
	if err != nil {
		return facts, err
	}
	if err := tree.HeaderError(tree.Root, tree.OfType(declarations)); err != nil {
		return facts, err
	}
	imports := make(map[string]bool)
	for _, c := range tree.Children(tree.Root) {
		switch tree.Type(c) {
		case "package_header":
			if id := tree.ChildOfType(c, "identifier"); id != nil {
				facts.Package = jvmsyntax.Join(tree.Leaves(id, "simple_identifier"))
			}
		case "import_list":
			for _, h := range tree.Children(c) {
				if tree.Type(h) == "import_header" {
					imports[importIdentity(tree, h)] = true
				}
			}
		case "import_header":
			imports[importIdentity(tree, c)] = true
		}
	}
	for _, c := range tree.Children(tree.Root) {
		if !declarations[tree.Type(c)] {
			continue
		}
		for _, name := range declaredNames(tree, c) {
			facts.Declared = append(facts.Declared, jvmsyntax.Qualify(facts.Package, name))
		}
	}
	facts.Imports = jvmsyntax.Sorted(imports)
	facts.Main = definesMain(tree)
	return facts, nil
}

func importIdentity(tree *jvmsyntax.Tree, n *ts.Node) string {
	id := tree.ChildOfType(n, "identifier")
	if id == nil {
		return ""
	}
	return jvmsyntax.Join(tree.Leaves(id, "simple_identifier"))
}

func declaredNames(tree *jvmsyntax.Tree, n *ts.Node) []string {
	switch tree.Type(n) {
	case "function_declaration":
		if id := tree.ChildOfType(n, "simple_identifier"); id != nil {
			return []string{jvmsyntax.Unquote(tree.Text(id))}
		}
	case "property_declaration":
		var names []string
		for _, typ := range []string{"variable_declaration", "multi_variable_declaration"} {
			if v := tree.ChildOfType(n, typ); v != nil {
				names = append(names, tree.Leaves(v, "simple_identifier")...)
			}
		}
		return names
	default:
		if id := tree.ChildOfType(n, "type_identifier"); id != nil {
			return []string{jvmsyntax.Unquote(tree.Text(id))}
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
		if tree.Type(n) == "function_declaration" {
			if id := tree.ChildOfType(n, "simple_identifier"); id != nil && jvmsyntax.Unquote(tree.Text(id)) == "main" {
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
