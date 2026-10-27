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
	packageRe  = regexp.MustCompile(`(?m)^[ \t]*package[ \t]+([A-Za-z_]\w*(?:\.\w+)*)[ \t]*(?:;|\{|$)`)
	mainRe     = regexp.MustCompile(`def\s+main\s*\(|@main\s+(?:@\w+\s+)*def\b|\bextends\s+(?:_root_\.)?(?:scala\.)?App\b`)
)

func ParseImports(content []byte) []string {
	stripped := common.MaskScalaStyle(content)
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

// ParsePackage joins chained and nested package clauses (package a / package b,
// package a { package b {) with dots. A clause that follows other code is a
// separate package and is an error. Clauses sharing one line (package a; package b)
// are not recognized.
func ParsePackage(content []byte) (string, error) {
	stripped := common.MaskScalaStyle(content)
	matches := packageRe.FindAllSubmatchIndex(stripped, -1)
	if len(matches) == 0 {
		return "", nil
	}
	name := string(stripped[matches[0][2]:matches[0][3]])
	for i := 1; i < len(matches); i++ {
		gap := stripped[matches[i-1][1]:matches[i][0]]
		if len(strings.Trim(string(gap), " \t\r\n;")) != 0 {
			return "", errSeparatePackage(name, string(stripped[matches[i][2]:matches[i][3]]))
		}
		name += "." + string(stripped[matches[i][2]:matches[i][3]])
	}
	return name, nil
}

type separatePackageError struct{ first, second string }

func (e *separatePackageError) Error() string {
	return "scala: separate package clauses " + e.first + ", " + e.second
}

func errSeparatePackage(first, second string) error {
	return &separatePackageError{first: first, second: second}
}

func DefinesMain(content []byte) bool {
	return mainRe.Match(common.MaskScalaStyle(content))
}

func normalizeImport(dotted string, onDemand bool) string {
	dotted = strings.TrimSpace(dotted)
	if dotted == "" || !onDemand || IsStdLib(dotted) {
		return dotted
	}
	return dotted + ".*"
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
