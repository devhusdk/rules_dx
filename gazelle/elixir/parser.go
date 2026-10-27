package elixir

import (
	"sort"
	"strings"
	"unicode/utf8"
)

const moduleKeyword = "__MODULE__"

type tokKind int

const (
	tkNone tokKind = iota
	tkAlias
	tkIdent
	tkKey
	tkAtom
	tkString
	tkPunct
)

type token struct {
	kind     tokKind
	val      string
	interp   bool
	afterDot bool
}

func (t token) is(kind tokKind, val string) bool { return t.kind == kind && t.val == val }

type lexer struct {
	s    string
	i    int
	toks []token
}

// lex splits Elixir source into code tokens. Comments, string, charlist and
// sigil bodies become single tokens; the code inside #{} is lexed inline.
func lex(s string) []token {
	l := &lexer{s: s}
	l.code(false)
	return l.toks
}

func (l *lexer) peek(k int) byte {
	if l.i+k < len(l.s) {
		return l.s[l.i+k]
	}
	return 0
}

func isUpper(c byte) bool { return c >= 'A' && c <= 'Z' }
func isLower(c byte) bool { return c >= 'a' && c <= 'z' }
func isDigit(c byte) bool { return c >= '0' && c <= '9' }
func isIdentStart(c byte) bool {
	return isLower(c) || c == '_' || c >= 0x80
}
func isIdentChar(c byte) bool {
	return isUpper(c) || isLower(c) || isDigit(c) || c == '_' || c >= 0x80
}

func (l *lexer) emit(t token) { l.toks = append(l.toks, t) }

func (l *lexer) code(nested bool) {
	depth := 0
	for l.i < len(l.s) {
		c := l.s[l.i]
		switch {
		case c == '#':
			for l.i < len(l.s) && l.s[l.i] != '\n' {
				l.i++
			}
		case c == ' ' || c == '\t' || c == '\n' || c == '\r':
			l.i++
		case c == '"' || c == '\'':
			l.str(c, true)
		case c == '~' && (isLower(l.peek(1)) || isUpper(l.peek(1))) && l.sigil():
		case c == '?':
			l.i++
			if l.peek(0) == '\\' {
				l.i++
			}
			if l.i < len(l.s) {
				_, size := utf8.DecodeRuneInString(l.s[l.i:])
				l.i += size
			}
		case c == ':':
			l.atom()
		case isUpper(c):
			l.alias()
		case isIdentStart(c):
			l.ident()
		case isDigit(c):
			l.i++
			for l.i < len(l.s) && (isIdentChar(l.s[l.i]) || (l.s[l.i] == '.' && isDigit(l.peek(1)))) {
				l.i++
			}
		case c == '{':
			depth++
			l.i++
			l.emit(token{kind: tkPunct, val: "{"})
		case c == '}':
			l.i++
			if nested && depth == 0 {
				return
			}
			depth--
			l.emit(token{kind: tkPunct, val: "}"})
		default:
			l.i++
			l.emit(token{kind: tkPunct, val: string(c)})
		}
	}
}

func (l *lexer) afterDot(start int) bool {
	return start > 0 && l.s[start-1] == '.' && (start < 2 || l.s[start-2] != '.')
}

func (l *lexer) isKeySuffix() bool {
	return l.peek(0) == ':' && l.peek(1) != ':'
}

func (l *lexer) word() string {
	start := l.i
	for l.i < len(l.s) && isIdentChar(l.s[l.i]) {
		l.i++
	}
	if l.i < len(l.s) && (l.s[l.i] == '?' || l.s[l.i] == '!') {
		l.i++
	}
	return l.s[start:l.i]
}

func (l *lexer) dotted(head string) string {
	for l.peek(0) == '.' && isUpper(l.peek(1)) {
		l.i++
		start := l.i
		for l.i < len(l.s) && isIdentChar(l.s[l.i]) {
			l.i++
		}
		head += "." + l.s[start:l.i]
	}
	return head
}

func (l *lexer) alias() {
	start := l.i
	for l.i < len(l.s) && isIdentChar(l.s[l.i]) {
		l.i++
	}
	name := l.dotted(l.s[start:l.i])
	if l.isKeySuffix() && !strings.Contains(name, ".") {
		l.i++
		l.emit(token{kind: tkKey, val: name})
		return
	}
	l.emit(token{kind: tkAlias, val: name, afterDot: l.afterDot(start)})
}

func (l *lexer) ident() {
	start := l.i
	name := l.word()
	if name == moduleKeyword {
		l.emit(token{kind: tkAlias, val: l.dotted(name), afterDot: l.afterDot(start)})
		return
	}
	if l.isKeySuffix() {
		l.i++
		l.emit(token{kind: tkKey, val: name})
		return
	}
	l.emit(token{kind: tkIdent, val: name, afterDot: l.afterDot(start)})
}

func (l *lexer) atom() {
	switch next := l.peek(1); {
	case next == ':':
		l.i += 2
		l.emit(token{kind: tkPunct, val: "::"})
	case next == '"' || next == '\'':
		l.i++
		idx := l.str(next, true)
		l.toks[idx].kind = tkAtom
	case isIdentStart(next) || isUpper(next):
		l.i++
		l.emit(token{kind: tkAtom, val: l.word()})
	default:
		l.i++
		l.emit(token{kind: tkPunct, val: ":"})
	}
}

// str lexes a quoted body that starts at l.i and returns its token index.
func (l *lexer) str(q byte, interp bool) int {
	closing := string(q)
	if l.peek(1) == q && l.peek(2) == q {
		closing = strings.Repeat(string(q), 3)
	}
	l.i += len(closing)
	return l.body(closing, interp)
}

func (l *lexer) body(closing string, interp bool) int {
	idx := len(l.toks)
	l.emit(token{kind: tkString})
	var b strings.Builder
	hadInterp := false
	for l.i < len(l.s) {
		if strings.HasPrefix(l.s[l.i:], closing) {
			l.i += len(closing)
			break
		}
		c := l.s[l.i]
		switch {
		case c == '\\' && l.i+1 < len(l.s):
			b.WriteString(l.s[l.i : l.i+2])
			l.i += 2
		case interp && c == '#' && l.peek(1) == '{':
			hadInterp = true
			l.i += 2
			l.code(true)
		default:
			b.WriteByte(c)
			l.i++
		}
	}
	l.toks[idx].val = b.String()
	l.toks[idx].interp = hadInterp
	return idx
}

var sigilClose = map[byte]byte{'(': ')', '[': ']', '{': '}', '<': '>', '/': '/', '|': '|', '"': '"', '\'': '\''}

func (l *lexer) sigil() bool {
	j := l.i + 1
	lower := isLower(l.s[j])
	if lower {
		j++
	} else {
		for j < len(l.s) && (isUpper(l.s[j]) || isDigit(l.s[j])) {
			j++
		}
	}
	if j >= len(l.s) {
		return false
	}
	d := l.s[j]
	closeCh, ok := sigilClose[d]
	if !ok {
		return false
	}
	l.i = j
	if (d == '"' || d == '\'') && l.peek(1) == d && l.peek(2) == d {
		l.str(d, lower)
	} else {
		l.i++
		l.body(string(closeCh), lower)
	}
	for l.i < len(l.s) && (isLower(l.s[l.i]) || isUpper(l.s[l.i]) || isDigit(l.s[l.i])) {
		l.i++
	}
	return true
}

type fileInfo struct {
	defs []string
	refs []string
}

type frame struct {
	module string
	depth  int
}

// analyze walks the tokens of one file. The alias map is file-wide: an alias
// applies to every reference in the file, not just the lexical scope that
// declares it.
func analyze(content []byte) fileInfo {
	toks := lex(string(content))
	var stack []frame
	depth := 0
	aliases := make(map[string]string)
	defs := make(map[string]bool)
	type rawRef struct {
		name     string
		absolute bool
	}
	var raws []rawRef
	cur := func() string {
		if len(stack) == 0 {
			return ""
		}
		return stack[len(stack)-1].module
	}
	expand := func(name string) string {
		switch {
		case name == moduleKeyword:
			return cur()
		case strings.HasPrefix(name, moduleKeyword+"."):
			if cur() == "" {
				return ""
			}
			return cur() + strings.TrimPrefix(name, moduleKeyword)
		case strings.HasPrefix(name, "Elixir."):
			return strings.TrimPrefix(name, "Elixir.")
		}
		head, rest, _ := strings.Cut(name, ".")
		if target, ok := aliases[head]; ok {
			if rest == "" {
				return target
			}
			return target + "." + rest
		}
		return name
	}
	at := func(i int) token {
		if i < len(toks) {
			return toks[i]
		}
		return token{}
	}
	for i := 0; i < len(toks); i++ {
		t := toks[i]
		switch t.kind {
		case tkIdent:
			if t.afterDot {
				continue
			}
			switch t.val {
			case "do", "fn":
				depth++
			case "end":
				if len(stack) > 0 && stack[len(stack)-1].depth == depth {
					stack = stack[:len(stack)-1]
				}
				depth--
			case "defmodule", "defprotocol":
				name := at(i + 1)
				if name.kind != tkAlias || name.val == moduleKeyword {
					continue
				}
				full := name.val
				switch {
				case strings.HasPrefix(full, moduleKeyword+"."), strings.HasPrefix(full, "Elixir."):
					full = expand(full)
				case cur() != "":
					head, _, _ := strings.Cut(full, ".")
					aliases[head] = cur() + "." + head
					full = cur() + "." + full
				}
				if full == "" {
					continue
				}
				defs[full] = true
				i++
				if at(i+1).is(tkIdent, "do") {
					depth++
					stack = append(stack, frame{module: full, depth: depth})
					i++
				}
			case "alias":
				target := at(i + 1)
				if target.kind != tkAlias {
					continue
				}
				base := expand(target.val)
				i++
				switch {
				case at(i+1).is(tkPunct, ".") && at(i+2).is(tkPunct, "{"):
					i += 2
					for i+1 < len(toks) && !at(i+1).is(tkPunct, "}") {
						i++
						if toks[i].kind == tkAlias && base != "" {
							full := base + "." + toks[i].val
							aliases[lastSegment(full)] = full
						}
					}
					i++
				case at(i+1).is(tkPunct, ",") && at(i+2).is(tkKey, "as") && at(i+3).kind == tkAlias:
					if base != "" {
						aliases[at(i+3).val] = base
					}
					i += 3
				default:
					if base != "" {
						aliases[lastSegment(base)] = base
					}
				}
			}
		case tkAlias:
			if t.afterDot || t.val == moduleKeyword {
				continue
			}
			if strings.HasPrefix(t.val, moduleKeyword) || strings.HasPrefix(t.val, "Elixir.") {
				if name := expand(t.val); name != "" {
					raws = append(raws, rawRef{name: name, absolute: true})
				}
				continue
			}
			raws = append(raws, rawRef{name: t.val})
		}
	}
	refs := make(map[string]bool)
	for _, r := range raws {
		name := r.name
		if !r.absolute {
			name = expand(name)
		}
		refs[name] = true
	}
	return fileInfo{defs: sortedSet(defs), refs: sortedSet(refs)}
}

func lastSegment(name string) string {
	if i := strings.LastIndexByte(name, '.'); i >= 0 {
		return name[i+1:]
	}
	return name
}

func sortedSet(set map[string]bool) []string {
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}

// ParseDefinitions returns the full names of every defmodule and defprotocol
// in content, nested ones included.
func ParseDefinitions(content []byte) []string { return analyze(content).defs }

// ParseReferences returns every module content names in code, expanded
// through its aliases. A plain alias line is not a reference by itself.
func ParseReferences(content []byte) []string { return analyze(content).refs }
