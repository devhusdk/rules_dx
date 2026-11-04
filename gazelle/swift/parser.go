package swift

import (
	"regexp"
	"sort"
	"strings"
)

var (
	blockComment = regexp.MustCompile(`(?s)/\*.*?\*/`)
	lineComment  = regexp.MustCompile(`//[^\n]*`)
	importLine   = regexp.MustCompile(`(?m)^[ \t]*(?:@\w+(?:\([^)]*\))?[ \t]+|(?:public|internal|package|private|fileprivate)[ \t]+)*import[ \t]+(?:(?:typealias|struct|class|enum|protocol|let|var|func)[ \t]+)?([A-Za-z_][A-Za-z0-9_]*)`)
	mainAttr     = regexp.MustCompile(`(?m)^[ \t]*@main\b`)
)

func stripComments(content []byte) string {
	return lineComment.ReplaceAllString(blockComment.ReplaceAllString(string(content), ""), "")
}

// ParseImports returns the sorted, deduplicated top-level module names content imports.
func ParseImports(content []byte) []string {
	set := make(map[string]struct{})
	for _, m := range importLine.FindAllStringSubmatch(stripComments(content), -1) {
		set[m[1]] = struct{}{}
	}
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}

// DefinesMain reports whether content has an @main entry point.
func DefinesMain(content []byte) bool { return mainAttr.MatchString(stripComments(content)) }

// IsEntryFile reports whether name is main.swift, which holds top-level code.
func IsEntryFile(name string) bool { return strings.EqualFold(name, "main.swift") }
