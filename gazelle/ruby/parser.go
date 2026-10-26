package ruby

import (
	"regexp"
	"sort"
	"strings"
)

var (
	requireRe = regexp.MustCompile(`(?m)^[ \t]*require[ \t]*\(?[ \t]*["']([^"']+)["'][ \t]*\)?[ \t]*(?:(?:if|unless)\b.*)?$`)
	mainRe    = regexp.MustCompile(`__FILE__\s*==\s*\$0|__FILE__\s*==\s*\$PROGRAM_NAME`)
)

type heredoc struct {
	id      string
	trimmed bool
}

// stripNonCode returns a same-length copy of content in which comments,
// =begin blocks, heredoc bodies and multi-line quoted literals are blanked
// (newlines are kept). Single-line literals are left as they are so that
// "require 'x'" still matches. When unsure it under-masks.
func stripNonCode(content []byte) []byte {
	out := make([]byte, len(content))
	copy(out, content)
	n := len(out)
	mask := func(from, to int) {
		for i := from; i < to && i < n; i++ {
			if out[i] != '\n' {
				out[i] = ' '
			}
		}
	}
	var pending []heredoc
	for i := 0; i < n; {
		lineStart := i == 0 || out[i-1] == '\n'
		if lineStart && hasLinePrefix(out, i, "=begin") {
			j := i
			for j < n {
				e := lineEnd(out, j)
				done := hasLinePrefix(out, j, "=end") && j != i
				j = e + 1
				if done {
					break
				}
			}
			if j > n {
				j = n
			}
			mask(i, j)
			i = j
			continue
		}
		c := out[i]
		switch {
		case c == '\n':
			i++
			for _, h := range pending {
				i = maskHeredoc(out, i, h, mask)
			}
			pending = pending[:0]
		case c == '#':
			j := lineEnd(out, i)
			mask(i, j)
			i = j
		case c == '"' || c == '\'' || c == '`':
			i = scanQuoted(out, i, mask)
		case c == '<' && i+2 < n && out[i+1] == '<':
			if h, next, ok := parseHeredocOpener(out, i); ok {
				pending = append(pending, h)
				i = next
			} else {
				i += 2
			}
		default:
			i++
		}
	}
	return out
}

func lineEnd(b []byte, i int) int {
	for i < len(b) && b[i] != '\n' {
		i++
	}
	return i
}

func hasLinePrefix(b []byte, i int, word string) bool {
	if !strings.HasPrefix(string(b[i:min(len(b), i+len(word))]), word) {
		return false
	}
	end := i + len(word)
	return end == len(b) || b[end] == '\n' || b[end] == ' ' || b[end] == '\t' || b[end] == '\r'
}

// scanQuoted handles a literal opened at i and returns the index after it.
// Single-line literals are untouched; multi-line ones are masked. An
// unterminated literal stops at the end of its line.
func scanQuoted(out []byte, i int, mask func(int, int)) int {
	n := len(out)
	q := out[i]
	interp := q != '\''
	j := i + 1
	for j < n {
		c := out[j]
		switch {
		case c == '\\':
			j += 2
		case interp && c == '#' && j+1 < n && out[j+1] == '{':
			j = skipInterpolation(out, j+2)
		case c == q:
			end := j + 1
			if strings.IndexByte(string(out[i:end]), '\n') >= 0 {
				mask(i, end)
			}
			return end
		default:
			j++
		}
	}
	// Unterminated: leave the literal and resume on the next line.
	return lineEnd(out, i)
}

// skipInterpolation returns the index after the matching close brace of an
// interpolation whose body starts at j, skipping nested quoted strings.
func skipInterpolation(out []byte, j int) int {
	n := len(out)
	depth := 1
	for j < n && depth > 0 {
		switch out[j] {
		case '{':
			depth++
			j++
		case '}':
			depth--
			j++
		case '"', '\'':
			q := out[j]
			j++
			for j < n && out[j] != q {
				if out[j] == '\\' {
					j++
				}
				j++
			}
			j++
		default:
			j++
		}
	}
	return min(j, n)
}

func parseHeredocOpener(out []byte, i int) (heredoc, int, bool) {
	n := len(out)
	j := i + 2
	h := heredoc{}
	if j < n && (out[j] == '~' || out[j] == '-') {
		h.trimmed = true
		j++
	}
	if j < n && (out[j] == '"' || out[j] == '\'' || out[j] == '`') {
		q := out[j]
		k := j + 1
		for k < n && out[k] != q && out[k] != '\n' {
			k++
		}
		if k >= n || out[k] != q || k == j+1 {
			return h, 0, false
		}
		h.id = string(out[j+1 : k])
		return h, k + 1, true
	}
	k := j
	for k < n && (out[k] == '_' || out[k] >= '0' && out[k] <= '9' || out[k] >= 'A' && out[k] <= 'Z' || (h.trimmed && out[k] >= 'a' && out[k] <= 'z')) {
		k++
	}
	if k == j {
		return h, 0, false
	}
	if !h.trimmed && (out[j] >= '0' && out[j] <= '9') {
		return h, 0, false
	}
	h.id = string(out[j:k])
	return h, k, true
}

// maskHeredoc blanks the heredoc body starting at i (including its
// terminator line) and returns the index after it.
func maskHeredoc(out []byte, i int, h heredoc, mask func(int, int)) int {
	n := len(out)
	j := i
	for j < n {
		e := lineEnd(out, j)
		line := strings.TrimRight(string(out[j:e]), "\r")
		if h.trimmed {
			line = strings.TrimSpace(line)
		}
		mask(j, e)
		j = e + 1
		if line == h.id {
			break
		}
	}
	return min(j, n)
}

func ParseImports(content []byte) []string {
	stripped := stripNonCode(content)
	set := make(map[string]struct{})
	for _, m := range requireRe.FindAllSubmatch(stripped, -1) {
		req := strings.TrimSpace(string(m[1]))
		if req == "" || strings.Contains(req, "#{") {
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
	return strings.TrimSuffix(strings.TrimSpace(req), ".rb")
}

func DefinesMain(content []byte) bool {
	return mainRe.Match(stripNonCode(content))
}
