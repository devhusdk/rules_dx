package common

import (
	"bytes"
	"path"
	"strings"
)

type blockComment struct {
	start  string
	end    string
	nested bool
}

var cStyleComments = []blockComment{{start: "/*", end: "*/"}}

var fStyleComments = []blockComment{
	{start: "/*", end: "*/"},
	{start: "(*", end: "*)", nested: true},
}

func MaskJavaStyle(content []byte) []byte {
	return maskCLike(content, cStyleComments)
}

func MaskCSharpStyle(content []byte) []byte {
	return maskCLike(content, cStyleComments)
}

func MaskFSharpStyle(content []byte) []byte {
	return maskCLike(content, fStyleComments)
}

func maskCLike(content []byte, comments []blockComment) []byte {
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
		if next, ok := maskQuoted(out, i, '\'', mask); ok {
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
			j += 2
			continue
		}
		if s[j] == quote {
			closed = true
			j++
			break
		}
		if quote == '"' && s[j] == '\n' {
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

func NormalizeDotted(dotted string, onDemand bool, isStdLib func(string) bool) string {
	dotted = strings.TrimSpace(dotted)
	if dotted == "" {
		return ""
	}
	if isStdLib(dotted) {
		return dotted
	}
	if onDemand {
		return path.Base(strings.ReplaceAll(dotted, ".", "/"))
	}
	if i := strings.LastIndexByte(dotted, '.'); i >= 0 {
		return dotted[i+1:]
	}
	return dotted
}
