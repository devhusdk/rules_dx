package gleam

import (
	"regexp"
	"sort"
	"strings"
)

var importPattern = regexp.MustCompile(`^import\s+([a-z][a-z0-9_]*(?:/[a-z][a-z0-9_]*)*)`)

func ParseImports(content []byte) []string {
	set := make(map[string]struct{})
	for _, line := range strings.Split(string(content), "\n") {
		if match := importPattern.FindStringSubmatch(strings.TrimSpace(line)); match != nil {
			set[match[1]] = struct{}{}
		}
	}
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}
