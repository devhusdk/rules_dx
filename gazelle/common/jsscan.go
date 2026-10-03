package common

import (
	"path"
	"sort"
	"strings"
)

func NormalizeJSSpec(spec string) string {
	spec = strings.TrimSpace(spec)
	if spec == "" {
		return ""
	}
	if strings.HasPrefix(spec, ".") || strings.HasPrefix(spec, "/") {
		trimmed := strings.TrimSuffix(spec, "/")
		base := path.Base(trimmed)
		if base == "" || base == "." || base == "/" {
			return ""
		}
		if idx := strings.LastIndexByte(base, '.'); idx > 0 {
			base = base[:idx]
		}
		return base
	}
	return spec
}

func Scan(src []byte, add func(string)) {
	n := len(src)
	i := 0
	for i < n {
		c := src[i]
		if c == '/' && i+1 < n && src[i+1] == '/' {
			j := i + 2
			for j < n && src[j] != '\n' {
				j++
			}
			i = j
			continue
		}
		if c == '/' && i+1 < n && src[i+1] == '*' {
			j := i + 2
			for j+1 < n && !(src[j] == '*' && src[j+1] == '/') {
				j++
			}
			if j+1 < n {
				i = j + 2
			} else {
				return
			}
			continue
		}
		if c == '\'' || c == '"' {
			i = skipQuoted(src, i)
			continue
		}
		if c == '`' {
			i = skipTemplate(src, i)
			continue
		}
		if c == '/' && isRegexStart(src, i) {
			i = skipRegex(src, i)
			continue
		}
		if isIdentStart(c) {
			j := i + 1
			for j < n && isIdentChar(src[j]) {
				j++
			}
			word := string(src[i:j])
			switch word {
			case "import":
				i = parseImport(src, j, add)
				continue
			case "export":
				i = parseExport(src, j, add)
				continue
			case "require":
				if isPrecededByDot(src, i) {
					i = j
					continue
				}
				i = parseRequire(src, j, add)
				continue
			}
			i = j
			continue
		}
		i++
	}
}

type ImportRef struct {
	Root     string
	Relative bool
}

func isRelativeSpec(spec string) bool {
	spec = strings.TrimSpace(spec)
	return strings.HasPrefix(spec, ".") || strings.HasPrefix(spec, "/")
}

func ParseImportRefs(src []byte) []ImportRef {
	rel := make(map[string]bool)
	Scan(src, func(spec string) {
		root := NormalizeJSSpec(spec)
		if root == "" {
			return
		}
		if isRelativeSpec(spec) {
			rel[root] = true
		} else if _, ok := rel[root]; !ok {
			rel[root] = false
		}
	})
	out := make([]ImportRef, 0, len(rel))
	for root, relative := range rel {
		out = append(out, ImportRef{Root: root, Relative: relative})
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Root < out[j].Root })
	return out
}

type SpecSet struct {
	roots map[string]struct{}
}

func (s *SpecSet) Add(spec string) {
	root := NormalizeJSSpec(spec)
	if root == "" {
		return
	}
	if s.roots == nil {
		s.roots = make(map[string]struct{})
	}
	s.roots[root] = struct{}{}
}

func (s *SpecSet) Roots() []string {
	var out []string
	for root := range s.roots {
		out = append(out, root)
	}
	sort.Strings(out)
	return out
}

func parseImport(src []byte, pos int, add func(string)) int {
	n := len(src)
	p := skipTrivia(src, pos)
	if p < n && src[p] == '.' {
		return p + 1
	}
	if p < n && src[p] == '(' {
		p++
		p = skipTrivia(src, p)
		if p < n && (src[p] == '\'' || src[p] == '"') {
			if spec, next, ok := parseQuoted(src, p); ok {
				q := skipTrivia(src, next)
				if q < n && src[q] == ')' {
					add(spec)
					return q + 1
				}
				return next
			}
			return p + 1
		}
		return p
	}
	if hasWordAt(src, p, "type") {
		p = skipTrivia(src, p+4)
	}
	if p < n && (src[p] == '\'' || src[p] == '"') {
		if spec, next, ok := parseQuoted(src, p); ok {
			add(spec)
			return next
		}
		return p + 1
	}
	for p < n {
		if src[p] == '\'' || src[p] == '"' {
			p = skipQuoted(src, p)
			continue
		}
		if src[p] == '`' {
			p = skipTemplate(src, p)
			continue
		}
		if src[p] == ';' {
			return p + 1
		}
		if src[p] == '\n' {
			p++
			continue
		}
		if isIdentStart(src[p]) {
			j := p + 1
			for j < n && isIdentChar(src[j]) {
				j++
			}
			if string(src[p:j]) == "from" {
				q := skipTrivia(src, j)
				if q < n && (src[q] == '\'' || src[q] == '"') {
					if spec, next, ok := parseQuoted(src, q); ok {
						add(spec)
						return next
					}
					return q + 1
				}
				return j
			}
			p = j
			continue
		}
		p++
	}
	return p
}

func parseExport(src []byte, pos int, add func(string)) int {
	n := len(src)
	p := skipTrivia(src, pos)
	if hasWordAt(src, p, "type") {
		p = skipTrivia(src, p+4)
	}
	depth := 0
	for p < n {
		if src[p] == '\'' || src[p] == '"' {
			p = skipQuoted(src, p)
			continue
		}
		if src[p] == '`' {
			p = skipTemplate(src, p)
			continue
		}
		if src[p] == '{' {
			depth++
			p++
			continue
		}
		if src[p] == '}' {
			if depth > 0 {
				depth--
			}
			p++
			continue
		}
		if src[p] == ';' || src[p] == '\n' {
			if src[p] == ';' {
				return p + 1
			}
			if depth > 0 {
				p++
				continue
			}
			q := skipTrivia(src, p)
			if hasWordAt(src, q, "from") {
				p = q
				continue
			}
			return q
		}
		if isIdentStart(src[p]) {
			j := p + 1
			for j < n && isIdentChar(src[j]) {
				j++
			}
			if string(src[p:j]) == "from" {
				q := skipTrivia(src, j)
				if q < n && (src[q] == '\'' || src[q] == '"') {
					if spec, next, ok := parseQuoted(src, q); ok {
						add(spec)
						return next
					}
					return q + 1
				}
				return j
			}
			p = j
			continue
		}
		p++
	}
	return p
}

func parseRequire(src []byte, pos int, add func(string)) int {
	n := len(src)
	p := skipTrivia(src, pos)
	if p >= n || src[p] != '(' {
		return pos
	}
	p++
	p = skipTrivia(src, p)
	if p < n && (src[p] == '\'' || src[p] == '"') {
		if spec, next, ok := parseQuoted(src, p); ok {
			q := skipTrivia(src, next)
			if q < n && src[q] == ')' {
				add(spec)
				return q + 1
			}
			return next
		}
		return p + 1
	}
	return p
}

func skipTrivia(src []byte, pos int) int {
	n := len(src)
	p := pos
	for p < n {
		c := src[p]
		if c == ' ' || c == '\t' || c == '\r' || c == '\n' {
			p++
			continue
		}
		if c == '/' && p+1 < n && src[p+1] == '/' {
			j := p + 2
			for j < n && src[j] != '\n' {
				j++
			}
			p = j
			continue
		}
		if c == '/' && p+1 < n && src[p+1] == '*' {
			j := p + 2
			for j+1 < n && !(src[j] == '*' && src[j+1] == '/') {
				j++
			}
			if j+1 < n {
				p = j + 2
				continue
			}
			return n
		}
		return p
	}
	return p
}

func parseQuoted(src []byte, pos int) (string, int, bool) {
	n := len(src)
	quote := src[pos]
	k := pos + 1
	var b strings.Builder
	for k < n {
		c := src[k]
		if c == '\\' {
			if k+1 < n {
				b.WriteByte(src[k+1])
				k += 2
				continue
			}
			return "", pos + 1, false
		}
		if c == quote {
			return b.String(), k + 1, true
		}
		if c == '\n' {
			return "", pos + 1, false
		}
		b.WriteByte(c)
		k++
	}
	return "", pos + 1, false
}

func skipQuoted(src []byte, pos int) int {
	if _, next, ok := parseQuoted(src, pos); ok {
		return next
	}
	n := len(src)
	k := pos + 1
	for k < n && src[k] != '\n' {
		k++
	}
	return k
}

func skipTemplate(src []byte, pos int) int {
	n := len(src)
	j := pos + 1
	for j < n {
		if src[j] == '\\' {
			j += 2
			continue
		}
		if src[j] == '`' {
			return j + 1
		}
		if src[j] == '$' && j+1 < n && src[j+1] == '{' {
			j = skipSubstitution(src, j+2)
			continue
		}
		j++
	}
	return n
}

func skipSubstitution(src []byte, pos int) int {
	n := len(src)
	depth := 1
	j := pos
	for j < n && depth > 0 {
		c := src[j]
		if c == '{' {
			depth++
		} else if c == '}' {
			depth--
		} else if c == '\'' || c == '"' {
			j = skipQuoted(src, j)
			continue
		} else if c == '`' {
			j = skipTemplate(src, j)
			continue
		}
		j++
	}
	return j
}

func isRegexStart(src []byte, pos int) bool {
	j := pos - 1
	for j >= 0 && (src[j] == ' ' || src[j] == '\t' || src[j] == '\r' || src[j] == '\n') {
		j--
	}
	if j < 0 {
		return true
	}
	c := src[j]
	if c == ')' || c == ']' || c == '}' {
		return false
	}
	if isIdentChar(c) || c == '"' || c == '\'' || c == '`' {
		return false
	}
	return true
}

func skipRegex(src []byte, pos int) int {
	n := len(src)
	k := pos + 1
	inClass := false
	for k < n {
		c := src[k]
		if c == '\\' {
			k += 2
			continue
		}
		if c == '\n' {
			return k
		}
		if c == '[' {
			inClass = true
			k++
			continue
		}
		if c == ']' {
			inClass = false
			k++
			continue
		}
		if c == '/' && !inClass {
			k++
			for k < n && isIdentChar(src[k]) {
				k++
			}
			return k
		}
		k++
	}
	return n
}

func hasWordAt(src []byte, pos int, word string) bool {
	if pos+len(word) > len(src) {
		return false
	}
	if string(src[pos:pos+len(word)]) != word {
		return false
	}
	after := pos + len(word)
	return after >= len(src) || !isIdentChar(src[after])
}

func isPrecededByDot(src []byte, pos int) bool {
	j := pos - 1
	for j >= 0 && (src[j] == ' ' || src[j] == '\t' || src[j] == '\r' || src[j] == '\n') {
		j--
	}
	return j >= 0 && src[j] == '.'
}

func isIdentStart(c byte) bool {
	return c == '_' || c == '$' || (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z')
}

func isIdentChar(c byte) bool {
	return isIdentStart(c) || (c >= '0' && c <= '9')
}
