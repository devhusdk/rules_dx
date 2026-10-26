package common

import (
	"path"
	"slices"
	"sort"
	"strings"
)

func Scan(src []byte, add func(string)) {
	n := len(src)
	i := 0
	for i < n {
		c := src[i]
		if np, ok := skipComment(src, i); ok {
			i = np
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
	Spec     string
	Root     string
	Ext      string
	Relative bool
}

// JSModuleExts are the extensions stripped from a relative JavaScript or
// TypeScript import to get its module identity.
var JSModuleExts = []string{".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx", ".mts", ".cts"}

// relativeRef returns the workspace-relative identity of a relative spec with
// any extension in exts stripped, and any other extension (such as .json or
// .prod) left in the root and returned as ext.
func relativeRef(dir, spec string, exts []string) (root, ext string) {
	spec = strings.TrimSpace(spec)
	trimmed := strings.TrimSuffix(spec, "/")
	switch path.Base(trimmed) {
	case "", ".", "..", "/":
		return "", ""
	}
	var joined string
	if strings.HasPrefix(spec, "/") {
		joined = strings.TrimPrefix(path.Clean(trimmed), "/")
	} else {
		joined = path.Join(dir, trimmed)
	}
	base := path.Base(joined)
	e := path.Ext(base)
	if e == "" || e == "." || e == base {
		return joined, ""
	}
	if slices.Contains(exts, e) {
		return strings.TrimSuffix(joined, e), ""
	}
	return joined, e
}

func ModuleIdentity(pkg, stem string) string {
	return path.Join(pkg, stem)
}

func isRelativeSpec(spec string) bool {
	spec = strings.TrimSpace(spec)
	return strings.HasPrefix(spec, ".") || strings.HasPrefix(spec, "/")
}

func ParseImportRefs(src []byte, dir string) []ImportRef {
	refs := make(map[string]ImportRef)
	Scan(src, func(spec string) {
		spec = strings.TrimSpace(spec)
		if spec == "" {
			return
		}
		relative := isRelativeSpec(spec)
		root, ext := spec, ""
		if relative {
			root, ext = relativeRef(dir, spec, JSModuleExts)
		}
		if root == "" {
			return
		}
		if prev, ok := refs[root]; ok && (prev.Relative || !relative) {
			return
		}
		refs[root] = ImportRef{Spec: spec, Root: root, Ext: ext, Relative: relative}
	})
	out := make([]ImportRef, 0, len(refs))
	for _, ref := range refs {
		out = append(out, ref)
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Root < out[j].Root })
	return out
}

var specSetExts = append(slices.Clone(JSModuleExts), ".vue", ".svelte", ".astro", ".mdx")

type SpecSet struct {
	Dir   string
	roots map[string]struct{}
}

func (s *SpecSet) Add(spec string) {
	spec = strings.TrimSpace(spec)
	root := spec
	if isRelativeSpec(spec) {
		root, _ = relativeRef(s.Dir, spec, specSetExts)
	}
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
	if np, ok := parseImportEquals(src, p, add); ok {
		return np
	}
	if hasWordAt(src, p, "type") {
		p = skipTrivia(src, p+4)
		if np, ok := parseImportEquals(src, p, add); ok {
			return np
		}
	}
	if p < n && (src[p] == '\'' || src[p] == '"') {
		if spec, next, ok := parseQuoted(src, p); ok {
			add(spec)
			return next
		}
		return p + 1
	}
	for p < n {
		if np, ok := skipComment(src, p); ok {
			p = np
			continue
		}
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
	if hasWordAt(src, p, "import") {
		return parseImport(src, p+6, add)
	}
	depth := 0
	for p < n {
		if np, ok := skipComment(src, p); ok {
			p = np
			continue
		}
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

// skipComment reports whether a comment starts at p and returns the index just
// past it. A line comment stops at the newline; an unclosed block comment runs
// to the end of src.
func skipComment(src []byte, p int) (int, bool) {
	n := len(src)
	if p+1 >= n || src[p] != '/' {
		return p, false
	}
	switch src[p+1] {
	case '/':
		j := p + 2
		for j < n && src[j] != '\n' {
			j++
		}
		return j, true
	case '*':
		j := p + 2
		for j+1 < n && !(src[j] == '*' && src[j+1] == '/') {
			j++
		}
		if j+1 < n {
			return j + 2, true
		}
		return n, true
	}
	return p, false
}

func parseImportEquals(src []byte, p int, add func(string)) (int, bool) {
	n := len(src)
	if p >= n || !isIdentStart(src[p]) {
		return p, false
	}
	j := p + 1
	for j < n && isIdentChar(src[j]) {
		j++
	}
	q := skipTrivia(src, j)
	if q >= n || src[q] != '=' || (q+1 < n && src[q+1] == '=') {
		return p, false
	}
	r := skipTrivia(src, q+1)
	if hasWordAt(src, r, "require") {
		return parseRequire(src, r+7, add), true
	}
	return r, true
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
		if np, ok := skipComment(src, p); ok {
			p = np
			continue
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
