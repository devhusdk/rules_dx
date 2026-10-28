package cc

import (
	"path"
	"sort"
	"strings"
)

// legacyLex is the hand-written scanner. It is the fallback for sources the
// tree-sitter C++ grammar cannot parse cleanly (truncated literals, stray
// directives), where error recovery would otherwise misplace includes.
func legacyLex(content []byte) lexed {
	s := strings.ReplaceAll(string(content), "\\\r\n", " ")
	s = strings.ReplaceAll(s, "\\\n", " ")
	b := []byte(s)
	code := []byte(s)
	mask := func(from, to int) {
		for k := from; k < to; k++ {
			if code[k] != '\n' {
				code[k] = ' '
			}
		}
	}
	set := make(map[string]struct{})
	var systems []SystemInclude
	var frames []bool
	guarded := func() bool {
		for _, f := range frames {
			if f {
				return true
			}
		}
		return false
	}
	n := len(b)
	i := 0
	for i < n {
		c := b[i]
		if c == '/' && i+1 < n && b[i+1] == '/' {
			j := i + 2
			for j < n && b[j] != '\n' {
				j++
			}
			mask(i, j)
			i = j
			continue
		}
		if c == '/' && i+1 < n && b[i+1] == '*' {
			j := i + 2
			for j+1 < n && !(b[j] == '*' && b[j+1] == '/') {
				j++
			}
			if j+1 < n {
				j += 2
			} else {
				j = n
			}
			mask(i, j)
			i = j
			continue
		}
		if isRawStringStart(b, i) {
			if end, ok := skipRawString(b, i); ok {
				mask(i, end)
				i = end
				continue
			}
		}
		if end := skipPPNumber(b, i); end > i {
			i = end
			continue
		}
		if c == '"' || c == '\'' {
			end := skipQuoted(b, i)
			mask(i, end)
			i = end
			continue
		}
		if c == '#' && isDirectiveStart(b, i) {
			frames = trackConditional(b, i+1, frames)
		}
		if c == '#' && isDirectiveStart(b, i) && hasIncludeKeyword(b, i+1) {
			if raw, next, angle, ok := parseIncludeTarget(b, i+1); ok {
				switch {
				case angle && raw != "":
					systems = append(systems, SystemInclude{Path: path.Clean(raw), Guarded: guarded()})
				case raw != "":
					set[path.Clean(raw)] = struct{}{}
				}
				mask(i, next)
				i = next
				continue
			}
		}
		i++
	}
	includes := make([]string, 0, len(set))
	for name := range set {
		includes = append(includes, name)
	}
	sort.Strings(includes)
	return lexed{code: code, quoted: includes, systems: systems}
}

// trackConditional updates the conditional stack for the directive after a '#'.
func trackConditional(b []byte, pos int, frames []bool) []bool {
	p := pos
	for p < len(b) && (b[p] == ' ' || b[p] == '\t') {
		p++
	}
	q := p
	for q < len(b) && isIdentChar(b[q]) {
		q++
	}
	end := q
	for end < len(b) && b[end] != '\n' {
		end++
	}
	android := strings.Contains(string(b[q:end]), "__ANDROID")
	switch string(b[p:q]) {
	case "if", "ifdef", "ifndef":
		return append(frames, android)
	case "elif", "elifdef", "elifndef":
		if len(frames) > 0 && android {
			frames[len(frames)-1] = true
		}
	case "endif":
		if len(frames) > 0 {
			return frames[:len(frames)-1]
		}
	}
	return frames
}

func isIdentChar(c byte) bool {
	return c == '_' || (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z') || (c >= '0' && c <= '9')
}

func isRawStringStart(b []byte, i int) bool {
	if b[i] != 'R' || i+1 >= len(b) || b[i+1] != '"' {
		return false
	}
	p := i
	if p >= 2 && string(b[p-2:p]) == "u8" {
		p -= 2
	} else if p >= 1 && (b[p-1] == 'u' || b[p-1] == 'U' || b[p-1] == 'L') {
		p--
	}
	return p == 0 || !isIdentChar(b[p-1])
}

func isDirectiveStart(b []byte, pos int) bool {
	j := pos - 1
	for j >= 0 && b[j] != '\n' {
		if b[j] != ' ' && b[j] != '\t' && b[j] != '\r' {
			return false
		}
		j--
	}
	return true
}

func hasIncludeKeyword(b []byte, pos int) bool {
	p := pos
	for p < len(b) && (b[p] == ' ' || b[p] == '\t' || b[p] == '\r') {
		p++
	}
	if p+7 > len(b) || string(b[p:p+7]) != "include" {
		return false
	}
	after := p + 7
	if after < len(b) && isIdentChar(b[after]) {
		return false
	}
	return true
}

func parseIncludePath(b []byte, hashPos int) (string, int, bool) {
	raw, next, angle, ok := parseIncludeTarget(b, hashPos)
	if angle {
		raw = ""
	}
	return raw, next, ok
}

func parseIncludeTarget(b []byte, hashPos int) (string, int, bool, bool) {
	p := hashPos
	for p < len(b) && (b[p] == ' ' || b[p] == '\t' || b[p] == '\r') {
		p++
	}
	if p+7 > len(b) || string(b[p:p+7]) != "include" {
		return "", p, false, false
	}
	p += 7
	for p < len(b) && (b[p] == ' ' || b[p] == '\t' || b[p] == '\r') {
		p++
	}
	if p >= len(b) {
		return "", p, false, false
	}
	if b[p] == '"' {
		j := p + 1
		for j < len(b) && b[j] != '"' && b[j] != '\n' {
			if b[j] == '\\' && j+1 < len(b) {
				j += 2
				continue
			}
			j++
		}
		if j >= len(b) || b[j] != '"' {
			return "", j, false, false
		}
		raw := strings.TrimSpace(string(b[p+1 : j]))
		return raw, j + 1, false, true
	}
	if b[p] == '<' {
		j := p + 1
		for j < len(b) && b[j] != '>' && b[j] != '\n' {
			j++
		}
		if j < len(b) && b[j] == '>' {
			return strings.TrimSpace(string(b[p+1 : j])), j + 1, true, true
		}
		return "", j, false, false
	}
	return "", p, false, false
}

func skipQuoted(b []byte, pos int) int {
	quote := b[pos]
	j := pos + 1
	for j < len(b) {
		if b[j] == '\\' {
			j += 2
			continue
		}
		if b[j] == quote {
			return j + 1
		}
		if b[j] == '\n' {
			return j
		}
		j++
	}
	return len(b)
}

func skipPPNumber(b []byte, pos int) int {
	if pos > 0 && isIdentChar(b[pos-1]) {
		return pos
	}
	j := pos
	if b[j] == '.' {
		j++
	}
	if j >= len(b) || b[j] < '0' || b[j] > '9' {
		return pos
	}
	for j < len(b) {
		c := b[j]
		switch {
		case isIdentChar(c) || c == '.':
			j++
		case c == '\'' && j+1 < len(b) && isIdentChar(b[j+1]):
			j += 2
		case (c == '+' || c == '-') && strings.IndexByte("eEpP", b[j-1]) >= 0:
			j++
		default:
			return j
		}
	}
	return j
}

func skipRawString(b []byte, pos int) (int, bool) {
	q := pos + 2
	for q < len(b) && b[q] != '(' {
		if b[q] == ' ' || b[q] == '\t' || b[q] == '\r' || b[q] == '\n' || b[q] == '\\' {
			return 0, false
		}
		q++
	}
	if q >= len(b) || b[q] != '(' {
		return 0, false
	}
	delim := string(b[pos+2 : q])
	closer := ")" + delim + "\""
	rest := string(b[q+1:])
	idx := strings.Index(rest, closer)
	if idx < 0 {
		return len(b), true
	}
	return q + 1 + idx + len(closer), true
}
