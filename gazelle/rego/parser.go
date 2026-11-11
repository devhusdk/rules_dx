package rego

import (
	"regexp"
	"sort"
	"strings"
)

var (
	packageRe = regexp.MustCompile(`(?m)^[ \t]*package[ \t]+([A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)`)
	importRe  = regexp.MustCompile(`(?m)^[ \t]*import[ \t]+([A-Za-z_][A-Za-z0-9_]*(?:\.[A-Za-z_][A-Za-z0-9_]*)*)(?:[ \t]+as[ \t]+[A-Za-z_][A-Za-z0-9_]*)?[ \t]*(?:#.*)?$`)
)

// stripComments blanks every # comment so commented-out lines never match.
func stripComments(content []byte) []byte {
	lines := strings.Split(string(content), "\n")
	for i, line := range lines {
		if j := commentStart(line); j >= 0 {
			lines[i] = line[:j]
		}
	}
	return []byte(strings.Join(lines, "\n"))
}

func commentStart(line string) int {
	inString := false
	for i := 0; i < len(line); i++ {
		switch c := line[i]; {
		case c == '\\' && inString:
			i++
		case c == '"':
			inString = !inString
		case c == '#' && !inString:
			return i
		}
	}
	return -1
}

// ParsePackage returns the dotted package a Rego file declares, or "".
func ParsePackage(content []byte) string {
	if m := packageRe.FindSubmatch(stripComments(content)); m != nil {
		return string(m[1])
	}
	return ""
}

// ParseImports returns the data documents a Rego file imports, without the
// data. prefix, sorted and unique.
func ParseImports(content []byte) []string {
	seen := map[string]bool{}
	var out []string
	for _, m := range importRe.FindAllSubmatch(stripComments(content), -1) {
		ref, ok := strings.CutPrefix(string(m[1]), "data.")
		if !ok || ref == "" || seen[ref] {
			continue
		}
		seen[ref] = true
		out = append(out, ref)
	}
	sort.Strings(out)
	return out
}
