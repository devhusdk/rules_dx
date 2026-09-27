package scala

import (
	"github.com/ralvik/rules_dx/gazelle/common"
	"regexp"
	"sort"
)

var (
	importRe  = regexp.MustCompile(`(?m)^\s*import\s+([A-Za-z_][\w]*(?:\.[\w]+)*)(\.\*)?(?:\s+as\s+[A-Za-z_][\w]*)?\s*;?\s*$`)
	packageRe = regexp.MustCompile(`(?m)^\s*package\s+([A-Za-z_][\w]*(?:\.[\w]+)*)\s*;?\s*$`)
	mainRe    = regexp.MustCompile(`def\s+main\s*\(`)
)

func ParseImports(content []byte) []string {
	stripped := common.MaskJavaStyle(content)
	set := make(map[string]struct{})
	for _, m := range importRe.FindAllSubmatch(stripped, -1) {
		dotted := string(m[1])
		onDemand := len(m[2]) > 0
		if id := normalizeImport(dotted, onDemand); id != "" {
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
	stripped := common.MaskJavaStyle(content)
	matches := packageRe.FindAllSubmatch(stripped, -1)
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
	return "scala: duplicate package clause " + e.first
}

func errDuplicatePackage(first string) error { return &duplicatePackageError{first: first} }

func DefinesMain(content []byte) bool {
	return mainRe.Match(common.MaskJavaStyle(content))
}

func normalizeImport(dotted string, onDemand bool) string {
	return common.NormalizeDotted(dotted, onDemand, IsStdLib)
}
