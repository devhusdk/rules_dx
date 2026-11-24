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

type Info struct {
	Module          string
	Imports         []string
	StringImports   []string
	StringImportErr error
	DefinesMain     bool
	HasUnittest     bool
}

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
	braces  []bool
	parens  int
	stmt    int
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

func ParseModule(content []byte) string {
	info, _ := Analyze(content)
	return info.Module
}

func ParseImports(content []byte) []string {
	info, _ := Analyze(content)
	return info.Imports
}

func ParseStringImports(content []byte) ([]string, error) {
	info, err := Analyze(content)
	if err != nil {
		return nil, err
	}
	return info.StringImports, info.StringImportErr
}

func DefinesMain(content []byte) bool {
	info, _ := Analyze(content)
	return info.DefinesMain
}

func HasUnittest(content []byte) bool {
	info, _ := Analyze(content)
	return info.HasUnittest
}
