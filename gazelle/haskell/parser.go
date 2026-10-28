package haskell

import (
	"regexp"
	"sort"
	"strings"
)

var (
	importRe = regexp.MustCompile(`^import\s+(?:safe\s+)?(?:qualified\s+)?(?:"[^"]*"\s+)?(?:qualified\s+)?([A-Z][A-Za-z0-9_']*(?:\.[A-Z][A-Za-z0-9_']*)*)`)
	mainRe   = regexp.MustCompile(`^module\s+Main\b`)
	mainSig  = regexp.MustCompile(`^main\s*(?:::|=)`)
)

// codeLines returns the source lines with comments blanked out.
func codeLines(content []byte) []string {
	s := string(content)
	var out strings.Builder
	depth := 0
	for i := 0; i < len(s); i++ {
		switch {
		case strings.HasPrefix(s[i:], "{-"):
			depth++
			i++
		case depth > 0 && strings.HasPrefix(s[i:], "-}"):
			depth--
			i++
		case depth > 0:
			if s[i] == '\n' {
				out.WriteByte('\n')
			}
		case strings.HasPrefix(s[i:], "--") && (i+2 >= len(s) || !strings.ContainsRune("!#$%&*+./<=>?@\\^|~:", rune(s[i+2]))):
			for i < len(s) && s[i] != '\n' {
				i++
			}
			out.WriteByte('\n')
		default:
			out.WriteByte(s[i])
		}
	}
	return strings.Split(strings.ReplaceAll(out.String(), "\r", ""), "\n")
}

// ParseImports returns the sorted unique module names imported by content.
func ParseImports(content []byte) []string {
	set := make(map[string]struct{})
	for _, line := range codeLines(content) {
		if m := importRe.FindStringSubmatch(line); m != nil {
			set[m[1]] = struct{}{}
		}
	}
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}

// DefinesMain reports whether content is a Main module or defines main.
func DefinesMain(content []byte) bool {
	for _, line := range codeLines(content) {
		if mainRe.MatchString(line) || mainSig.MatchString(line) {
			return true
		}
	}
	return false
}
