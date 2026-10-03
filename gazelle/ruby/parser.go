package ruby

import (
	"regexp"
	"sort"
	"strings"
)

var (
	requireRe = regexp.MustCompile(`(?m)^\s*require\s+["']([^"']+)["']\s*$`)
	mainRe    = regexp.MustCompile(`__FILE__\s*==\s*\$0|__FILE__\s*==\s*\$PROGRAM_NAME`)
)

func stripNonCode(content []byte) []byte {
	s := string(content)
	out := make([]byte, len(s))
	copy(out, s)
	mask := func(from, to int) {
		for i := from; i < to; i++ {
			if out[i] != '\n' {
				out[i] = ' '
			}
		}
	}
	lines := strings.Split(s, "\n")
	offset := 0
	offsets := make([]int, len(lines))
	for i, line := range lines {
		offsets[i] = offset
		offset += len(line) + 1
	}
	inBlock := false
	for i, line := range lines {
		trimmed := strings.TrimSpace(line)
		if !inBlock && (trimmed == "=begin" || strings.HasPrefix(trimmed, "=begin ")) {
			inBlock = true
			mask(offsets[i], offsets[i]+len(line))
			continue
		}
		if inBlock {
			mask(offsets[i], offsets[i]+len(line))
			if trimmed == "=end" || strings.HasPrefix(trimmed, "=end ") {
				inBlock = false
			}
			continue
		}
	}
	for i := 0; i < len(out); {
		if out[i] == '#' {
			j := i + 1
			for j < len(out) && out[j] != '\n' {
				j++
			}
			mask(i, j)
			i = j
			continue
		}
		i++
	}
	return out
}

func ParseImports(content []byte) []string {
	stripped := stripNonCode(content)
	set := make(map[string]struct{})
	for _, m := range requireRe.FindAllSubmatch(stripped, -1) {
		req := strings.TrimSpace(string(m[1]))
		if req == "" {
			continue
		}
		if strings.HasPrefix(req, ".") {
			continue
		}
		if id := normalizeImport(req); id != "" {
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
	return "", nil
}

func normalizeImport(req string) string {
	return strings.TrimSpace(req)
}

func DefinesMain(content []byte) bool {
	return mainRe.Match(stripNonCode(content))
}
