package d

import (
	"fmt"
	"strings"
)

type kind byte

const (
	kIdent kind = iota
	// kString is a string literal with no escapes and no postfix, so its text is its value.
	kString
	// kOther is any other literal: numbers, characters, escaped or postfixed strings, token strings.
	kOther
	kPunct
)

type token struct {
	kind kind
	text string
	line int
}

func (t token) is(text string) bool { return t.kind != kString && t.kind != kOther && t.text == text }

type lexer struct {
	src  string
	pos  int
	line int
}

// lex splits D source into tokens. Comments, whitespace, a shebang line and
// #line directives are dropped, and lexing stops at __EOF__. An unterminated
// literal or comment fails with its line.
func lex(src string) ([]token, error) {
	l := &lexer{src: src, line: 1}
	if strings.HasPrefix(src, "#!") {
		l.skipLine()
	}
	var out []token
	for {
		tok, ok, err := l.next()
		if err != nil {
			return nil, err
		}
		if !ok || tok.kind == kIdent && tok.text == "__EOF__" {
			return out, nil
		}
		out = append(out, tok)
	}
}

func (l *lexer) errf(line int, format string, args ...any) error {
	return fmt.Errorf("syntax error at line %d: %s", line, fmt.Sprintf(format, args...))
}

func (l *lexer) peek(off int) byte {
	if l.pos+off < len(l.src) {
		return l.src[l.pos+off]
	}
	return 0
}

func (l *lexer) skipLine() {
	for l.pos < len(l.src) && l.src[l.pos] != '\n' {
		l.pos++
	}
}

func isIdentStart(c byte) bool {
	return c == '_' || c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= 0x80
}

func isDigit(c byte) bool { return c >= '0' && c <= '9' }

func isIdentChar(c byte) bool { return isIdentStart(c) || isDigit(c) }

// next returns the next token, or false at the end of the input.
func (l *lexer) next() (token, bool, error) {
	for l.pos < len(l.src) {
		c := l.src[l.pos]
		switch {
		case c == '\n':
			l.line++
			l.pos++
		case c == ' ' || c == '\t' || c == '\r' || c == '\f' || c == '\v':
			l.pos++
		case c == '#':
			l.skipLine()
		case c == '/' && l.peek(1) == '/':
			l.skipLine()
		case c == '/' && l.peek(1) == '*':
			if err := l.blockComment(); err != nil {
				return token{}, false, err
			}
		case c == '/' && l.peek(1) == '+':
			if err := l.nestedComment(); err != nil {
				return token{}, false, err
			}
		default:
			tok, err := l.token()
			return tok, err == nil, err
		}
	}
	return token{}, false, nil
}

func (l *lexer) blockComment() error {
	start := l.line
	end := strings.Index(l.src[l.pos+2:], "*/")
	if end < 0 {
		return l.errf(start, "unterminated /* comment")
	}
	l.line += strings.Count(l.src[l.pos:l.pos+2+end], "\n")
	l.pos += end + 4
	return nil
}

func (l *lexer) nestedComment() error {
	start := l.line
	depth := 0
	for l.pos < len(l.src) {
		switch {
		case l.src[l.pos] == '/' && l.peek(1) == '+':
			depth++
			l.pos += 2
		case l.src[l.pos] == '+' && l.peek(1) == '/':
			depth--
			l.pos += 2
			if depth == 0 {
				return nil
			}
		default:
			if l.src[l.pos] == '\n' {
				l.line++
			}
			l.pos++
		}
	}
	return l.errf(start, "unterminated /+ comment")
}

func (l *lexer) token() (token, error) {
	c := l.src[l.pos]
	line := l.line
	switch {
	case isIdentStart(c):
		return l.wordOrPrefixed()
	case isDigit(c) || c == '.' && isDigit(l.peek(1)):
		return l.number(), nil
	case c == '.' && l.peek(1) == '.':
		l.pos += 2
		return token{kind: kPunct, text: "..", line: line}, nil
	case c == '"':
		return l.quoted('"', true)
	case c == '`':
		return l.quoted('`', false)
	case c == '\'':
		return l.char()
	}
	l.pos++
	return token{kind: kPunct, text: string(c), line: line}, nil
}

func (l *lexer) wordOrPrefixed() (token, error) {
	rest := l.src[l.pos:]
	prefixed := func(prefix string) bool { return strings.HasPrefix(rest, prefix) }
	switch {
	case prefixed(`r"`):
		l.pos++
		return l.quoted('"', false)
	case prefixed(`x"`):
		l.pos++
		return l.quoted('"', false)
	case prefixed(`q"`):
		l.pos++
		return l.delimited()
	case prefixed("q{"):
		l.pos++
		return l.tokenString()
	case prefixed(`i"`):
		l.pos++
		return l.interpolated('"')
	case prefixed("i`"):
		l.pos++
		return l.interpolated('`')
	case prefixed("iq{"):
		l.pos += 2
		return l.tokenString()
	case prefixed(`iq"`):
		l.pos += 2
		return l.delimited()
	}
	start := l.pos
	for l.pos < len(l.src) && isIdentChar(l.src[l.pos]) {
		l.pos++
	}
	return token{kind: kIdent, text: l.src[start:l.pos], line: l.line}, nil
}

func (l *lexer) number() token {
	start := l.pos
	hex := l.peek(0) == '0' && (l.peek(1) == 'x' || l.peek(1) == 'X')
	for l.pos < len(l.src) {
		c := l.src[l.pos]
		switch {
		case isIdentChar(c):
			l.pos++
			sign := l.peek(0) == '+' || l.peek(0) == '-'
			if sign && (!hex && (c == 'e' || c == 'E') || hex && (c == 'p' || c == 'P')) {
				l.pos++
			}
		case c == '.' && isDigit(l.peek(1)):
			l.pos++
		default:
			return token{kind: kOther, text: l.src[start:l.pos], line: l.line}
		}
	}
	return token{kind: kOther, text: l.src[start:l.pos], line: l.line}
}

func (l *lexer) postfix() bool {
	if c := l.peek(0); (c == 'c' || c == 'w' || c == 'd') && !isIdentChar(l.peek(1)) {
		l.pos++
		return true
	}
	return false
}

// quoted lexes a string whose opening quote is at pos. With escapes, a
// backslash skips the next character.
func (l *lexer) quoted(quote byte, escapes bool) (token, error) {
	line := l.line
	start := l.pos + 1
	plain := true
	i := start
	for ; i < len(l.src) && l.src[i] != quote; i++ {
		switch {
		case l.src[i] == '\n':
			l.line++
		case l.src[i] == '\\' && escapes:
			plain = false
			if i+1 < len(l.src) && l.src[i+1] == '\n' {
				l.line++
			}
			i++
		}
	}
	if i >= len(l.src) {
		return token{}, l.errf(line, "unterminated string")
	}
	l.pos = i + 1
	text := l.src[start:i]
	if l.postfix() {
		plain = false
	}
	if plain {
		return token{kind: kString, text: text, line: line}, nil
	}
	return token{kind: kOther, line: line}, nil
}

func (l *lexer) char() (token, error) {
	line := l.line
	i := l.pos + 1
	for ; i < len(l.src) && l.src[i] != '\''; i++ {
		if l.src[i] == '\n' {
			return token{}, l.errf(line, "unterminated character literal")
		}
		if l.src[i] == '\\' {
			i++
		}
	}
	if i >= len(l.src) {
		return token{}, l.errf(line, "unterminated character literal")
	}
	l.pos = i + 1
	l.postfix()
	return token{kind: kOther, line: line}, nil
}

var closers = map[byte]byte{'(': ')', '[': ']', '{': '}', '<': '>'}

// delimited lexes q"(...)" with nesting brackets, q"/.../" with a single
// delimiter and the heredoc q"ID ... ID". pos is at the opening quote.
func (l *lexer) delimited() (token, error) {
	line := l.line
	body := l.pos + 1
	if body >= len(l.src) {
		return token{}, l.errf(line, "unterminated delimited string")
	}
	open := l.src[body]
	var end int
	switch {
	case closers[open] != 0:
		end = l.closeNested(body+1, open, closers[open])
	case isIdentStart(open):
		end = l.closeHeredoc(body)
	default:
		end = strings.Index(l.src[body+1:], string(open)+`"`)
		if end >= 0 {
			end += body + 1 + 1
		}
	}
	if end < 0 {
		return token{}, l.errf(line, "unterminated delimited string")
	}
	l.line += strings.Count(l.src[l.pos:end], "\n")
	l.pos = end + 1
	l.postfix()
	return token{kind: kOther, line: line}, nil
}

// closeNested returns the index of the quote ending a bracketed string.
func (l *lexer) closeNested(from int, open, closer byte) int {
	depth := 1
	for i := from; i < len(l.src); i++ {
		switch l.src[i] {
		case open:
			depth++
		case closer:
			depth--
			if depth == 0 {
				if i+1 < len(l.src) && l.src[i+1] == '"' {
					return i + 1
				}
				return -1
			}
		}
	}
	return -1
}

func (l *lexer) closeHeredoc(from int) int {
	nl := strings.IndexByte(l.src[from:], '\n')
	if nl < 0 {
		return -1
	}
	id := l.src[from : from+nl]
	end := strings.Index(l.src[from+nl:], "\n"+id+`"`)
	if end < 0 {
		return -1
	}
	return from + nl + end + len(id) + 1
}

// tokenString lexes q{...}. pos is at the opening brace. Its contents are
// lexed so that braces inside strings and comments do not count.
func (l *lexer) tokenString() (token, error) {
	line := l.line
	l.pos++
	depth := 1
	for depth > 0 {
		tok, ok, err := l.next()
		if err != nil {
			return token{}, err
		}
		if !ok {
			return token{}, l.errf(line, "unterminated token string")
		}
		switch {
		case tok.is("{"):
			depth++
		case tok.is("}"):
			depth--
		}
	}
	l.postfix()
	return token{kind: kOther, line: line}, nil
}

// interpolated lexes i"..." and i`...`. A $(...) expression inside may hold
// quotes of its own, so it is lexed as code.
func (l *lexer) interpolated(quote byte) (token, error) {
	line := l.line
	l.pos++
	for l.pos < len(l.src) {
		c := l.src[l.pos]
		switch {
		case c == quote:
			l.pos++
			l.postfix()
			return token{kind: kOther, line: line}, nil
		case c == '\n':
			l.line++
			l.pos++
		case c == '\\' && quote == '"':
			if l.peek(1) == '\n' {
				l.line++
			}
			l.pos += 2
		case c == '$' && l.peek(1) == '(':
			l.pos += 2
			if err := l.skipParens(line); err != nil {
				return token{}, err
			}
		default:
			l.pos++
		}
	}
	return token{}, l.errf(line, "unterminated string")
}

func (l *lexer) skipParens(line int) error {
	depth := 1
	for depth > 0 {
		tok, ok, err := l.next()
		if err != nil {
			return err
		}
		if !ok {
			return l.errf(line, "unterminated string")
		}
		switch {
		case tok.is("("):
			depth++
		case tok.is(")"):
			depth--
		}
	}
	return nil
}
