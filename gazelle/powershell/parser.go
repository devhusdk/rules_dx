package powershell

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
	moduleFileRE      = regexp.MustCompile(`(?i)([A-Za-z0-9_][A-Za-z0-9_.\-]*)\.ps[dm]1\b`)
	moduleIdentRE     = regexp.MustCompile(`^[A-Za-z0-9_][A-Za-z0-9_.\-]*$`)
)

// importSwitches are Import-Module parameters that take no value.
var importSwitches = map[string]bool{
	"-ascustomobject":       true,
	"-confirm":              true,
	"-debug":                true,
	"-disablenamechecking":  true,
	"-force":                true,
	"-global":               true,
	"-noclobber":            true,
	"-passthru":             true,
	"-skipeditioncheck":     true,
	"-usewindowspowershell": true,
	"-verbose":              true,
	"-whatif":               true,
}

// ParseImports returns the module names a PowerShell source imports through
// Import-Module, using module and #Requires -Modules, plus RequiredModules when
// manifest is set (a .psd1). Literal names and paths to .psm1/.psd1 files
// count; variables and other expressions are skipped. Comments, strings and
// here-strings never yield imports.
func ParseImports(content []byte, manifest bool) []string {
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

type token struct {
	kind byte // 'b' bare word, 's' single-quoted, 'd' double-quoted, 'p' (...), 'h' @{...}, 'a' @(...), 'x' {...}, ',' comma
	text string
}

// readArgs tokenizes the command arguments that start at offset i of clean,
// stopping at the end of the statement. masked locates statement terminators
// outside strings. With multiline set, newlines separate tokens instead.
func readArgs(clean, masked string, i int, multiline bool) []token {
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

func isFlag(t token) bool {
	if t.kind != 'b' || len(t.text) < 2 || t.text[0] != '-' {
		return false
	}
	c := t.text[1]
	return (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z')
}

// valueList collects the comma-separated values that start at args[i].
func valueList(args []token, i int) []string {
	var names []string
	for ; i < len(args); i++ {
		if isFlag(args[i]) {
			break
		}
		if args[i].kind == ',' {
			continue
		}
		names = append(names, moduleNames(args[i])...)
		if i+1 >= len(args) || args[i+1].kind != ',' {
			break
		}
	}
	return names
}

// skipValue returns the index after the comma-separated value list at args[i].
func skipValue(args []token, i int) int {
	for i < len(args) && !isFlag(args[i]) {
		if args[i].kind != ',' && (i+1 >= len(args) || args[i+1].kind != ',') {
			return i + 1
		}
		i++
	}
	return i
}

func importModuleNames(args []token) []string {
	var names []string
	positional := false
	for i := 0; i < len(args); {
		t := args[i]
		if !isFlag(t) {
			if !positional {
				names = append(names, valueList(args, i)...)
				positional = true
			}
			i = skipValue(args, i)
			continue
		}
		flag, inline, hasInline := strings.Cut(strings.ToLower(t.text), ":")
		i++
		switch {
		case flag == "-name" || flag == "-fullyqualifiedname":
			if hasInline {
				if inline != "" {
					names = append(names, moduleNames(token{kind: 'b', text: t.text[len(flag)+1:]})...)
					continue
				}
			}
			names = append(names, valueList(args, i)...)
			i = skipValue(args, i)
			positional = true
		case importSwitches[flag] || hasInline:
		default:
			i = skipValue(args, i)
		}
	}
	return names
}

// moduleNames reduces one argument to module names.
func moduleNames(t token) []string {
	var name string
	switch t.kind {
	case 's', 'd', 'b':
		name = moduleFromPath(t.text)
	case 'p':
		if m := moduleFileRE.FindAllStringSubmatch(t.text, -1); len(m) > 0 {
			name = moduleFromPath(m[len(m)-1][1])
		}
	case 'h':
		if m := moduleNameKeyRE.FindStringSubmatch(t.text); m != nil {
			name = moduleFromPath(m[1] + m[2] + m[3])
		}
	case 'a':
		var names []string
		_, masked, _ := scrub(t.text)
		for _, inner := range readArgs(t.text, masked, 0, true) {
			if inner.kind != ',' {
				names = append(names, moduleNames(inner)...)
			}
		}
		return names
	}
	if name == "" {
		return nil
	}
	return []string{name}
}

// moduleFromPath turns a module name or a path to a .psm1/.psd1 file into a
// module name, or "" when the value is computed or names no module.
func moduleFromPath(v string) string {
	v = strings.TrimSpace(v)
	base := v
	if i := strings.LastIndexAny(base, `/\`); i >= 0 {
		base = base[i+1:]
	}
	lower := strings.ToLower(base)
	switch {
	case strings.HasSuffix(lower, ModuleExt) || strings.HasSuffix(lower, ManifestExt):
		base = base[:len(base)-len(ModuleExt)]
	case strings.HasSuffix(lower, ".dll") || strings.HasSuffix(lower, ".ps1") || strings.HasSuffix(lower, ".cdxml") || strings.HasSuffix(lower, ".xml"):
		return ""
	}
	if !moduleIdentRE.MatchString(base) {
		return ""
	}
	return base
}
