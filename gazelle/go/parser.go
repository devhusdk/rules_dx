package golang

import (
	"go/parser"
	"go/token"
	"sort"
	"strconv"
	"strings"
)

func ParseImports(content []byte) ([]string, error) {
	fset := token.NewFileSet()
	f, err := parser.ParseFile(fset, "source.go", content, parser.ImportsOnly)
	if err != nil {
		return nil, err
	}
	set := make(map[string]struct{})
	for _, imp := range f.Imports {
		raw, err := strconv.Unquote(imp.Path.Value)
		if err != nil || strings.TrimSpace(raw) == "" {
			continue
		}
		set[raw] = struct{}{}
	}
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out, nil
}

func ParsePackage(content []byte) (string, error) {
	fset := token.NewFileSet()
	f, err := parser.ParseFile(fset, "source.go", content, parser.PackageClauseOnly)
	if err != nil {
		return "", err
	}
	return f.Name.Name, nil
}

func IsCgoImport(path string) bool {
	return strings.TrimSpace(path) == "C"
}
