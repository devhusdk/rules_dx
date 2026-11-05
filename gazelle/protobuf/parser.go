package protobuf

import (
	"regexp"
	"sort"
)

var importLine = regexp.MustCompile(`(?m)^\s*import\s+(?:public\s+|weak\s+)?(?:"([^"\n]+)"|'([^'\n]+)')\s*;`)

var comments = regexp.MustCompile(`(?s)/\*.*?\*/|//[^\n]*`)

// ParseImports returns the sorted, deduplicated paths named by the import
// statements of a .proto file. Imports inside comments are skipped.
func ParseImports(content []byte) []string {
	set := make(map[string]bool)
	for _, m := range importLine.FindAllSubmatch(stripComments(content), -1) {
		for _, group := range m[1:] {
			if len(group) > 0 {
				set[string(group)] = true
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

var serviceDecl = regexp.MustCompile(`(?m)^\s*service\s+[A-Za-z_][A-Za-z0-9_]*\s*\{`)

// HasService reports whether a .proto file declares a service block.
func HasService(content []byte) bool {
	return serviceDecl.Match(stripComments(content))
}

// stripComments blanks comments and keeps line breaks.
func stripComments(content []byte) []byte {
	return comments.ReplaceAllFunc(content, func(m []byte) []byte {
		out := make([]byte, len(m))
		for i, b := range m {
			if b == '\n' {
				out[i] = b
			} else {
				out[i] = ' '
			}
		}
		return out
	})
}
