package scala

import (
	"github.com/devhusdk/rules_dx/gazelle/common"
	"regexp"
	"sort"
	"strings"
)

var (
	importRe   = regexp.MustCompile(`(?m)(?:^|;)[ \t]*import[ \t]+`)
	wildcardRe = regexp.MustCompile(`^([A-Za-z_]\w*(?:\.\w+)*)\.(?:_|\*|given(?:\s.*)?)$`)
	plainRe    = regexp.MustCompile(`^([A-Za-z_]\w*(?:\.\w+)*)(?:\s+as\s+[A-Za-z_]\w*)?$`)
	dottedRe   = regexp.MustCompile(`^[A-Za-z_]\w*(?:\.\w+)*$`)
	selectorRe = regexp.MustCompile(`^([A-Za-z_]\w*)(?:\s*(?:=>|\s+as\s+)\s*([A-Za-z_]\w*))?$`)
	packageRe  = regexp.MustCompile(`(?m)^\s*package\s+([A-Za-z_][\w]*(?:\.[\w]+)*)\s*;?\s*$`)
	mainRe     = regexp.MustCompile(`def\s+main\s*\(`)
)

func ParseImports(content []byte) []string {
	stripped := common.MaskJavaStyle(content)
	set := make(map[string]struct{})
	for _, loc := range importRe.FindAllIndex(stripped, -1) {
		for _, importer := range splitTopLevel(importClause(stripped, loc[1])) {
			for _, id := range importerIDs(importer) {
				if id != "" {
					set[id] = struct{}{}
				}
			}
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

func importClause(src []byte, start int) string {
	depth := 0
	end := start
	for ; end < len(src); end++ {
		c := src[end]
		if depth == 0 && (c == '\n' || c == ';' || c == '}') {
			break
		}
		switch c {
		case '{', '[', '(':
			depth++
		case '}', ']', ')':
			depth--
		}
	}
	return string(src[start:end])
}

func splitTopLevel(body string) []string {
	var parts []string
	depth := 0
	last := 0
	for i := 0; i < len(body); i++ {
		switch body[i] {
		case '{', '[', '(':
			depth++
		case '}', ']', ')':
			depth--
		case ',':
			if depth == 0 {
				parts = append(parts, strings.TrimSpace(body[last:i]))
				last = i + 1
			}
		}
	}
	return append(parts, strings.TrimSpace(body[last:]))
}

func importerIDs(importer string) []string {
	importer = strings.TrimSpace(importer)
	if open := strings.IndexByte(importer, '{'); open >= 0 {
		prefix := strings.TrimSpace(importer[:open])
		if !strings.HasSuffix(prefix, ".") || !strings.HasSuffix(importer, "}") {
			return nil
		}
		pkg := strings.TrimSpace(strings.TrimSuffix(prefix, "."))
		if !dottedRe.MatchString(pkg) {
			return nil
		}
		var ids []string
		for _, sel := range splitTopLevel(importer[open+1 : len(importer)-1]) {
			if sel == "_" || sel == "*" || sel == "given" || strings.HasPrefix(sel, "given ") {
				ids = append(ids, normalizeImport(pkg, true))
				continue
			}
			m := selectorRe.FindStringSubmatch(sel)
			if m == nil || m[2] == "_" {
				continue
			}
			ids = append(ids, normalizeImport(pkg+"."+m[1], false))
		}
		return ids
	}
	if m := wildcardRe.FindStringSubmatch(importer); m != nil {
		return []string{normalizeImport(m[1], true)}
	}
	if m := plainRe.FindStringSubmatch(importer); m != nil {
		return []string{normalizeImport(m[1], false)}
	}
	return nil
}
