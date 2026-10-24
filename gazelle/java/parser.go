package java

import (
	"os"

	"github.com/devhusdk/rules_dx/gazelle/common"
	"github.com/devhusdk/rules_dx/gazelle/jvmsyntax"

	ts "github.com/odvcencio/gotreesitter"
	javagrammar "github.com/odvcencio/gotreesitter/grammars/java"
)

var grammar = jvmsyntax.NewGrammar("java", javagrammar.Language)

var typeDeclarations = map[string]bool{
	"class_declaration":           true,
	"interface_declaration":       true,
	"enum_declaration":            true,
	"record_declaration":          true,
	"annotation_type_declaration": true,
	"module_declaration":          true,
}

func ParseSource(content []byte) (common.SourceFacts, error) {
	var facts common.SourceFacts
	tree, err := grammar.Parse(content)
	if err != nil {
		return facts, err
	}
	if err := tree.HeaderError(tree.Root, tree.OfType(typeDeclarations)); err != nil {
		return facts, err
	}
	imports := make(map[string]bool)
	var packages []string
	for _, c := range tree.Children(tree.Root) {
		switch tree.Type(c) {
		case "package_declaration":
			packages = append(packages, packageName(tree, c))
		case "import_declaration":
			imports[importIdentity(tree, c)] = true
		}
	}
	if len(packages) > 1 {
		return facts, errDuplicatePackage(packages[0])
	}
	if len(packages) == 1 {
		facts.Package = packages[0]
	}
	for _, c := range tree.Children(tree.Root) {
		if typ := tree.Type(c); typeDeclarations[typ] && typ != "module_declaration" {
			if name := tree.Field(c, "name"); name != nil {
				facts.Declared = append(facts.Declared, jvmsyntax.Qualify(facts.Package, tree.Text(name)))
			}
		}
	}
	facts.Imports = jvmsyntax.Sorted(imports)
	facts.Main = definesMain(tree)
	return facts, nil
}

func packageName(tree *jvmsyntax.Tree, n *ts.Node) string {
	for _, c := range tree.Children(n) {
		if typ := tree.Type(c); typ == "identifier" || typ == "scoped_identifier" {
			return jvmsyntax.Join(tree.Leaves(c, "identifier"))
		}
	}
	return ""
}

func importIdentity(tree *jvmsyntax.Tree, n *ts.Node) string {
	parts := tree.Leaves(n, "identifier")
	if tree.HasToken(n, "static") && tree.ChildOfType(n, "asterisk") == nil && len(parts) > 1 {
		parts = parts[:len(parts)-1]
	}
	return jvmsyntax.Join(parts)
}

func definesMain(tree *jvmsyntax.Tree) bool {
	found := false
	tree.Walk(tree.Root, func(n *ts.Node) bool {
		if found {
			return false
		}
		if tree.Type(n) != "method_declaration" {
			return true
		}
		name := tree.Field(n, "name")
		typ := tree.Field(n, "type")
		mods := tree.ChildOfType(n, "modifiers")
		if name != nil && tree.Text(name) == "main" && typ != nil && tree.Type(typ) == "void_type" && mods != nil && tree.HasToken(mods, "static") {
			found = true
		}
		return true
	})
	return found
}

type duplicatePackageError struct{ first string }

func (e *duplicatePackageError) Error() string {
	return "java: duplicate package clause " + e.first
}

func errDuplicatePackage(first string) error { return &duplicatePackageError{first: first} }

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
