package zig

import (
	"sort"
	"strings"
)

type token struct {
	text    string
	literal bool
}

func (t token) is(text string) bool { return !t.literal && t.text == text }

func scan(content []byte) []token {
	s := string(content)
	var out []token
	for i := 0; i < len(s); {
		c := s[i]
		switch {
		case c == '/' && i+1 < len(s) && s[i+1] == '/', c == '\\' && i+1 < len(s) && s[i+1] == '\\':
			for i < len(s) && s[i] != '\n' {
				i++
			}
		case c == '"' || c == '\'':
			j := i + 1
			for j < len(s) && s[j] != c && s[j] != '\n' {
				if s[j] == '\\' {
					j++
				}
				j++
			}
			closed := j < len(s) && s[j] == c
			if j > len(s) {
				j = len(s)
			}
			if c == '"' && closed {
				out = append(out, token{text: s[i+1 : j], literal: true})
			}
			i = j + 1
		case c == '@' || c == '_' || c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z':
			j := i + 1
			for j < len(s) && (s[j] == '_' || s[j] >= 'a' && s[j] <= 'z' || s[j] >= 'A' && s[j] <= 'Z' || s[j] >= '0' && s[j] <= '9') {
				j++
			}
			out = append(out, token{text: s[i:j]})
			i = j
		case c == ' ' || c == '\t' || c == '\r' || c == '\n':
			i++
		default:
			out = append(out, token{text: s[i : i+1]})
			i++
		}
	}
	return out
}

func IsLocalImport(name string) bool {
	return strings.HasSuffix(name, ".zig") || strings.HasSuffix(name, ".zon")
}

func literalCalls(content []byte, builtin string) []string {
	toks := scan(content)
	var out []string
	for i := 0; i+3 < len(toks); i++ {
		if toks[i].is(builtin) && toks[i+1].is("(") && toks[i+2].literal && toks[i+3].is(")") && toks[i+2].text != "" {
			out = append(out, toks[i+2].text)
		}
	}
	return out
}

func sortedUnique(names []string) []string {
	set := make(map[string]struct{}, len(names))
	for _, name := range names {
		set[name] = struct{}{}
	}
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}

func ParseImports(content []byte) []string {
	var names []string
	for _, name := range literalCalls(content, "@import") {
		if !IsLocalImport(name) {
			names = append(names, name)
		}
	}
	return sortedUnique(names)
}

func ParseFileRefs(content []byte) []string {
	var names []string
	for _, name := range literalCalls(content, "@import") {
		if IsLocalImport(name) {
			names = append(names, name)
		}
	}
	return sortedUnique(append(names, literalCalls(content, "@embedFile")...))
}

func ParsePackage(content []byte) (string, error) { return "", nil }

func DefinesMain(content []byte) bool {
	toks := scan(content)
	for i := 0; i+3 < len(toks); i++ {
		if toks[i].is("pub") && toks[i+1].is("fn") && toks[i+2].is("main") && toks[i+3].is("(") {
			return true
		}
	}
	return false
}
