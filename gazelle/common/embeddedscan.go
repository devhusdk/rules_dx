package common

import (
	"strings"
)

func ScanEmbedded(src []byte, add func(string)) {
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
			i = SkipEmbeddedQuoted(src, i)
			continue
		}
		if c == '`' {
			i = SkipEmbeddedTemplate(src, i)
			continue
		}
		if c == '/' && IsEmbeddedRegexStart(src, i) {
			i = skipEmbeddedRegex(src, i)
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
				i = parseEmbeddedImport(src, i, j, add)
				continue
			case "export":
				i = parseEmbeddedExport(src, j, add)
				continue
			case "require":
				if IsEmbeddedPrecededByDot(src, i) {
					i = j
					continue
				}
				i = parseEmbeddedRequire(src, j, add)
				continue
			}
			i = j
			continue
		}
		i++
	}
}

func parseEmbeddedImport(src []byte, start, j int, add func(string)) int {
	n := len(src)
	k := skipTrivia(src, j)
	if k < n && (src[k] == '\'' || src[k] == '"') {
		if spec, next, ok := readEmbeddedQuoted(src, k); ok {
			add(spec)
			return next
		}
		return k + 1
	}
	if k < n && src[k] == '(' {
		m := skipTrivia(src, k+1)
		if m < n && (src[m] == '\'' || src[m] == '"') {
			if spec, next, ok := readEmbeddedQuoted(src, m); ok {
				after := skipTrivia(src, next)
				if after < n && src[after] == ')' {
					add(spec)
					return after + 1
				}
				return next
			}
		}
		return k + 1
	}
	depth := 0
	p := k
	for p < n {
		if p+1 < n && src[p] == '/' && (src[p+1] == '/' || src[p+1] == '*') {
			p = skipTrivia(src, p)
			continue
		}
		c := src[p]
		if c == '{' {
			depth++
		} else if c == '}' {
			if depth > 0 {
				depth--
			}
		} else if c == '\'' || c == '"' {
			p = SkipEmbeddedQuoted(src, p)
			continue
		} else if c == '`' {
			p = SkipEmbeddedTemplate(src, p)
			continue
		} else if c == ';' {
			return p + 1
		} else if depth == 0 && isIdentStart(c) {
			q := p + 1
			for q < n && isIdentChar(src[q]) {
				q++
			}
			if string(src[p:q]) == "from" {
				m := skipTrivia(src, q)
				if m < n && (src[m] == '\'' || src[m] == '"') {
					if spec, next, ok := readEmbeddedQuoted(src, m); ok {
						add(spec)
						return next
					}
				}
				return q
			}
			p = q
			continue
		}
		p++
	}
	return p
}

func parseEmbeddedExport(src []byte, j int, add func(string)) int {
	n := len(src)
	depth := 0
	p := skipTrivia(src, j)
	for p < n {
		if p+1 < n && src[p] == '/' && (src[p+1] == '/' || src[p+1] == '*') {
			p = skipTrivia(src, p)
			continue
		}
		c := src[p]
		if c == '{' {
			depth++
		} else if c == '}' {
			if depth > 0 {
				depth--
			}
		} else if c == '\'' || c == '"' {
			p = SkipEmbeddedQuoted(src, p)
			continue
		} else if c == '`' {
			p = SkipEmbeddedTemplate(src, p)
			continue
		} else if c == ';' {
			return p + 1
		} else if depth == 0 && c == '*' {
		} else if depth == 0 && isIdentStart(c) {
			q := p + 1
			for q < n && isIdentChar(src[q]) {
				q++
			}
			if string(src[p:q]) == "from" {
				m := skipTrivia(src, q)
				if m < n && (src[m] == '\'' || src[m] == '"') {
					if spec, next, ok := readEmbeddedQuoted(src, m); ok {
						add(spec)
						return next
					}
				}
				return q
			}
			p = q
			continue
		}
		p++
	}
	return p
}

func parseEmbeddedRequire(src []byte, j int, add func(string)) int {
	n := len(src)
	k := skipTrivia(src, j)
	if k >= n || src[k] != '(' {
		return j
	}
	m := skipTrivia(src, k+1)
	if m < n && (src[m] == '\'' || src[m] == '"') {
		if spec, next, ok := readEmbeddedQuoted(src, m); ok {
			after := skipTrivia(src, next)
			if after < n && src[after] == ')' {
				add(spec)
				return after + 1
			}
			return next
		}
	}
	return k + 1
}

func SkipEmbeddedQuoted(src []byte, i int) int {
	quote := src[i]
	j := i + 1
	for j < len(src) {
		if src[j] == '\\' {
			j += 2
			continue
		}
		if src[j] == quote {
			return j + 1
		}
		if src[j] == '\n' {
			return j
		}
		j++
	}
	return len(src)
}

func SkipEmbeddedTemplate(src []byte, i int) int {
	j := i + 1
	for j < len(src) {
		if src[j] == '\\' {
			j += 2
			continue
		}
		if src[j] == '`' {
			return j + 1
		}
		if src[j] == '$' && j+1 < len(src) && src[j+1] == '{' {
			depth := 1
			j += 2
			for j < len(src) && depth > 0 {
				if src[j] == '{' {
					depth++
				} else if src[j] == '}' {
					depth--
				} else if src[j] == '\'' || src[j] == '"' {
					j = SkipEmbeddedQuoted(src, j)
					continue
				} else if src[j] == '`' {
					j = SkipEmbeddedTemplate(src, j)
					continue
				}
				j++
			}
			continue
		}
		j++
	}
	return len(src)
}

func IsEmbeddedRegexStart(src []byte, i int) bool {
	j := i - 1
	for j >= 0 && (src[j] == ' ' || src[j] == '\t' || src[j] == '\n' || src[j] == '\r') {
		j--
	}
	if j < 0 {
		return true
	}
	p := src[j]
	if p == ')' || p == ']' || p == '}' {
		return false
	}
	if isIdentChar(p) || p == '$' || p == '"' || p == '\'' || p == '`' {
		return false
	}
	return true
}

func skipEmbeddedRegex(src []byte, i int) int {
	j := i + 1
	inClass := false
	for j < len(src) {
		c := src[j]
		if c == '\\' {
			j += 2
			continue
		}
		if c == '\n' {
			return j
		}
		if c == '[' {
			inClass = true
		} else if c == ']' {
			inClass = false
		} else if c == '/' && !inClass {
			j++
			for j < len(src) && isIdentChar(src[j]) {
				j++
			}
			return j
		}
		j++
	}
	return len(src)
}

func IsEmbeddedPrecededByDot(src []byte, i int) bool {
	j := i - 1
	for j >= 0 && (src[j] == ' ' || src[j] == '\t' || src[j] == '\n' || src[j] == '\r') {
		j--
	}
	return j >= 0 && src[j] == '.'
}

func readEmbeddedQuoted(src []byte, i int) (string, int, bool) {
	quote := src[i]
	var b strings.Builder
	j := i + 1
	for j < len(src) {
		if src[j] == '\\' && j+1 < len(src) {
			b.WriteByte(src[j+1])
			j += 2
			continue
		}
		if src[j] == quote {
			return b.String(), j + 1, true
		}
		if src[j] == '\n' {
			return "", j, false
		}
		b.WriteByte(src[j])
		j++
	}
	return "", len(src), false
}

func ExtractScript(src []byte) []byte {
	n := len(src)
	i := 0
	for i < n {
		if i+4 <= n && src[i] == '<' && src[i+1] == '!' && src[i+2] == '-' && src[i+3] == '-' {
			j := i + 4
			end := -1
			for j+2 < n {
				if src[j] == '-' && src[j+1] == '-' && src[j+2] == '>' {
					end = j + 3
					break
				}
				j++
			}
			if end < 0 {
				return nil
			}
			i = end
			continue
		}
		if src[i] != '<' {
			i++
			continue
		}
		if i+1 < n && src[i+1] == '/' {
			i += 2
			continue
		}
		name, after, ok := scanTagName(src, i+1)
		if !ok {
			i++
			continue
		}
		if !equalFold(name, "script") {
			i = after
			continue
		}
		closePos, innerStart := scanTagEnd(src, after)
		if closePos < 0 {
			return nil
		}
		if src[closePos-1] == '/' {
			return nil
		}
		end := findCloseTag(src, innerStart, "script")
		if end < 0 {
			return nil
		}
		return src[innerStart:end]
	}
	return nil
}

func ExtractScripts(src []byte) [][]byte {
	n := len(src)
	var out [][]byte
	i := 0
	for i < n {
		if i+4 <= n && src[i] == '<' && src[i+1] == '!' && src[i+2] == '-' && src[i+3] == '-' {
			j := i + 4
			end := -1
			for j+2 < n {
				if src[j] == '-' && src[j+1] == '-' && src[j+2] == '>' {
					end = j + 3
					break
				}
				j++
			}
			if end < 0 {
				return nil
			}
			i = end
			continue
		}
		if src[i] != '<' {
			i++
			continue
		}
		if i+1 < n && src[i+1] == '/' {
			i += 2
			continue
		}
		name, after, ok := scanTagName(src, i+1)
		if !ok {
			i++
			continue
		}
		if !equalFold(name, "script") {
			i = after
			continue
		}
		closePos, innerStart := scanTagEnd(src, after)
		if closePos < 0 {
			return nil
		}
		if src[closePos-1] == '/' {
			i = innerStart
			continue
		}
		end := findCloseTag(src, innerStart, "script")
		if end < 0 {
			return nil
		}
		out = append(out, src[innerStart:end])
		i = end + len("</script>")
	}
	if len(out) == 0 {
		return nil
	}
	return out
}

func scanTagName(src []byte, i int) ([]byte, int, bool) {
	n := len(src)
	j := i
	for j < n && isTagNameChar(src[j]) {
		j++
	}
	if j == i {
		return nil, i, false
	}
	return src[i:j], j, true
}

func scanTagEnd(src []byte, i int) (int, int) {
	n := len(src)
	j := i
	for j < n {
		c := src[j]
		if c == '"' || c == '\'' {
			k := j + 1
			for k < n && src[k] != c {
				k++
			}
			if k >= n {
				return -1, -1
			}
			j = k + 1
			continue
		}
		if c == '>' {
			return j, j + 1
		}
		j++
	}
	return -1, -1
}

func findCloseTag(src []byte, start int, name string) int {
	n := len(src)
	i := start
	for i < n {
		if i+4 <= n && src[i] == '<' && src[i+1] == '!' && src[i+2] == '-' && src[i+3] == '-' {
			j := i + 4
			end := -1
			for j+2 < n {
				if src[j] == '-' && src[j+1] == '-' && src[j+2] == '>' {
					end = j + 3
					break
				}
				j++
			}
			if end < 0 {
				return -1
			}
			i = end
			continue
		}
		if src[i] == '<' && i+1 < n && src[i+1] == '/' {
			raw, _, ok := scanTagName(src, i+2)
			if ok && equalFold(raw, name) {
				return i
			}
		}
		i++
	}
	return -1
}

func isTagNameChar(c byte) bool {
	return c == '_' || c == ':' || c == '-' ||
		(c >= 'a' && c <= 'z') ||
		(c >= 'A' && c <= 'Z') ||
		(c >= '0' && c <= '9')
}

func equalFold(a []byte, b string) bool {
	if len(a) != len(b) {
		return false
	}
	for i := 0; i < len(a); i++ {
		ca := a[i]
		if ca >= 'A' && ca <= 'Z' {
			ca += 'a' - 'A'
		}
		if ca != b[i] {
			return false
		}
	}
	return true
}
