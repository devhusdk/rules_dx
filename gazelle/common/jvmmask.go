package common

import (
	"bytes"
	"path/filepath"
	"strings"
	"unicode"
	"unicode/utf8"
)

type blockComment struct {
	start    string
	end      string
	nested   bool
	operator string
}

var cStyleComments = []blockComment{{start: "/*", end: "*/"}}

var fStyleComments = []blockComment{
	{start: "/*", end: "*/"},
	{start: "(*", end: "*)", nested: true, operator: "(*)"},
}

func MaskCSharpStyle(content []byte) []byte {
	return maskCLike(content, cStyleComments, maskSingleQuoted)
}

func MaskFSharpStyle(content []byte) []byte {
	return maskCLike(content, fStyleComments, maskCharLiteral)
}

// maskSingleQuoted masks a single-quoted literal, or an unclosed quote to the
// end of its line.
func maskSingleQuoted(s []byte, pos int, mask func(int, int)) (int, bool) {
	return maskQuoted(s, pos, '\'', mask)
}

// maskCharLiteral masks only a closed one-line character literal, so type
// variables ('a) and primed names (x') never open a literal.
func maskCharLiteral(s []byte, pos int, mask func(int, int)) (int, bool) {
	if s[pos] != '\'' {
		return 0, false
	}
	if pos > 0 {
		b := s[pos-1]
		if b == '_' || b == '\'' || b >= '0' && b <= '9' || b >= 'a' && b <= 'z' || b >= 'A' && b <= 'Z' || b >= 0x80 {
			return 0, false
		}
	}
	j := pos + 1
	if j >= len(s) {
		return 0, false
	}
	if s[j] == '\\' {
		j++
		if j >= len(s) {
			return 0, false
		}
		c := s[j]
		n, digits := 0, isHexDigit
		switch {
		case c >= '0' && c <= '9' && j+2 < len(s) && s[j+1] >= '0' && s[j+1] <= '9':
			n, digits = 3, func(b byte) bool { return b >= '0' && b <= '9' }
		case strings.IndexByte("ntbrafv\\\"'0", c) >= 0:
			j++
		case c == 'x':
			n, j = 2, j+1
		case c == 'u':
			n, j = 4, j+1
		case c == 'U':
			n, j = 8, j+1
		default:
			return 0, false
		}
		for k := 0; k < n; k++ {
			if j >= len(s) || !digits(s[j]) {
				return 0, false
			}
			j++
		}
	} else {
		if s[j] == '\'' || s[j] == '\n' || s[j] == '\r' {
			return 0, false
		}
		_, size := utf8.DecodeRune(s[j:])
		j += size
	}
	if j >= len(s) || s[j] != '\'' {
		return 0, false
	}
	j++
	mask(pos, j)
	return j, true
}

func isHexDigit(b byte) bool {
	return b >= '0' && b <= '9' || b >= 'a' && b <= 'f' || b >= 'A' && b <= 'F'
}

func maskCLike(content []byte, comments []blockComment, maskChar func([]byte, int, func(int, int)) (int, bool)) []byte {
	out := make([]byte, len(content))
	copy(out, content)
	mask := func(from, to int) {
		for i := from; i < to && i < len(out); i++ {
			if out[i] != '\n' {
				out[i] = ' '
			}
		}
	}
	i := 0
	for i < len(out) {
		if next, ok := maskLineComment(out, i, mask); ok {
			i = next
			continue
		}
		if next, ok := maskBlockComment(out, i, comments, mask); ok {
			i = next
			continue
		}
		if next, ok := maskRawString(out, i, mask); ok {
			i = next
			continue
		}
		if next, ok := maskVerbatimString(out, i, mask); ok {
			i = next
			continue
		}
		if next, ok := maskQuoted(out, i, '"', mask); ok {
			i = next
			continue
		}
		if next, ok := maskChar(out, i, mask); ok {
			i = next
			continue
		}
		i++
	}
	return out
}

func maskLineComment(s []byte, pos int, mask func(int, int)) (int, bool) {
	if !bytes.HasPrefix(s[pos:], []byte("//")) {
		return 0, false
	}
	j := pos + 2
	for j < len(s) && s[j] != '\n' {
		j++
	}
	mask(pos, j)
	return j, true
}

func maskBlockComment(s []byte, pos int, comments []blockComment, mask func(int, int)) (int, bool) {
	var open blockComment
	found := false
	for _, c := range comments {
		if c.operator != "" && bytes.HasPrefix(s[pos:], []byte(c.operator)) {
			continue
		}
		if bytes.HasPrefix(s[pos:], []byte(c.start)) {
			open = c
			found = true
			break
		}
	}
	if !found {
		return 0, false
	}
	j := pos + len(open.start)
	depth := 1
	for j < len(s) {
		if open.operator != "" && bytes.HasPrefix(s[j:], []byte(open.operator)) {
			j += len(open.operator)
			continue
		}
		if open.nested && bytes.HasPrefix(s[j:], []byte(open.start)) {
			depth++
			j += len(open.start)
			continue
		}
		if bytes.HasPrefix(s[j:], []byte(open.end)) {
			depth--
			j += len(open.end)
			if depth == 0 {
				break
			}
			continue
		}
		j++
	}
	mask(pos, j)
	return j, true
}

func maskRawString(s []byte, pos int, mask func(int, int)) (int, bool) {
	if !bytes.HasPrefix(s[pos:], []byte(`"""`)) {
		return 0, false
	}
	rest := bytes.Index(s[pos+3:], []byte(`"""`))
	if rest < 0 {
		mask(pos, len(s))
		return len(s), true
	}
	end := pos + 3 + rest + 3
	mask(pos, end)
	return end, true
}

func maskVerbatimString(s []byte, pos int, mask func(int, int)) (int, bool) {
	if s[pos] != '@' {
		return 0, false
	}
	j := pos + 1
	if j < len(s) && s[j] == '$' {
		j++
	}
	if j >= len(s) || s[j] != '"' {
		return 0, false
	}
	rest := bytes.IndexByte(s[j+1:], '"')
	if rest < 0 {
		mask(pos, len(s))
		return len(s), true
	}
	end := j + 1 + rest + 1
	mask(pos, end)
	return end, true
}

func maskQuoted(s []byte, pos int, quote byte, mask func(int, int)) (int, bool) {
	if s[pos] != quote {
		return 0, false
	}
	j := pos + 1
	closed := false
	for j < len(s) {
		if s[j] == '\\' {
			if j+1 < len(s) && s[j+1] == '\n' {
				break
			}
			j += 2
			continue
		}
		if s[j] == quote {
			closed = true
			j++
			break
		}
		if s[j] == '\n' {
			break
		}
		j++
	}
	if !closed {
		j = pos
		for j < len(s) && s[j] != '\n' {
			j++
		}
	}
	mask(pos, j)
	return j, true
}

// JVMCandidates lists the names a qualified JVM import may resolve through when
// no rule indexes the import itself: its enclosing classes, then its own
// package. The walk stops at the first prefix whose last segment starts
// lowercase, so it never climbs into a parent package, and it keeps only dotted
// prefixes. A wildcard pkg.* has the single candidate pkg.
func JVMCandidates(name string) []string {
	if pkg, ok := strings.CutSuffix(name, ".*"); ok {
		if pkg == "" {
			return nil
		}
		return []string{pkg}
	}
	var out []string
	for {
		i := strings.LastIndexByte(name, '.')
		if i < 0 {
			return out
		}
		name = name[:i]
		j := strings.LastIndexByte(name, '.')
		if j < 0 {
			return out
		}
		out = append(out, name)
		if last := name[j+1:]; last != "" && !unicode.IsUpper(rune(last[0])) {
			return out
		}
	}
}

// JVMQualifiedIDs returns the package and the package-qualified class identity
// of a JVM source, or nothing when the source declares no package.
func JVMQualifiedIDs(path string, parsePackage func(content []byte) (string, error), identity func(name string) string) []string {
	content, err := ReadSource(path)
	if err != nil {
		return nil
	}
	pkg, err := parsePackage(content)
	if err != nil || pkg == "" {
		return nil
	}
	return []string{pkg, pkg + "." + identity(filepath.Base(path))}
}

func NormalizeDotted(dotted string, isStdLib func(string) bool) string {
	dotted = strings.TrimSpace(dotted)
	if dotted == "" {
		return ""
	}
	if isStdLib(dotted) {
		return dotted
	}
	if i := strings.LastIndexByte(dotted, '.'); i >= 0 {
		return dotted[i+1:]
	}
	return dotted
}
