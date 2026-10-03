package fsharp

import (
	"github.com/ralvik/rules_dx/gazelle/common"
	"regexp"
	"sort"
)

var (
	openRe      = regexp.MustCompile(`(?m)^\s*open\s+([A-Za-z_][\w]*(?:\.[\w]+)*)\s*$`)
	namespaceRe = regexp.MustCompile(`(?m)^\s*(?:namespace|module)\s+([A-Za-z_][\w]*(?:\.[\w]+)*)\s*(?:=\s*)?$`)
	mainRe      = regexp.MustCompile(`\[<\s*EntryPoint\s*>\]`)
)

func ParseImports(content []byte) []string {
	stripped := common.MaskFSharpStyle(content)
	set := make(map[string]struct{})
	for _, m := range openRe.FindAllSubmatch(stripped, -1) {
		dotted := string(m[1])
		if id := normalizeImport(dotted); id != "" {
			set[id] = struct{}{}
		}
	}
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}

func ParsePackage(content []byte) (string, error) {
	stripped := common.MaskFSharpStyle(content)
	matches := namespaceRe.FindAllSubmatch(stripped, -1)
	if len(matches) > 1 {
		return "", errDuplicatePackage(string(matches[0][1]))
	}
	if len(matches) == 0 {
		return "", nil
	}
	return string(matches[0][1]), nil
}

type duplicatePackageError struct{ first string }

func (e *duplicatePackageError) Error() string {
	return "fsharp: duplicate namespace/module declaration " + e.first
}

func errDuplicatePackage(first string) error { return &duplicatePackageError{first: first} }

func DefinesMain(content []byte) bool {
	return mainRe.Match(common.MaskFSharpStyle(content))
}

func normalizeImport(dotted string) string {
	return common.NormalizeDotted(dotted, false, IsStdLib)
}
