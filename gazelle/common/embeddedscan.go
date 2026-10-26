package common

import "bytes"

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
				i = parseEmbeddedImport(src, j, add)
				continue
			case "export":
				i = parseEmbeddedExport(src, j, add)
				continue
			case "require":
				if isPrecededByDot(src, i) {
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

func parseEmbeddedImport(src []byte, j int, add func(string)) int {
	n := len(src)
	k := skipTrivia(src, j)
	if k < n && src[k] == '.' {
		return k + 1
	}
	if k < n && (src[k] == '\'' || src[k] == '"') {
		if spec, next, ok := parseQuoted(src, k); ok {
			add(spec)
			return next
		}
		return k + 1
	}
	if k < n && src[k] == '(' {
		m := skipTrivia(src, k+1)
		if m < n && (src[m] == '\'' || src[m] == '"') {
			if spec, next, ok := parseQuoted(src, m); ok {
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
			p = skipQuoted(src, p)
			continue
		} else if c == '`' {
			p = skipTemplate(src, p)
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
					if spec, next, ok := parseQuoted(src, m); ok {
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
	p := skipTrivia(src, j)
	if hasWordAt(src, p, "type") {
		p = skipTrivia(src, p+4)
	}
	if p >= n || (src[p] != '{' && src[p] != '*') {
		// A declaration (let, const, default, function, ...): let ScanEmbedded
		// read it so a require( or import( inside it is still found.
		return p
	}
	if src[p] == '*' {
		p = skipTrivia(src, p+1)
		if hasWordAt(src, p, "as") {
			p = skipTrivia(src, p+2)
			if p < n && (src[p] == '\'' || src[p] == '"') {
				p = skipQuoted(src, p)
			} else {
				for p < n && isIdentChar(src[p]) {
					p++
				}
			}
			p = skipTrivia(src, p)
		}
		return parseEmbeddedFrom(src, p, add)
	}
	depth := 0
	for p < n {
		if p+1 < n && src[p] == '/' && (src[p+1] == '/' || src[p+1] == '*') {
			p = skipTrivia(src, p)
			continue
		}
		c := src[p]
		if c == '\'' || c == '"' {
			p = skipQuoted(src, p)
			continue
		}
		if c == '`' {
			p = skipTemplate(src, p)
			continue
		}
		p++
		if c == '{' {
			depth++
		} else if c == '}' {
			depth--
			if depth <= 0 {
				return parseEmbeddedFrom(src, skipTrivia(src, p), add)
			}
		}
	}
	return p
}

// parseEmbeddedFrom takes an optional `from '<spec>'` at p.
func parseEmbeddedFrom(src []byte, p int, add func(string)) int {
	if !hasWordAt(src, p, "from") {
		return p
	}
	m := skipTrivia(src, p+4)
	if m < len(src) && (src[m] == '\'' || src[m] == '"') {
		if spec, next, ok := parseQuoted(src, m); ok {
			add(spec)
			return next
		}
	}
	return m
}

func parseEmbeddedRequire(src []byte, j int, add func(string)) int {
	n := len(src)
	k := skipTrivia(src, j)
	if k >= n || src[k] != '(' {
		return j
	}
	m := skipTrivia(src, k+1)
	if m < n && (src[m] == '\'' || src[m] == '"') {
		if spec, next, ok := parseQuoted(src, m); ok {
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

// IncompleteJS reports whether src ends inside an open bracket, template
// literal, or block comment, so later lines could still complete it.
func IncompleteJS(src []byte) bool {
	n := len(src)
	var stack []byte
	i := 0
	for i < n {
		c := src[i]
		if len(stack) > 0 && stack[len(stack)-1] == '`' {
			switch {
			case c == '\\':
				i += 2
			case c == '`':
				stack = stack[:len(stack)-1]
				i++
			case c == '$' && i+1 < n && src[i+1] == '{':
				stack = append(stack, '}')
				i += 2
			default:
				i++
			}
			continue
		}
		switch {
		case c == '/' && i+1 < n && src[i+1] == '/':
			for i < n && src[i] != '\n' {
				i++
			}
		case c == '/' && i+1 < n && src[i+1] == '*':
			end := bytes.Index(src[i+2:], []byte("*/"))
			if end < 0 {
				return true
			}
			i += end + 4
		case c == '\'' || c == '"':
			i = skipQuoted(src, i)
		case c == '/' && isRegexStart(src, i):
			i = skipRegex(src, i)
		case c == '`':
			stack = append(stack, '`')
			i++
		case c == '(':
			stack = append(stack, ')')
			i++
		case c == '[':
			stack = append(stack, ']')
			i++
		case c == '{':
			stack = append(stack, '}')
			i++
		case len(stack) > 0 && c == stack[len(stack)-1]:
			stack = stack[:len(stack)-1]
			i++
		default:
			i++
		}
	}
	return len(stack) > 0
}
