package zig

import (
	"fmt"
	"path/filepath"
	"strings"
	"sync"

	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/devhusdk/rules_dx/gazelle/common"
	"github.com/devhusdk/rules_dx/gazelle/common/treesitter"
	ts "github.com/odvcencio/gotreesitter"
	"github.com/odvcencio/gotreesitter/grammars/zig"
)

var SupportedExts = []string{".zig"}

var TestSourceSuffixes = []string{"_test"}

const LibraryKind = "zig_library"

// TestKind is the rule generated for each *_test.zig source.
const TestKind = "zig_test"

const ManifestFile = "build.zig.zon"

var grammar = sync.OnceValue(func() *ts.Language { return zig.Language() })

func rootHint(args language.GenerateArgs) string {
	content, err := common.ReadSource(filepath.Join(args.Dir, ManifestFile))
	if err != nil {
		return ""
	}
	return manifestName(content)
}

func manifestName(src []byte) string {
	src = common.StripBOM(src)
	wrapped := append(append([]byte("const manifest = "), src...), ';')
	tree := treesitter.Parse(grammar(), wrapped)
	defer tree.Release()
	if tree.Root == nil {
		return ""
	}
	var name string
	tree.Walk(tree.Root, func(n *treesitter.Node) bool {
		if name != "" {
			return false
		}
		if tree.Type(n) == "initializer_list" {
			name = nameField(tree, n)
			return false
		}
		return true
	})
	return name
}

// nameField returns the value of the .name field of the top-level initializer
// list n. Fields nested in other values are separate nodes and never visited.
func nameField(tree *treesitter.Tree, list *treesitter.Node) string {
	for _, f := range list.Children() {
		if tree.Type(f) != "assignment_expression" {
			continue
		}
		kids := f.Children()
		if len(kids) != 3 || tree.Text(kids[0]) != ".name" {
			continue
		}
		return fieldValue(tree, kids[2])
	}
	return ""
}

func fieldValue(tree *treesitter.Tree, v *treesitter.Node) string {
	switch tree.Type(v) {
	case "string":
		return stringContent(tree, v)
	case "field_expression":
		kids := v.Children()
		if len(kids) != 2 || tree.Type(kids[1]) != "identifier" {
			return ""
		}
		id := kids[1]
		for _, c := range id.Children() {
			if tree.Type(c) == "string" {
				return stringContent(tree, c)
			}
		}
		return tree.Text(id)
	}
	return ""
}

func stringContent(tree *treesitter.Tree, s *treesitter.Node) string {
	text := strings.TrimSuffix(strings.TrimPrefix(tree.Text(s), `"`), `"`)
	return strings.NewReplacer(`\"`, `"`, `\\`, `\`).Replace(text)
}

func IsTestSource(name string) bool {
	return common.IsTestSource(name, SupportedExts, TestSourceSuffixes)
}

func IsBuildScript(name string) bool { return name == "build.zig" }

func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

func RootCandidates(name string) []string {
	return []string{name + ".zig", "root.zig", "lib.zig"}
}

func OrderRootFirst(name string, sources []string) ([]string, error) {
	present := make(map[string]bool, len(sources))
	for _, src := range sources {
		present[src] = true
	}
	root := ""
	for _, cand := range RootCandidates(name) {
		if present[cand] {
			root = cand
			break
		}
	}
	if root == "" && len(sources) == 1 {
		root = sources[0]
	}
	if root == "" {
		return nil, fmt.Errorf("no module root among %s; name one of %s", strings.Join(sources, ", "), strings.Join(RootCandidates(name), ", "))
	}
	out := []string{root}
	for _, src := range sources {
		if src != root {
			out = append(out, src)
		}
	}
	return out, nil
}
