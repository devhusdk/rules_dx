package d

import (
	"fmt"
	"regexp"
	"sort"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

var (
	makeDepLine   = regexp.MustCompile(`^[^\s:;(){}"'#]+\.\w+:(\s|$)`)
	dtraceProvide = regexp.MustCompile(`(?m)^provider\s+\w+\s*\{`)
)

// IsForeign reports whether a .d file is not D source: a Make dependency
// file or a DTrace script. Only these explicit facts skip a file; any other
// file that fails to parse is an error.
func IsForeign(content []byte) bool {
	content = common.StripBOM(content)
	text := string(content)
	first, _, _ := strings.Cut(text, "\n")
	first = strings.TrimRight(first, "\r")
	if strings.HasPrefix(first, "#!") && strings.Contains(first, "dtrace") {
		return true
	}
	if strings.Contains(text, "#pragma D ") || strings.HasPrefix(text, "#pragma D") {
		return true
	}
	if dtraceProvide.MatchString(text) {
		return true
	}
	return !strings.HasPrefix(first, "//") && !strings.HasPrefix(first, "/*") && !strings.HasPrefix(first, "/+") && makeDepLine.MatchString(first)
}

// Info holds the facts read from one D source file.
type Info struct {
	// Module is the declared module name, or "" without a declaration.
	Module string
	// Imports lists the imported modules, sorted and deduplicated.
	Imports []string
	// StringImports lists the literal import("file") arguments.
	StringImports []string
	// StringImportErr is set when an import(expr) argument is not a literal.
	StringImportErr error
	// DefinesMain reports a module-scope main function.
	DefinesMain bool
	// HasUnittest reports a unittest block.
	HasUnittest bool
}

// Analyze reads content. An unterminated literal or comment, or a malformed
// import, fails with its line, so malformed input is never taken for a clean
// file.
func Analyze(content []byte) (Info, error) {
	toks, err := lex(string(common.StripBOM(content)))
	if err != nil {
		return Info{}, err
	}
	p := &scanner{toks: toks, imports: map[string]bool{}, strs: map[string]bool{}}
	if err := p.run(); err != nil {
		return Info{}, err
	}
	p.info.Imports = sortedKeys(p.imports)
	p.info.StringImports = sortedKeys(p.strs)
	return p.info, nil
}

func sortedKeys(set map[string]bool) []string {
	out := make([]string, 0, len(set))
	for k := range set {
		out = append(out, k)
	}
	sort.Strings(out)
	return out
}

type scanner struct {
	toks    []token
	i       int
	info    Info
	imports map[string]bool
	strs    map[string]bool
	// braces holds one entry per open brace: true when declarations inside
	// are still module scope.
	braces []bool
	parens int
	// stmt is the index of the first token after the last ; { or }.
	stmt int
}

func (p *scanner) at(i int) token {
	if i >= 0 && i < len(p.toks) {
		return p.toks[i]
	}
	return token{kind: kPunct}
}

func (p *scanner) moduleScope() bool {
	for _, transparent := range p.braces {
		if !transparent {
			return false
		}
	}
	return p.parens == 0
}

func (p *scanner) run() error {
	for p.i = 0; p.i < len(p.toks); p.i++ {
		t := p.toks[p.i]
		switch {
		case t.is("("), t.is("["):
			p.parens++
		case t.is(")"), t.is("]"):
			p.parens = max(p.parens-1, 0)
		case t.is("{"):
			p.braces = append(p.braces, p.transparentBlock(p.toks[p.stmt:p.i]))
			p.stmt = p.i + 1
		case t.is("}"):
			if len(p.braces) > 0 {
				p.braces = p.braces[:len(p.braces)-1]
			}
			p.stmt = p.i + 1
		case t.is(";"):
			p.stmt = p.i + 1
		case t.kind != kIdent:
		case t.text == "module":
			p.module()
		case t.text == "import":
			if err := p.importAt(); err != nil {
				return err
			}
		case t.text == "unittest" && p.at(p.i+1).is("{"):
			p.info.HasUnittest = true
		case t.text == "main" && p.at(p.i+1).is("("):
			if p.moduleScope() && p.returnTypeEnds(p.i-1) {
				p.info.DefinesMain = true
			}
		}
	}
	return nil
}

func (p *scanner) returnTypeEnds(i int) bool {
	prev := p.at(i)
	switch {
	case prev.kind == kIdent:
		return true
	case prev.kind == kPunct:
		return prev.text == "]" || prev.text == "*" || prev.text == ")"
	}
	return false
}

func (p *scanner) dotted(i int) (string, int) {
	var parts []string
	for p.at(i).kind == kIdent {
		parts = append(parts, p.at(i).text)
		if !p.at(i + 1).is(".") {
			return strings.Join(parts, "."), i + 1
		}
		i += 2
	}
	return strings.Join(parts, "."), i
}

func (p *scanner) module() {
	if p.info.Module != "" || len(p.braces) > 0 {
		return
	}
	if name, _ := p.dotted(p.i + 1); name != "" {
		p.info.Module = name
	}
}

// importAt reads an import declaration or an import("file") expression at
// the current token and leaves i on its last token.
func (p *scanner) importAt() error {
	line := p.toks[p.i].line
	if p.at(p.i + 1).is("(") {
		arg, closer := p.at(p.i+2), p.at(p.i+3)
		switch {
		case arg.kind == kString && arg.text != "" && closer.is(")"):
			p.strs[arg.text] = true
		case p.info.StringImportErr == nil:
			p.info.StringImportErr = fmt.Errorf("import(...) at line %d is not a string literal", line)
		}
		return nil
	}
	i := p.i + 1
	for {
		if p.at(i).kind == kIdent && p.at(i+1).is("=") {
			i += 2
		}
		name, next := p.dotted(i)
		if name == "" {
			return fmt.Errorf("syntax error at line %d: malformed import", line)
		}
		p.imports[name] = true
		i = next
		switch {
		case p.at(i).is(","):
			i++
		case p.at(i).is(":"):
			for i < len(p.toks) && !p.at(i).is(";") {
				i++
			}
			p.i = i
			p.stmt = i + 1
			return nil
		case p.at(i).is(";"):
			p.i = i
			p.stmt = i + 1
			return nil
		default:
			return fmt.Errorf("syntax error at line %d: malformed import", line)
		}
	}
}

var attributeKeywords = map[string]bool{
	"static": true, "public": true, "private": true, "package": true, "protected": true,
	"export": true, "extern": true, "deprecated": true, "pure": true, "nothrow": true,
	"final": true, "override": true, "abstract": true, "synchronized": true,
	"const": true, "immutable": true, "inout": true, "shared": true, "__gshared": true,
	"align": true, "scope": true, "auto": true, "ref": true, "lazy": true,
}

// transparentBlock reports whether the brace that follows head keeps its
// contents in module scope: version, debug, static if, else and attribute
// blocks do, everything else does not.
func (p *scanner) transparentBlock(head []token) bool {
	i := 0
	if len(head) > 0 && head[0].is("else") {
		i++
	}
	for i < len(head) {
		switch {
		case head[i].is("@") && i+1 < len(head) && head[i+1].kind == kIdent:
			i += 2
		case head[i].kind == kIdent && attributeKeywords[head[i].text]:
			i++
		default:
			return transparentKeyword(head[i])
		}
		if i < len(head) && head[i].is("(") {
			i = skipGroup(head, i)
		}
	}
	return true
}

func transparentKeyword(t token) bool {
	switch t.text {
	case "version", "debug", "if", "foreach", "foreach_reverse":
		return t.kind == kIdent
	}
	return false
}

func skipGroup(head []token, i int) int {
	depth := 0
	for ; i < len(head); i++ {
		switch {
		case head[i].is("("):
			depth++
		case head[i].is(")"):
			depth--
			if depth == 0 {
				return i + 1
			}
		}
	}
	return i
}

// ParseModule returns the declared module name, or "" when the file has no
// module declaration or does not parse.
func ParseModule(content []byte) string {
	info, _ := Analyze(content)
	return info.Module
}

// ParseImports returns the sorted imported modules of content.
func ParseImports(content []byte) []string {
	info, _ := Analyze(content)
	return info.Imports
}

// ParseStringImports returns the literal import("file") arguments, and an
// error when any import expression is not a literal.
func ParseStringImports(content []byte) ([]string, error) {
	info, err := Analyze(content)
	if err != nil {
		return nil, err
	}
	return info.StringImports, info.StringImportErr
}

// DefinesMain reports a module-scope main function. Aggregate members and
// function bodies never count.
func DefinesMain(content []byte) bool {
	info, _ := Analyze(content)
	return info.DefinesMain
}

// HasUnittest reports whether content has a unittest block.
func HasUnittest(content []byte) bool {
	info, _ := Analyze(content)
	return info.HasUnittest
}
