package common

import (
	"bytes"
	"path"
	"slices"
	"sort"
	"strings"
	"sync"

	"github.com/devhusdk/rules_dx/gazelle/common/treesitter"
	ts "github.com/odvcencio/gotreesitter"
	"github.com/odvcencio/gotreesitter/grammars/tsx"
	"github.com/odvcencio/gotreesitter/grammars/typescript"
)

var (
	tsxGrammar = sync.OnceValue(func() *ts.Language { return tsx.Language() })
	tsGrammar  = sync.OnceValue(func() *ts.Language { return typescript.Language() })
)

func Scan(src []byte, add func(string)) {
	src = bytes.TrimPrefix(src, []byte("\xef\xbb\xbf"))
	tree := treesitter.Parse(tsxGrammar(), src)
	defer tree.Release()
	lang := tsxGrammar()
	if tree.HasError {
		alt := treesitter.Parse(tsGrammar(), src)
		defer alt.Release()
		if !alt.HasError {
			tree, lang = alt, tsGrammar()
		}
	}
	if tree.Root == nil {
		return
	}
	emit := func(n *treesitter.Node) {
		if spec, _, ok := parseQuoted(src, int(n.StartByte())); ok {
			add(spec)
		}
	}
	treesitter.Walk(tree.Root, func(n *treesitter.Node) bool {
		switch n.Type(lang) {
		case "import_statement", "export_statement":
			if s := n.ChildByFieldName("source", lang); s != nil {
				if !n.HasError() && s.Type(lang) == "string" {
					emit(s)
				}
			} else if n.Type(lang) == "export_statement" {
				if s := exportRepair(n, src, lang); s != nil {
					emit(s)
				}
			}
		case "import_require_clause":
			if s := n.ChildByFieldName("source", lang); s != nil && !n.HasError() && s.Type(lang) == "string" {
				emit(s)
			}
		case "call_expression":
			if s := callSpec(n, src, lang); s != nil {
				emit(s)
			}
		}
		return true
	})
}

func callSpec(n *treesitter.Node, src []byte, lang *ts.Language) *treesitter.Node {
	fn := n.ChildByFieldName("function", lang)
	args := n.ChildByFieldName("arguments", lang)
	if fn == nil || args == nil || args.HasError() || args.NamedChildCount() != 1 {
		return nil
	}
	if t := fn.Type(lang); t != "import" && (t != "identifier" || treesitter.Text(fn, src) != "require") {
		return nil
	}
	if a := args.NamedChild(0); a.Type(lang) == "string" {
		return a
	}
	return nil
}

func exportRepair(n *treesitter.Node, src []byte, lang *ts.Language) *treesitter.Node {
	count := n.NamedChildCount()
	if count == 0 {
		return nil
	}
	last := n.NamedChild(count - 1)
	next := n.NextSibling()
	if next == nil || next.Type(lang) != "expression_statement" {
		return nil
	}
	switch last.Type(lang) {
	case "import_alias":
		if last.NamedChildCount() != 2 || treesitter.Text(last.NamedChild(1), src) != "require" {
			return nil
		}
		if next.NamedChildCount() != 1 || next.NamedChild(0).Type(lang) != "parenthesized_expression" {
			return nil
		}
		p := next.NamedChild(0)
		if p.NamedChildCount() != 1 || p.NamedChild(0).Type(lang) != "string" {
			return nil
		}
		return p.NamedChild(0)
	case "export_clause":
		if next.NamedChildCount() != 1 {
			return nil
		}
		id := next.NamedChild(0)
		if id.Type(lang) != "identifier" || treesitter.Text(id, src) != "from" || next.EndByte() != id.EndByte() {
			return nil
		}
		str := next.NextSibling()
		if str == nil || str.Type(lang) != "expression_statement" || str.NamedChildCount() != 1 || str.NamedChild(0).Type(lang) != "string" {
			return nil
		}
		return str.NamedChild(0)
	}
	return nil
}

type ImportRef struct {
	Spec      string
	Root      string
	Ext       string
	ModuleExt string
	Relative  bool
}

var JSModuleExts = []string{".js", ".jsx", ".mjs", ".cjs", ".ts", ".tsx", ".mts", ".cts"}

func relativeRef(dir, spec string, exts []string) (root, ext, moduleExt string) {
	spec = strings.TrimSpace(spec)
	trimmed := strings.TrimSuffix(spec, "/")
	if trimmed == "" {
		return "", "", ""
	}
	var joined string
	if strings.HasPrefix(spec, "/") {
		joined = strings.TrimPrefix(path.Clean(trimmed), "/")
		if joined == "" {
			return "", "", ""
		}
	} else {
		joined = path.Join(dir, trimmed)
		if joined == "." {
			return "index", "", ""
		}
		if strings.Trim(joined, "./") == "" {
			return joined, "", ""
		}
	}
	base := path.Base(joined)
	e := path.Ext(base)
	if e == "" || e == "." || e == base {
		return joined, "", ""
	}
	if slices.Contains(exts, e) {
		return strings.TrimSuffix(joined, e), "", e
	}
	return joined, e, ""
}

func ModuleIdentity(pkg, stem string) string {
	return path.Join(pkg, stem)
}

func isRelativeSpec(spec string) bool {
	spec = strings.TrimSpace(spec)
	return strings.HasPrefix(spec, ".") || strings.HasPrefix(spec, "/")
}

func ParseImportRefs(src []byte, dir string) []ImportRef {
	return parseRefs(src, dir, Scan)
}

func ParseSourceImportRefs(name string, src []byte, dir string) []ImportRef {
	if !IsJSTestConfig(name) {
		return ParseImportRefs(src, dir)
	}
	return parseRefs(src, dir, func(src []byte, add func(string)) {
		Scan(src, add)
		ScanJSTestConfig(src, add)
	})
}

func parseRefs(src []byte, dir string, scan func([]byte, func(string))) []ImportRef {
	refs := make(map[string]ImportRef)
	scan(src, func(spec string) {
		spec = strings.TrimSpace(spec)
		if spec == "" {
			return
		}
		relative := isRelativeSpec(spec)
		root, ext, moduleExt := spec, "", ""
		if relative {
			root, ext, moduleExt = relativeRef(dir, spec, JSModuleExts)
		}
		if root == "" {
			return
		}
		if prev, ok := refs[root]; ok && (prev.Relative || !relative) {
			return
		}
		refs[root] = ImportRef{Spec: spec, Root: root, Ext: ext, ModuleExt: moduleExt, Relative: relative}
	})
	out := make([]ImportRef, 0, len(refs))
	for _, ref := range refs {
		out = append(out, ref)
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Root < out[j].Root })
	return out
}

var refSetExts = append(slices.Clone(JSModuleExts), ".vue", ".svelte", ".astro", ".mdx")

type RefSet struct {
	Dir  string
	refs map[refKey]ImportRef
}

type refKey struct{ root, moduleExt string }

func (s *RefSet) Add(spec string) {
	spec = strings.TrimSpace(spec)
	relative := isRelativeSpec(spec)
	root, ext, moduleExt := spec, "", ""
	if relative {
		root, ext, moduleExt = relativeRef(s.Dir, spec, refSetExts)
	}
	if root == "" {
		return
	}
	key := refKey{root, moduleExt}
	if prev, ok := s.refs[key]; ok && (prev.Relative || !relative) {
		return
	}
	if s.refs == nil {
		s.refs = make(map[refKey]ImportRef)
	}
	s.refs[key] = ImportRef{Spec: spec, Root: root, Ext: ext, ModuleExt: moduleExt, Relative: relative}
}

func (s *RefSet) Refs() []ImportRef {
	var out []ImportRef
	for _, ref := range s.refs {
		out = append(out, ref)
	}
	SortRefs(out)
	return out
}

func SortRefs(refs []ImportRef) {
	sort.Slice(refs, func(i, j int) bool {
		if refs[i].Root != refs[j].Root {
			return refs[i].Root < refs[j].Root
		}
		return refs[i].ModuleExt < refs[j].ModuleExt
	})
}

func RefRoots(refs []ImportRef) []string {
	var out []string
	for _, ref := range refs {
		out = append(out, ref.Root)
	}
	return out
}

func skipComment(src []byte, p int) (int, bool) {
	n := len(src)
	if p+1 >= n || src[p] != '/' {
		return p, false
	}
	switch src[p+1] {
	case '/':
		j := p + 2
		for j < n && src[j] != '\n' {
			j++
		}
		return j, true
	case '*':
		j := p + 2
		for j+1 < n && !(src[j] == '*' && src[j+1] == '/') {
			j++
		}
		if j+1 < n {
			return j + 2, true
		}
		return n, true
	}
	return p, false
}

func skipTrivia(src []byte, pos int) int {
	n := len(src)
	p := pos
	for p < n {
		c := src[p]
		if c == ' ' || c == '\t' || c == '\r' || c == '\n' {
			p++
			continue
		}
		if np, ok := skipComment(src, p); ok {
			p = np
			continue
		}
		return p
	}
	return p
}

func parseQuoted(src []byte, pos int) (string, int, bool) {
	n := len(src)
	quote := src[pos]
	k := pos + 1
	var b strings.Builder
	for k < n {
		c := src[k]
		if c == '\\' {
			if k+1 < n {
				b.WriteByte(src[k+1])
				k += 2
				continue
			}
			return "", pos + 1, false
		}
		if c == quote {
			return b.String(), k + 1, true
		}
		if c == '\n' {
			return "", pos + 1, false
		}
		b.WriteByte(c)
		k++
	}
	return "", pos + 1, false
}

func skipQuoted(src []byte, pos int) int {
	if _, next, ok := parseQuoted(src, pos); ok {
		return next
	}
	n := len(src)
	k := pos + 1
	for k < n && src[k] != '\n' {
		k++
	}
	return k
}

func skipTemplate(src []byte, pos int) int {
	n := len(src)
	j := pos + 1
	for j < n {
		if src[j] == '\\' {
			j += 2
			continue
		}
		if src[j] == '`' {
			return j + 1
		}
		if src[j] == '$' && j+1 < n && src[j+1] == '{' {
			j = skipSubstitution(src, j+2)
			continue
		}
		j++
	}
	return n
}

func skipSubstitution(src []byte, pos int) int {
	n := len(src)
	depth := 1
	j := pos
	for j < n && depth > 0 {
		c := src[j]
		if c == '{' {
			depth++
		} else if c == '}' {
			depth--
		} else if c == '\'' || c == '"' {
			j = skipQuoted(src, j)
			continue
		} else if c == '`' {
			j = skipTemplate(src, j)
			continue
		}
		j++
	}
	return j
}

func isRegexStart(src []byte, pos int) bool {
	j := pos - 1
	for j >= 0 && (src[j] == ' ' || src[j] == '\t' || src[j] == '\r' || src[j] == '\n') {
		j--
	}
	if j < 0 {
		return true
	}
	c := src[j]
	if c == ')' || c == ']' || c == '}' {
		return false
	}
	if isIdentChar(c) || c == '"' || c == '\'' || c == '`' {
		return false
	}
	return true
}

func skipRegex(src []byte, pos int) int {
	n := len(src)
	k := pos + 1
	inClass := false
	for k < n {
		c := src[k]
		if c == '\\' {
			k += 2
			continue
		}
		if c == '\n' {
			return k
		}
		if c == '[' {
			inClass = true
			k++
			continue
		}
		if c == ']' {
			inClass = false
			k++
			continue
		}
		if c == '/' && !inClass {
			k++
			for k < n && isIdentChar(src[k]) {
				k++
			}
			return k
		}
		k++
	}
	return n
}

func isIdentStart(c byte) bool {
	return c == '_' || c == '$' || (c >= 'a' && c <= 'z') || (c >= 'A' && c <= 'Z')
}

func isIdentChar(c byte) bool {
	return isIdentStart(c) || (c >= '0' && c <= '9')
}

func IncompleteJS(src []byte) bool {
	n := len(src)
	var stack []byte
	i := 0
	for i < n {
		c := src[i]
		if len(stack) > 0 && stack[len(stack)-1] == '`' {
			switch {
			case c == '\\':
				i += 2
			case c == '`':
				stack = stack[:len(stack)-1]
				i++
			case c == '$' && i+1 < n && src[i+1] == '{':
				stack = append(stack, '}')
				i += 2
			default:
				i++
			}
			continue
		}
		switch {
		case c == '/' && i+1 < n && src[i+1] == '/':
			for i < n && src[i] != '\n' {
				i++
			}
		case c == '/' && i+1 < n && src[i+1] == '*':
			end := bytes.Index(src[i+2:], []byte("*/"))
			if end < 0 {
				return true
			}
			i += end + 4
		case c == '\'' || c == '"':
			i = skipQuoted(src, i)
		case c == '/' && isRegexStart(src, i):
			i = skipRegex(src, i)
		case c == '`':
			stack = append(stack, '`')
			i++
		case c == '(':
			stack = append(stack, ')')
			i++
		case c == '[':
			stack = append(stack, ']')
			i++
		case c == '{':
			stack = append(stack, '}')
			i++
		case len(stack) > 0 && c == stack[len(stack)-1]:
			stack = stack[:len(stack)-1]
			i++
		default:
			i++
		}
	}
	return len(stack) > 0
}

func ScanEmbedded(src []byte, add func(string)) { Scan(src, add) }

func isPrecededByDot(src []byte, pos int) bool {
	j := pos - 1
	for j >= 0 && (src[j] == ' ' || src[j] == '\t' || src[j] == '\r' || src[j] == '\n') {
		j--
	}
	return j >= 0 && src[j] == '.'
}
