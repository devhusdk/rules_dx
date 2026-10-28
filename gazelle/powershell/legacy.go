package powershell

// This file is a temporary fallback for the tree-sitter grammar gap on
// unquoted comma lists; the follow-up issue deletes it.

import (
	"regexp"
	"sort"
	"strings"
)

var (
	importModuleRE    = regexp.MustCompile(`(?i)(?:^|[^\w-])import-module\b`)
	usingModuleRE     = regexp.MustCompile(`(?im)^[ \t]*using[ \t]+module\b`)
	requiresModulesRE = regexp.MustCompile(`(?i)^#requires\b.*?\s-modules\b`)
	requiredModulesRE = regexp.MustCompile(`(?i)(?:^|[^\w-])requiredmodules[ \t]*=`)
	moduleNameKeyRE   = regexp.MustCompile(`(?i)\bmodulename\s*=\s*(?:'([^']*)'|"([^"]*)"|([\w.\-]+))`)
)

// legacyImports is the regex-and-scanner implementation ParseImports used
// before the tree-sitter parser.
func legacyImports(content []byte, manifest bool) []string {
	clean, masked, requires := scrub(string(content))
	set := make(map[string]bool)
	add := func(names []string) {
		for _, name := range names {
			set[name] = true
		}
	}
	for _, loc := range importModuleRE.FindAllStringIndex(masked, -1) {
		add(importModuleNames(readArgs(clean, masked, loc[1], false)))
	}
	for _, loc := range usingModuleRE.FindAllStringIndex(masked, -1) {
		if args := readArgs(clean, masked, loc[1], false); len(args) > 0 {
			add(moduleNames(args[0]))
		}
	}
	for _, line := range requires {
		loc := requiresModulesRE.FindStringIndex(line)
		if loc == nil {
			continue
		}
		sub, subMasked, _ := scrub(line[loc[1]:])
		add(valueList(readArgs(sub, subMasked, 0, false), 0))
	}
	if manifest {
		for _, loc := range requiredModulesRE.FindAllStringIndex(masked, -1) {
			add(valueList(readArgs(clean, masked, loc[1], false), 0))
		}
	}
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}

// scrub blanks comments and here-strings in clean, additionally blanks string
// contents in masked, and returns the #Requires comment lines it saw. Both
// strings keep the input's length and newlines, so offsets line up.
func scrub(s string) (string, string, []string) {
	clean := []byte(s)
	masked := []byte(s)
	var requires []string
	blank := func(buf []byte, from, to int) {
		for k := from; k < to && k < len(buf); k++ {
			if buf[k] != '\n' {
				buf[k] = ' '
			}
		}
	}
	lineStart := true
	for i := 0; i < len(s); {
		c := s[i]
		switch {
		case c == '\n':
			lineStart = true
			i++
			continue
		case c == ' ' || c == '\t' || c == '\r':
			i++
			continue
		case c == '<' && i+1 < len(s) && s[i+1] == '#':
			end := strings.Index(s[i+2:], "#>")
			stop := len(s)
			if end >= 0 {
				stop = i + 2 + end + 2
			}
			blank(clean, i, stop)
			blank(masked, i, stop)
			i = stop
		case c == '#':
			stop := strings.IndexByte(s[i:], '\n')
			if stop < 0 {
				stop = len(s)
			} else {
				stop += i
			}
			if lineStart {
				requires = append(requires, strings.TrimRight(s[i:stop], "\r"))
			}
			blank(clean, i, stop)
			blank(masked, i, stop)
			i = stop
		case c == '@' && i+1 < len(s) && (s[i+1] == '"' || s[i+1] == '\'') && hereStringOpens(s, i+2):
			term := "\n" + string(s[i+1]) + "@"
			end := strings.Index(s[i+2:], term)
			stop := len(s)
			if end >= 0 {
				stop = i + 2 + end + len(term)
			}
			blank(clean, i, stop)
			blank(masked, i, stop)
			i = stop
		case c == '\'' || c == '"':
			stop := stringEnd(s, i)
			for k := i + 1; k < stop-1; k++ {
				if masked[k] != '\n' {
					masked[k] = '_'
				}
			}
			i = stop
		case c == '`':
			i += 2
		default:
			i++
		}
		lineStart = false
	}
	return string(clean), string(masked), requires
}

func hereStringOpens(s string, i int) bool {
	for ; i < len(s); i++ {
		switch s[i] {
		case ' ', '\t', '\r':
		case '\n':
			return true
		default:
			return false
		}
	}
	return false
}

// stringEnd returns the offset just past the quoted string that opens at i.
func stringEnd(s string, i int) int {
	q := s[i]
	for k := i + 1; k < len(s); k++ {
		switch {
		case q == '"' && s[k] == '`':
			k++
		case s[k] == q:
			if k+1 < len(s) && s[k+1] == q {
				k++
				continue
			}
			return k + 1
		}
	}
	return len(s)
}

// readArgs is readArgsRaw with @{...} and @(...) tokens resolved into the
// grammar-independent shapes moduleNames understands.
func readArgs(clean, masked string, i int, multiline bool) []token {
	out := readArgsRaw(clean, masked, i, multiline)
	for k, t := range out {
		switch t.kind {
		case 'h':
			name := ""
			if m := moduleNameKeyRE.FindStringSubmatch(t.text); m != nil {
				name = m[1] + m[2] + m[3]
			}
			out[k] = token{kind: 'b', text: name}
		case 'a':
			_, inner, _ := scrub(t.text)
			out[k] = token{kind: 'a', items: readArgs(t.text, inner, 0, true)}
		}
	}
	return out
}

// readArgs tokenizes the command arguments that start at offset i of clean,
// stopping at the end of the statement. masked locates statement terminators
// outside strings. With multiline set, newlines separate tokens instead.
func readArgsRaw(clean, masked string, i int, multiline bool) []token {
	var out []token
	for i < len(clean) {
		c := masked[i]
		switch {
		case c == ' ' || c == '\t' || c == '\r':
			i++
		case c == '`' && i+1 < len(clean) && (clean[i+1] == '\n' || clean[i+1] == '\r'):
			i += 2
			if i < len(clean) && clean[i-1] == '\r' && clean[i] == '\n' {
				i++
			}
		case c == '\n':
			if !multiline {
				return out
			}
			i++
		case c == ';' || c == '|' || c == '}' || c == ')' || c == '&':
			return out
		case c == ',':
			out = append(out, token{kind: ','})
			i++
		case c == '\'' || c == '"':
			stop := stringEnd(clean, i)
			inner := clean[i+1 : max(i+1, stop-1)]
			kind := byte('s')
			if c == '"' {
				kind = 'd'
			}
			out = append(out, token{kind: kind, text: inner})
			i = stop
		case c == '(' || c == '{' || (c == '@' && i+1 < len(clean) && (masked[i+1] == '(' || masked[i+1] == '{')):
			open := i
			kind := byte('p')
			if c == '{' {
				kind = 'x'
			}
			if c == '@' {
				open = i + 1
				kind = 'a'
				if masked[open] == '{' {
					kind = 'h'
				}
			}
			stop := balancedEnd(masked, open)
			out = append(out, token{kind: kind, text: clean[open+1 : max(open+1, stop-1)]})
			i = stop
		default:
			start := i
			for i < len(clean) && !strings.ContainsRune(" \t\r\n,;|&(){}", rune(masked[i])) {
				i++
			}
			out = append(out, token{kind: 'b', text: clean[start:i]})
		}
	}
	return out
}

// balancedEnd returns the offset just past the bracket group opening at i;
// masked has string contents blanked, so quoted brackets do not count.
func balancedEnd(masked string, i int) int {
	depth := 0
	for k := i; k < len(masked); k++ {
		switch masked[k] {
		case '(', '{', '[':
			depth++
		case ')', '}', ']':
			depth--
			if depth == 0 {
				return k + 1
			}
		}
	}
	return len(masked)
}
