package common

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

func ExtractScript(src []byte) []byte {
	scripts := ExtractScripts(src)
	if len(scripts) == 0 {
		return nil
	}
	return scripts[0]
}

func ExtractScripts(src []byte) [][]byte {
	n := len(src)
	var out [][]byte
	i := 0
	for i < n {
		after := skipHTMLComment(src, i)
		if after < 0 {
			return nil
		}
		if after > i {
			i = after
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

func OpensTag(src []byte, name string) bool {
	n := len(src)
	i := 0
	for i < n {
		after := skipHTMLComment(src, i)
		if after < 0 {
			return false
		}
		if after > i {
			i = after
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
		raw, next, ok := scanTagName(src, i+1)
		if !ok {
			i++
			continue
		}
		if equalFold(raw, name) {
			return true
		}
		i = next
	}
	return false
}

func skipHTMLComment(src []byte, i int) int {
	n := len(src)
	if i+4 > n || src[i] != '<' || src[i+1] != '!' || src[i+2] != '-' || src[i+3] != '-' {
		return i
	}
	j := i + 4
	for j+2 < n {
		if src[j] == '-' && src[j+1] == '-' && src[j+2] == '>' {
			return j + 3
		}
		j++
	}
	return -1
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
		after := skipHTMLComment(src, i)
		if after < 0 {
			return -1
		}
		if after > i {
			i = after
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
