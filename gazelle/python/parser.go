package python

import (
	"sort"
	"strings"
)

func ParseImports(content []byte) []string {
	blanked, dynamic := blankAndCollect(string(content))
	set := make(map[string]struct{})
	add := func(root string) {
		root = strings.TrimSpace(root)
		if idx := strings.IndexByte(root, '.'); idx >= 0 {
			root = strings.TrimSpace(root[:idx])
		}
		if root == "" || strings.HasPrefix(root, ".") {
			return
		}
		set[root] = struct{}{}
	}
	for _, d := range dynamic {
		add(d)
	}
	collectStatic(blanked, add)
	var out []string
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}

func blankAndCollect(src string) (string, []string) {
	out := []byte(src)
	n := len(out)
	var dynamic []string
	blank := func(from, to int) {
		for k := from; k < to && k < n; k++ {
			if out[k] != '\n' {
				out[k] = ' '
			}
		}
	}
	i := 0
	for i < n {
		c := out[i]
		if c == '#' {
			j := i + 1
			for j < n && out[j] != '\n' {
				j++
			}
			blank(i, j)
			i = j
			continue
		}
		if c == '\'' || c == '"' {
			if i+2 < n && out[i+1] == c && out[i+2] == c {
				end := findTripleEnd(out, i+3, c)
				blank(i, end)
				i = end
				continue
			}
			end := findSingleEnd(out, i+1, c)
			blank(i, end)
			i = end
			continue
		}
		if isIdentStart(c) {
			j := i + 1
			for j < n && isIdentChar(out[j]) {
				j++
			}
			word := string(out[i:j])
			if word == "import_module" {
				if root, end, ok := tryImportModule(out, j); ok {
					dynamic = append(dynamic, root)
					blank(i, end)
					i = end
					continue
				}
			}
			i = j
			continue
		}
		i++
	}
	return string(out), dynamic
}

func tryImportModule(out []byte, pos int) (string, int, bool) {
	n := len(out)
	p := skipInline(out, pos)
	if p >= n || out[p] != '(' {
		return "", 0, false
	}
	p++
	p = skipAll(out, p)
	if p >= n || (out[p] != '\'' && out[p] != '"') {
		return "", 0, false
	}
	quote := out[p]
	if p+2 < n && out[p+1] == quote && out[p+2] == quote {
		end := findTripleEnd(out, p+3, quote)
		if end < 3 || out[end-3] != quote || out[end-2] != quote || out[end-1] != quote {
			return "", 0, false
		}
		body := string(out[p+3 : end-3])
		return strings.TrimSpace(body), end, true
	}
	end := findSingleEnd(out, p+1, quote)
	if end < 1 || out[end-1] != quote {
		return "", 0, false
	}
	body := string(out[p+1 : end-1])
	return strings.TrimSpace(body), end, true
}

func findTripleEnd(out []byte, pos int, quote byte) int {
	n := len(out)
	k := pos
	for k < n {
		if out[k] == '\\' {
			k += 2
			continue
		}
		if out[k] == quote && k+2 < n && out[k+1] == quote && out[k+2] == quote {
			return k + 3
		}
		k++
	}
	return n
}

func findSingleEnd(out []byte, pos int, quote byte) int {
	n := len(out)
	k := pos
	for k < n {
		if out[k] == '\\' {
			k += 2
			continue
		}
		if out[k] == quote {
			return k + 1
		}
		if out[k] == '\n' {
			return k
		}
		k++
	}
	return n
}

func collectStatic(s string, add func(string)) {
	out := []byte(s)
	n := len(out)
	i := 0
	for i < n {
		c := out[i]
		if !isIdentStart(c) {
			i++
			continue
		}
		j := i + 1
		for j < n && isIdentChar(out[j]) {
			j++
		}
		switch string(out[i:j]) {
		case "import":
			i = parseImportList(out, j, add)
		case "from":
			if statementStart(out, i) {
				i = parseFrom(out, j, add)
			} else {
				i = j
			}
		default:
			i = j
		}
	}
}

func parseImportList(out []byte, pos int, add func(string)) int {
	n := len(out)
	p := pos
	for p < n {
		p = skipInline(out, p)
		if p >= n {
			return p
		}
		switch out[p] {
		case '\n':
			return p + 1
		case ';':
			return p + 1
		case ',':
			p++
			continue
		case '\\':
			p = skipAll(out, p+1)
			continue
		case '.':
			p++
			continue
		}
		if !isIdentStart(out[p]) {
			p++
			continue
		}
		root, next := scanDotted(out, p)
		add(root)
		p = skipInline(out, next)
		if hasWord(out, p, "as") {
			p = skipIdent(out, p+2)
			p = skipInline(out, p)
			if p < n && isIdentStart(out[p]) {
				p = skipIdent(out, p)
			}
		}
	}
	return p
}

func parseFrom(out []byte, pos int, add func(string)) int {
	n := len(out)
	p := skipInline(out, pos)
	dots := 0
	for p < n && out[p] == '.' {
		dots++
		p++
	}
	p = skipInline(out, p)
	module := ""
	if p < n && isIdentStart(out[p]) {
		var next int
		module, next = scanDotted(out, p)
		p = next
	}
	if dots == 0 && module != "" {
		add(module)
	}
	p = skipInline(out, p)
	if p < n && out[p] == '\\' {
		p = skipAll(out, p+1)
	}
	if !hasWord(out, p, "import") {
		if module == "" {
			if p < n && out[p] != '\n' {
				return p + 1
			}
			return p
		}
		return p
	}
	p += len("import")
	p = skipInline(out, p)
	if p < n && out[p] == '\\' {
		p = skipAll(out, p+1)
	}
	if p < n && out[p] == '(' {
		for p < n && out[p] != ')' {
			p++
		}
		if p < n {
			return p + 1
		}
		return p
	}
	for p < n && out[p] != '\n' && out[p] != ';' {
		p++
	}
	if p < n {
		return p + 1
	}
	return p
}

func scanDotted(out []byte, pos int) (string, int) {
	n := len(out)
	p := skipIdent(out, pos)
	for {
		q := skipInline(out, p)
		if q < n && out[q] == '.' {
			r := skipInline(out, q+1)
			if r < n && isIdentStart(out[r]) {
				p = skipIdent(out, r)
				continue
			}
		}
		return string(out[pos:p]), p
	}
}

func hasWord(out []byte, pos int, word string) bool {
	if pos+len(word) > len(out) {
		return false
	}
	if string(out[pos:pos+len(word)]) != word {
		return false
	}
	after := pos + len(word)
	return after >= len(out) || !isIdentChar(out[after])
}

func skipIdent(out []byte, pos int) int {
	n := len(out)
	p := pos
	for p < n && isIdentChar(out[p]) {
		p++
	}
	return p
}

func statementStart(out []byte, pos int) bool {
	for k := pos - 1; k >= 0; k-- {
		switch out[k] {
		case ' ', '\t', '\r':
			continue
		case '\n', ';', ':':
			return true
		default:
			return false
		}
	}
	return true
}

func skipInline(out []byte, pos int) int {
	for pos < len(out) && (out[pos] == ' ' || out[pos] == '\t' || out[pos] == '\r') {
		pos++
	}
	return pos
}

func skipAll(out []byte, pos int) int {
	for pos < len(out) && (out[pos] == ' ' || out[pos] == '\t' || out[pos] == '\r' || out[pos] == '\n' || out[pos] == ';') {
		pos++
	}
	return pos
}

func isIdentStart(c byte) bool {
	return c == '_' || (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z')
}

func isIdentChar(c byte) bool {
	return isIdentStart(c) || (c >= '0' && c <= '9')
}
