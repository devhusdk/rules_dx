package rust

import (
	"regexp"
	"fmt"
	"path"
	"strings"
)

type ModuleDecl struct {
	Name              string
	Path              string
	Parent            string
	ParentFromFileDir bool
	Inline            bool
	CfgTest           bool
	Scope             string
}

type UseDecl struct {
	Path    string
	Glob    bool
	CfgTest bool
	Scope   string
}

type ExternCrate struct {
	Name    string
	As      string
	CfgTest bool
	Scope   string
}

type LocalName struct {
	Name  string
	Scope string
}

type FileFacts struct {
	Modules      []ModuleDecl
	Uses         []UseDecl
	Externs      []ExternCrate
	Locals       []LocalName
	HasTestAttr  bool
	HasCfgTest   bool
	InnerCfgTest bool
}

type AmbiguousError struct {
	What string
	Attr string
}

func (e *AmbiguousError) Error() string {
	return fmt.Sprintf("rust: ambiguous test context on %s: %s is not exactly #[cfg(test)]; disambiguate with #[cfg(test)] or move the item", e.What, e.Attr)
}

func ParseFacts(path string, content []byte) (*FileFacts, error) {
	p := &parser{path: path, src: cleanSource(string(content))}
	if err := p.parseItems(0, false, &p.facts); err != nil {
		return nil, err
	}
	return &p.facts, nil
}

type parser struct {
	path  string
	src   string
	pos   int
	facts FileFacts
	fnDepth int
}

func cleanSource(s string) string {
	out := []byte(s)
	n := len(out)
	i := 0
	attrDepth := 0
	blank := func(from, to int) {
		for k := from; k < to; k++ {
			if out[k] != '\n' {
				out[k] = ' '
			}
		}
	}
	for i < n {
		c := out[i]
		switch {
		case c == '#' && i+1 < n && (out[i+1] == '[' || (out[i+1] == '!' && i+2 < n && out[i+2] == '[')):
			attrDepth = 1
			if out[i+1] == '!' {
				i += 3
			} else {
				i += 2
			}
		case attrDepth > 0 && c == '[':
			attrDepth++
			i++
		case attrDepth > 0 && c == ']':
			attrDepth--
			i++
		case attrDepth > 0:
			i++
		case c == '/' && i+1 < n && out[i+1] == '/':
			j := i + 2
			for j < n && out[j] != '\n' {
				j++
			}
			blank(i, j)
			i = j
		case c == '/' && i+1 < n && out[i+1] == '*':
			depth := 1
			j := i + 2
			for j < n && depth > 0 {
				if out[j] == '/' && j+1 < n && out[j+1] == '*' {
					depth++
					j += 2
				} else if out[j] == '*' && j+1 < n && out[j+1] == '/' {
					depth--
					j += 2
				} else {
					j++
				}
			}
			blank(i, j)
			i = j
		case c == '"' || (c == 'r' && i+1 < n && (out[i+1] == '"' || out[i+1] == '#')) || (c == 'b' && i+1 < n && (out[i+1] == '"' || out[i+1] == '\'' || (out[i+1] == 'r' && i+2 < n && (out[i+2] == '"' || out[i+2] == '#')))):
			j := scanString(out, i)
			blank(i, j)
			i = j
		case c == '\'':
			if j, ok := scanChar(out, i); ok {
				blank(i, j)
				i = j
			} else {
				i++
			}
		case isIdentStart(c):
			j := i + 1
			for j < n && isIdentChar(out[j]) {
				j++
			}
			word := string(out[i:j])
			if word == "macro_rules" || word == "macro" {
				k := j
				for k < n && (out[k] == ' ' || out[k] == '\t' || out[k] == '\n' || out[k] == '\r' || out[k] == '!') {
					k++
				}
				if k < n && isIdentStart(out[k]) {
					for k < n && isIdentChar(out[k]) {
						k++
					}
					for k < n && (out[k] == ' ' || out[k] == '\t' || out[k] == '\n' || out[k] == '\r') {
						k++
					}
				}
				if k < n && (out[k] == '{' || out[k] == '[' || out[k] == '(') {
					if end, ok := matchBalanced(out, k); ok {
						blank(i, end)
						i = end
						continue
					}
				}
				i = j
			} else {
				i = j
			}
		default:
			i++
		}
	}
	return string(out)
}

func scanString(out []byte, i int) int {
	n := len(out)
	j := i
	if out[j] == 'b' {
		j++
	}
	if j < n && out[j] == 'r' {
		j++
		hashes := 0
		for j < n && out[j] == '#' {
			hashes++
			j++
		}
		if j < n && out[j] == '"' {
			j++
			for j < n {
				if out[j] == '"' {
					k := j + 1
					h := 0
					for h < hashes && k < n && out[k] == '#' {
						h++
						k++
					}
					if h == hashes {
						return k
					}
				}
				j++
			}
			return n
		}
		return i + 1
	}
	if j < n && (out[j] == '"' || out[j] == '\'') {
		q := out[j]
		j++
		for j < n {
			if out[j] == '\\' {
				j += 2
				continue
			}
			if out[j] == q {
				return j + 1
			}
			if out[j] == '\n' && q == '\'' {
				return j
			}
			j++
		}
		return n
	}
	return i + 1
}

func scanChar(out []byte, i int) (int, bool) {
	n := len(out)
	j := i + 1
	if j >= n {
		return 0, false
	}
	if out[j] == '\\' {
		j++
		if j < n && out[j] == 'u' {
			j++
			if j < n && out[j] == '{' {
				for j < n && out[j] != '}' {
					j++
				}
				if j < n {
					j++
				}
			} else {
				for k := 0; k < 4 && j < n; k++ {
					j++
				}
			}
		} else if j < n {
			j++
		}
	} else if out[j] == '\'' || out[j] == '\n' {
		return 0, false
	} else {
		k := j
		for k < n && k-j < 8 && out[k] != '\'' && out[k] != '\n' {
			k++
		}
		if k < n && out[k] == '\'' && k-j >= 1 {
			if k == j+1 || (k == j+2 && out[j] == '\\') {
				return k + 1, true
			}
			return 0, false
		}
		return 0, false
	}
	if j < n && out[j] == '\'' {
		return j + 1, true
	}
	return 0, false
}

func matchBalanced(out []byte, i int) (int, bool) {
	pairs := map[byte]byte{'{': '}', '[': ']', '(': ')'}
	open := out[i]
	want := pairs[open]
	depth := 0
	for j := i; j < len(out); j++ {
		switch out[j] {
		case open:
			depth++
		case want:
			depth--
			if depth == 0 {
				return j + 1, true
			}
		}
	}
	return 0, false
}

func isIdentStart(c byte) bool {
	return c == '_' || (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z')
}

func isIdentChar(c byte) bool {
	return isIdentStart(c) || (c >= '0' && c <= '9')
}

func isSpace(c byte) bool {
	return c == ' ' || c == '\t' || c == '\n' || c == '\r'
}

func (p *parser) parseItems(depth int, testScope bool, facts *FileFacts) error {
	attrs := make([]string, 0, 2)
	braces := 0
	for {
		p.skipSpace()
		if p.pos >= len(p.src) {
			return nil
		}
		if p.src[p.pos] == '}' {
			if braces > 0 {
				braces--
				p.pos++
				attrs = attrs[:0]
				continue
			}
			if depth == 0 {
				p.pos++
				continue
			}
			return nil
		}
		if p.src[p.pos] == '#' {
			attr, inner, ok := p.scanAttr()
			if !ok {
				p.pos++
				attrs = attrs[:0]
				continue
			}
			if inner {
				if p.fnDepth == 0 && isCfgTest(attr) {
					facts.InnerCfgTest = true
				}
				continue
			}
			attrs = append(attrs, attr)
			continue
		}
		word, ok := p.scanWord()
		if !ok {
			if p.src[p.pos] == '{' {
				braces++
			}
			p.pos++
			attrs = attrs[:0]
			continue
		}
		switch word {
		case "mod":
			if err := p.parseMod(attrs, testScope, depth, facts); err != nil {
				return err
			}
		case "use":
			if err := p.parseUse(attrs, testScope, facts); err != nil {
				return err
			}
		case "extern":
			if err := p.parseExtern(attrs, testScope, facts); err != nil {
				return err
			}
		case "fn":
			if err := p.parseFn(attrs, testScope, depth, facts); err != nil {
				return err
			}
		case "enum":
			p.skipSpace()
			if name, ok := p.scanWord(); ok {
				facts.Locals = append(facts.Locals, LocalName{Name: name})
			}
			attrs = attrs[:0]
			continue
		case "pub", "unsafe", "const", "static", "struct", "trait", "impl", "type", "where":
			if word != "pub" {
				attrs = attrs[:0]
			} else {
				p.skipPubRestrict()
			}
			continue
		default:
			if p.fnDepth > 0 {
				p.skipMacroGroup()
			}
			attrs = attrs[:0]
			continue
		}
		attrs = attrs[:0]
	}
}

func (p *parser) parseMod(attrs []string, testScope bool, depth int, facts *FileFacts) error {
	p.skipSpace()
	name, ok := p.scanWord()
	if !ok {
		return fmt.Errorf("rust: %s: expected module name after `mod`", p.path)
	}
	cfgTest := testScope
	pathOverride := ""
	for _, a := range attrs {
		kind, err := classifyTestAttr("module "+name, a)
		if err != nil {
			return err
		}
		switch kind {
		case attrTest:
			cfgTest = true
		}
		if v, ok := parsePathAttr(a); ok {
			pathOverride = v
		}
	}
	p.skipSpace()
	if p.pos >= len(p.src) {
		return fmt.Errorf("rust: %s: unexpected end after `mod %s`", p.path, name)
	}
	switch p.src[p.pos] {
	case ';':
		p.pos++
		facts.Modules = append(facts.Modules, ModuleDecl{Name: name, Path: pathOverride, CfgTest: cfgTest})
		if cfgTest {
			facts.HasCfgTest = true
		}
		return nil
	case '{':
		p.pos++
		facts.Modules = append(facts.Modules, ModuleDecl{Name: name, Inline: true, CfgTest: cfgTest})
		if cfgTest {
			facts.HasCfgTest = true
		}
		sub := &FileFacts{}
		if err := p.parseItems(depth+1, cfgTest, sub); err != nil {
			return err
		}
		p.skipSpace()
		if p.pos < len(p.src) && p.src[p.pos] == '}' {
			p.pos++
		}
		seg := name
		if pathOverride != "" {
			seg = pathOverride
		}
		for i := range sub.Modules {
			m := &sub.Modules[i]
			if !strings.HasPrefix(m.Parent, "/") {
				m.Parent = path.Join(seg, m.Parent)
			}
			m.ParentFromFileDir = pathOverride != ""
			m.Scope = path.Join(name, m.Scope)
		}
		for i := range sub.Uses {
			sub.Uses[i].Scope = path.Join(name, sub.Uses[i].Scope)
		}
		facts.Modules = append(facts.Modules, sub.Modules...)
		facts.Uses = append(facts.Uses, sub.Uses...)
		for i := range sub.Externs {
			sub.Externs[i].Scope = path.Join(name, sub.Externs[i].Scope)
		}
		for i := range sub.Locals {
			sub.Locals[i].Scope = path.Join(name, sub.Locals[i].Scope)
		}
		facts.Externs = append(facts.Externs, sub.Externs...)
		facts.Locals = append(facts.Locals, sub.Locals...)
		if sub.HasTestAttr {
			facts.HasTestAttr = true
		}
		if sub.HasCfgTest {
			facts.HasCfgTest = true
		}
		return nil
	default:
		return fmt.Errorf("rust: %s: expected `;` or `{` after `mod %s`", p.path, name)
	}
}

func (p *parser) parseUse(attrs []string, testScope bool, facts *FileFacts) error {
	cfgTest := testScope
	for _, a := range attrs {
		kind, err := classifyTestAttr("use declaration", a)
		if err != nil {
			return err
		}
		if kind == attrTest {
			cfgTest = true
		}
	}
	start := p.pos
	semi := strings.IndexByte(p.src[start:], ';')
	if semi < 0 {
		return fmt.Errorf("rust: %s: unterminated `use` declaration", p.path)
	}
	raw := strings.TrimSpace(p.src[start : start+semi])
	p.pos = start + semi + 1
	if raw == "" {
		return fmt.Errorf("rust: %s: empty `use` declaration", p.path)
	}
	for _, m := range useAliasRe.FindAllStringSubmatch(raw, -1) {
		facts.Locals = append(facts.Locals, LocalName{Name: m[1]})
	}
	for _, path := range expandUse(raw) {
		decl := UseDecl{Path: path, CfgTest: cfgTest}
		if strings.HasSuffix(path, "::*") {
			decl.Glob = true
		}
		facts.Uses = append(facts.Uses, decl)
	}
	if cfgTest {
		facts.HasCfgTest = true
	}
	return nil
}

func (p *parser) parseExtern(attrs []string, testScope bool, facts *FileFacts) error {
	cfgTest := testScope
	for _, a := range attrs {
		kind, err := classifyTestAttr("extern crate declaration", a)
		if err != nil {
			return err
		}
		if kind == attrTest {
			cfgTest = true
		}
	}
	p.skipSpace()
	word, ok := p.scanWord()
	if !ok || word != "crate" {
		for p.pos < len(p.src) && p.src[p.pos] != ';' && p.src[p.pos] != '{' && p.src[p.pos] != '}' {
			p.pos++
		}
		return nil
	}
	p.skipSpace()
	name, ok := p.scanWord()
	if !ok {
		return fmt.Errorf("rust: %s: expected crate name after `extern crate`", p.path)
	}
	alias := ""
	p.skipSpace()
	if w, ok := p.scanWord(); ok && w == "as" {
		p.skipSpace()
		a, ok := p.scanWord()
		if !ok {
			return fmt.Errorf("rust: %s: expected alias after `as`", p.path)
		}
		alias = a
	} else if ok {
		p.rewindWord(w)
	}
	facts.Externs = append(facts.Externs, ExternCrate{Name: name, As: alias, CfgTest: cfgTest})
	if cfgTest {
		facts.HasCfgTest = true
	}
	return nil
}

func (p *parser) skipMacroGroup() {
	if p.pos+1 >= len(p.src) || p.src[p.pos] != '!' || p.src[p.pos+1] == '=' {
		return
	}
	save := p.pos
	p.pos++
	p.skipSpace()
	if p.pos < len(p.src) && (p.src[p.pos] == '(' || p.src[p.pos] == '[' || p.src[p.pos] == '{') {
		if end, ok := matchBalanced([]byte(p.src), p.pos); ok {
			p.pos = end
			return
		}
		p.pos = len(p.src)
		return
	}
	p.pos = save
}

func (p *parser) parseFn(attrs []string, testScope bool, depth int, facts *FileFacts) error {
	isTest := testScope
	for _, a := range attrs {
		kind, err := classifyTestAttr("function", a)
		if err != nil {
			return err
		}
		if kind == attrTest {
			isTest = true
		}
	}
	p.skipSpace()
	if p.pos < len(p.src) && p.src[p.pos] == '(' {
		return nil
	}
	for p.pos < len(p.src) && p.src[p.pos] != '{' && p.src[p.pos] != ';' && p.src[p.pos] != '}' {
		if c := p.src[p.pos]; c == '(' || c == '[' {
			if end, ok := matchBalanced([]byte(p.src), p.pos); ok {
				p.pos = end
				continue
			}
			p.pos = len(p.src)
			break
		}
		p.pos++
	}
	if p.pos < len(p.src) && p.src[p.pos] == '{' {
		p.pos++
		p.fnDepth++
		err := p.parseItems(depth+1, isTest, facts)
		p.fnDepth--
		if err != nil {
			return err
		}
		p.skipSpace()
		if p.pos < len(p.src) && p.src[p.pos] == '}' {
			p.pos++
		}
	} else if p.pos < len(p.src) && p.src[p.pos] == ';' {
		p.pos++
	}
	if isTest {
		facts.HasTestAttr = true
	}
	return nil
}

type attrKind int

const (
	attrNone attrKind = iota
	attrTest
)

func classifyTestAttr(what, attr string) (attrKind, error) {
	body := strings.TrimSpace(attr)
	if body == "test" {
		return attrTest, nil
	}
	if isCfgTest(body) {
		return attrTest, nil
	}
	if mentionsTest(body) {
		return attrNone, &AmbiguousError{What: what, Attr: "#[" + body + "]"}
	}
	return attrNone, nil
}

func isTestAttr(attr string) bool {
	kind, err := classifyTestAttr("item", attr)
	return err == nil && kind == attrTest
}

func isCfgTest(body string) bool {
	rest := strings.TrimSpace(body)
	if !strings.HasPrefix(rest, "cfg") {
		return false
	}
	rest = strings.TrimSpace(rest[len("cfg"):])
	if !strings.HasPrefix(rest, "(") || !strings.HasSuffix(rest, ")") {
		return false
	}
	return strings.TrimSpace(rest[1:len(rest)-1]) == "test"
}

func mentionsTest(body string) bool {
	for i := 0; i < len(body); {
		if body[i] == '"' {
			i++
			for i < len(body) && body[i] != '"' {
				if body[i] == '\\' {
					i++
				}
				i++
			}
			if i < len(body) {
				i++
			}
			continue
		}
		if isIdentStart(body[i]) {
			j := i + 1
			for j < len(body) && (isIdentChar(body[j])) {
				j++
			}
			if body[i:j] == "test" {
				return true
			}
			i = j
			continue
		}
		i++
	}
	return false
}

func parsePathAttr(attr string) (string, bool) {
	i := 0
	for i < len(attr) {
		for i < len(attr) && (isSpace(attr[i]) || attr[i] == ',') {
			i++
		}
		if !strings.HasPrefix(attr[i:], "path") {
			for i < len(attr) && attr[i] != ',' {
				i++
			}
			continue
		}
		j := i + len("path")
		if j < len(attr) && (isIdentChar(attr[j])) {
			i = j
			continue
		}
		for j < len(attr) && isSpace(attr[j]) {
			j++
		}
		if j >= len(attr) || attr[j] != '=' {
			i = j
			continue
		}
		j++
		for j < len(attr) && isSpace(attr[j]) {
			j++
		}
		if j >= len(attr) || attr[j] != '"' {
			return "", false
		}
		j++
		var b strings.Builder
		for j < len(attr) && attr[j] != '"' {
			if attr[j] == '\\' && j+1 < len(attr) {
				j++
			}
			b.WriteByte(attr[j])
			j++
		}
		return b.String(), true
	}
	return "", false
}

func expandUse(raw string) []string {
	raw = strings.TrimSpace(raw)
	if raw == "" {
		return nil
	}
	if idx := strings.IndexByte(raw, '{'); idx >= 0 {
		prefix := strings.TrimSpace(raw[:idx])
		prefix = strings.TrimSuffix(prefix, "::")
		end := strings.LastIndexByte(raw, '}')
		if end < 0 {
			return []string{raw}
		}
		inner := raw[idx+1 : end]
		var out []string
		for _, part := range splitTopLevel(inner, ',') {
			part = strings.TrimSpace(part)
			if part == "" {
				continue
			}
			if part == "self" {
				out = append(out, prefix)
				continue
			}
			expanded := expandUse(part)
			for _, e := range expanded {
				if prefix == "" {
					out = append(out, e)
				} else {
					out = append(out, prefix+"::"+e)
				}
			}
		}
		rest := strings.TrimSpace(raw[end+1:])
		_ = rest
		return out
	}
	if alias := stripAlias(raw); alias != "" {
		return []string{alias}
	}
	return []string{raw}
}

var useAliasRe = regexp.MustCompile(`\bas\s+([A-Za-z_][A-Za-z0-9_]*)`)

func stripAlias(raw string) string {
	if i := strings.LastIndex(raw, " as "); i >= 0 {
		return strings.TrimSpace(raw[:i])
	}
	return ""
}

func splitTopLevel(s string, sep byte) []string {
	var parts []string
	depth := 0
	start := 0
	for i := 0; i < len(s); i++ {
		switch s[i] {
		case '{', '(', '[':
			depth++
		case '}', ')', ']':
			depth--
		default:
			if s[i] == sep && depth == 0 {
				parts = append(parts, s[start:i])
				start = i + 1
			}
		}
	}
	return append(parts, s[start:])
}

func (p *parser) scanAttr() (body string, inner bool, ok bool) {
	if p.pos >= len(p.src) || p.src[p.pos] != '#' {
		return "", false, false
	}
	i := p.pos + 1
	if i < len(p.src) && p.src[i] == '!' {
		inner = true
		i++
	}
	for i < len(p.src) && isSpace(p.src[i]) {
		i++
	}
	if i >= len(p.src) || p.src[i] != '[' {
		return "", false, false
	}
	depth := 0
	for j := i; j < len(p.src); j++ {
		switch p.src[j] {
		case '[':
			depth++
		case ']':
			depth--
			if depth == 0 {
				p.pos = j + 1
				return strings.TrimSpace(p.src[i+1 : j]), inner, true
			}
		}
	}
	return "", false, false
}

func (p *parser) scanWord() (string, bool) {
	p.skipSpace()
	if p.pos >= len(p.src) || !isIdentStart(p.src[p.pos]) {
		return "", false
	}
	i := p.pos + 1
	for i < len(p.src) && isIdentChar(p.src[i]) {
		i++
	}
	w := p.src[p.pos:i]
	p.pos = i
	return w, true
}

func (p *parser) rewindWord(w string) {
	p.pos -= len(w)
}

func (p *parser) skipSpace() {
	for p.pos < len(p.src) && isSpace(p.src[p.pos]) {
		p.pos++
	}
}

func (p *parser) skipPubRestrict() {
	p.skipSpace()
	if p.pos < len(p.src) && p.src[p.pos] == '(' {
		if end, ok := matchBalanced([]byte(p.src), p.pos); ok {
			p.pos = end
		}
	}
}
