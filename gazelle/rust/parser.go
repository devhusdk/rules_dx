package rust

import (
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
	// DeclCfgTest records whether the declaration itself sits under an outer
	// #[cfg(test)] (on the mod item or an enclosing inline module). Unlike
	// CfgTest it is never widened by an inner #![cfg(test)] of the file.
	DeclCfgTest bool
	Scope       string
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
	// TestOnly marks a file that rustc reads only under cfg(test): it is
	// reached through an outer #[cfg(test)] module declaration. Files reached
	// only through an inner #![cfg(test)] are not TestOnly. Set by LoadCrate.
	TestOnly bool
}

type AmbiguousError struct {
	What string
	Attr string
	// Path is the workspace-relative file; empty when unknown.
	Path string
	// Line and Col are 1-based; Col is a byte column.
	Line, Col int
	// StartByte and EndByte are offsets into the original file.
	StartByte, EndByte int
}

func (e *AmbiguousError) Error() string {
	if e.Path != "" {
		return fmt.Sprintf("rust: %s:%d:%d: ambiguous test context on %s: %s is not exactly #[cfg(test)]; disambiguate with #[cfg(test)] or move the item", e.Path, e.Line, e.Col, e.What, e.Attr)
	}
	return fmt.Sprintf("rust: ambiguous test context on %s: %s is not exactly #[cfg(test)]; disambiguate with #[cfg(test)] or move the item", e.What, e.Attr)
}
func ParseFacts(path string, content []byte) (*FileFacts, error) {
	tree, err := parseSyntax(content)
	if err != nil {
		return nil, fmt.Errorf("rust: %s: %v", path, err)
	}
	defer tree.release()
	w := &walker{path: path, tree: tree}
	if err := w.checkSyntax(); err != nil {
		return nil, err
	}
	facts := &FileFacts{}
	if err := w.items(tree.root, false, false, facts); err != nil {
		return nil, err
	}
	return facts, nil
}

type walker struct {
	path string
	tree *syntaxTree
}

type attrKind int

const (
	attrNone attrKind = iota
	attrTest
)

func (w *walker) checkSyntax() error {
	e, anc, ok := w.tree.root.findError(nil, tolerateError)
	if !ok {
		return nil
	}
	line, col := e.position()
	return fmt.Errorf("rust: %s:%d:%d: %s", w.path, line, col, w.describe(e, anc))
}

// tolerateError lets errors inside function bodies through unless they sit on
// a mod, use or extern declaration. Gazelle reads only item structure, and
// older editions use keywords such as async as identifiers.
func tolerateError(e syntaxNode, anc []syntaxNode) bool {
	if keywordOf(e) != "" {
		return false
	}
	inBlock := false
	for i := len(anc) - 1; i >= 0; i-- {
		switch anc[i].kind() {
		case "mod_item", "use_declaration", "extern_crate_declaration":
			return false
		case "block":
			inBlock = true
		}
	}
	return inBlock
}

func keywordOf(e syntaxNode) string {
	if e.n.IsMissing() {
		return ""
	}
	switch tok := e.firstToken(); tok {
	case "mod", "use", "extern":
		return tok
	}
	return ""
}

func (w *walker) describe(e syntaxNode, anc []syntaxNode) string {
	kw := keywordOf(e)
	for i := len(anc) - 1; kw == "" && i >= 0; i-- {
		switch anc[i].kind() {
		case "mod_item":
			kw = "mod"
		case "use_declaration":
			kw = "use"
		case "extern_crate_declaration":
			kw = "extern"
		}
	}
	switch kw {
	case "mod":
		if !w.modHasName(e, anc) {
			return "expected module name after `mod`"
		}
		return "expected `;` or `{` after `mod` declaration"
	case "use":
		return "malformed or unterminated `use` declaration"
	case "extern":
		return "malformed `extern crate` declaration"
	}
	if e.n.IsMissing() {
		return fmt.Sprintf("syntax error: missing %s", e.kind())
	}
	return fmt.Sprintf("syntax error near %q", w.snippet(e))
}

func (w *walker) modHasName(e syntaxNode, anc []syntaxNode) bool {
	node := e
	if !e.n.IsMissing() && e.kind() == "ERROR" {
		toks := e.tokens()
		return len(toks) > 1 && isIdentWord(toks[1])
	}
	for i := len(anc) - 1; i >= 0; i-- {
		if anc[i].kind() == "mod_item" {
			node = anc[i]
			break
		}
	}
	return node.field("name").valid()
}

func isIdentWord(s string) bool {
	if s == "" {
		return false
	}
	c := s[0]
	return c == '_' || c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z' || c >= 0x80
}

func (w *walker) snippet(e syntaxNode) string {
	text := e.text()
	if i := strings.IndexByte(text, '\n'); i >= 0 {
		text = text[:i]
	}
	if len(text) > 40 {
		text = text[:40]
	}
	return text
}

func (w *walker) items(parent syntaxNode, testScope, inFn bool, facts *FileFacts) error {
	var attrs []syntaxNode
	for _, child := range parent.children() {
		switch child.kind() {
		case "attribute_item":
			attrs = append(attrs, child)
			continue
		case "line_comment", "block_comment":
			continue
		case "inner_attribute_item":
			if !inFn && parent.kind() == "source_file" && isCfgTestAttr(child.attribute()) {
				facts.InnerCfgTest = true
			}
			continue
		case "token_tree", "macro_definition":
		case "mod_item":
			if err := w.mod(child, attrs, testScope, inFn, facts); err != nil {
				return err
			}
		case "use_declaration":
			cfgTest, err := w.scopeTest("use declaration", attrs, testScope)
			if err != nil {
				return err
			}
			w.use(child, cfgTest, facts)
		case "extern_crate_declaration":
			cfgTest, err := w.scopeTest("extern crate declaration", attrs, testScope)
			if err != nil {
				return err
			}
			w.extern(child, cfgTest, facts)
		case "function_item", "function_signature_item":
			if err := w.function(child, attrs, testScope, inFn, facts); err != nil {
				return err
			}
		case "enum_item":
			if name := child.field("name"); name.valid() {
				facts.Locals = append(facts.Locals, LocalName{Name: name.text()})
			}
		default:
			if err := w.items(child, testScope, inFn, facts); err != nil {
				return err
			}
		}
		attrs = attrs[:0]
	}
	return nil
}

func (w *walker) scopeTest(what string, attrs []syntaxNode, testScope bool) (bool, error) {
	cfgTest := testScope
	for _, a := range attrs {
		kind, err := w.classify(what, a)
		if err != nil {
			return false, err
		}
		if kind == attrTest {
			cfgTest = true
		}
	}
	return cfgTest, nil
}

func (w *walker) function(fn syntaxNode, attrs []syntaxNode, testScope, inFn bool, facts *FileFacts) error {
	isTest, err := w.scopeTest("function", attrs, testScope)
	if err != nil {
		return err
	}
	if body := fn.field("body"); body.valid() {
		if err := w.items(body, isTest, true, facts); err != nil {
			return err
		}
	}
	if isTest {
		facts.HasTestAttr = true
	}
	return nil
}

func (w *walker) mod(m syntaxNode, attrs []syntaxNode, testScope, inFn bool, facts *FileFacts) error {
	name := m.field("name").text()
	cfgTest := testScope
	pathOverride := ""
	for _, a := range attrs {
		kind, err := w.classify("module "+name, a)
		if err != nil {
			return err
		}
		if kind == attrTest {
			cfgTest = true
		}
		if v, ok := pathAttr(a.attribute()); ok {
			pathOverride = v
		}
	}
	if cfgTest {
		facts.HasCfgTest = true
	}
	body := m.field("body")
	if !body.valid() {
		facts.Modules = append(facts.Modules, ModuleDecl{Name: name, Path: pathOverride, CfgTest: cfgTest, DeclCfgTest: cfgTest})
		return nil
	}
	facts.Modules = append(facts.Modules, ModuleDecl{Name: name, Inline: true, CfgTest: cfgTest, DeclCfgTest: cfgTest})
	sub := &FileFacts{}
	if err := w.items(body, cfgTest, inFn, sub); err != nil {
		return err
	}
	seg := name
	if pathOverride != "" {
		seg = pathOverride
	}
	for i := range sub.Modules {
		mod := &sub.Modules[i]
		if !strings.HasPrefix(mod.Parent, "/") {
			mod.Parent = path.Join(seg, mod.Parent)
		}
		mod.ParentFromFileDir = pathOverride != ""
		mod.Scope = path.Join(name, mod.Scope)
	}
	for i := range sub.Uses {
		sub.Uses[i].Scope = path.Join(name, sub.Uses[i].Scope)
	}
	for i := range sub.Externs {
		sub.Externs[i].Scope = path.Join(name, sub.Externs[i].Scope)
	}
	for i := range sub.Locals {
		sub.Locals[i].Scope = path.Join(name, sub.Locals[i].Scope)
	}
	facts.Modules = append(facts.Modules, sub.Modules...)
	facts.Uses = append(facts.Uses, sub.Uses...)
	facts.Externs = append(facts.Externs, sub.Externs...)
	facts.Locals = append(facts.Locals, sub.Locals...)
	if sub.HasTestAttr {
		facts.HasTestAttr = true
	}
	if sub.HasCfgTest {
		facts.HasCfgTest = true
	}
	return nil
}

func (w *walker) use(u syntaxNode, cfgTest bool, facts *FileFacts) {
	arg := u.field("argument")
	for _, p := range w.expandUse(arg, "", facts) {
		facts.Uses = append(facts.Uses, UseDecl{Path: p, Glob: strings.HasSuffix(p, "::*"), CfgTest: cfgTest})
	}
	if cfgTest {
		facts.HasCfgTest = true
	}
}

func joinUse(prefix, rest string) string {
	if prefix == "" {
		return rest
	}
	if rest == "" {
		return prefix
	}
	return prefix + "::" + rest
}

func (w *walker) expandUse(n syntaxNode, prefix string, facts *FileFacts) []string {
	if !n.valid() {
		return nil
	}
	switch n.kind() {
	case "use_as_clause":
		if alias := n.field("alias"); alias.valid() {
			facts.Locals = append(facts.Locals, LocalName{Name: alias.text()})
		}
		return w.expandUse(n.field("path"), prefix, facts)
	case "scoped_use_list":
		inner := prefix
		if p := n.field("path"); p.valid() {
			inner = joinUse(prefix, p.joined())
		}
		return w.expandUse(n.field("list"), inner, facts)
	case "use_list":
		var out []string
		for _, c := range n.children() {
			if !c.isNamed() || c.isExtra() {
				continue
			}
			out = append(out, w.expandUse(c, prefix, facts)...)
		}
		return out
	case "self":
		if prefix == "" {
			return nil
		}
		return []string{prefix}
	}
	return []string{joinUse(prefix, n.joined())}
}

func (w *walker) extern(e syntaxNode, cfgTest bool, facts *FileFacts) {
	ex := ExternCrate{Name: e.field("name").text(), CfgTest: cfgTest}
	if alias := e.field("alias"); alias.valid() {
		ex.As = alias.text()
	}
	facts.Externs = append(facts.Externs, ex)
	if cfgTest {
		facts.HasCfgTest = true
	}
}

func (w *walker) classify(what string, item syntaxNode) (attrKind, error) {
	attr := item.attribute()
	if !attr.valid() {
		return attrNone, nil
	}
	if isTestAttr(attr) || isCfgTestAttr(attr) {
		return attrTest, nil
	}
	if mentionsTest(attr) {
		line, col := item.position()
		return attrNone, &AmbiguousError{
			What: what, Attr: "#[" + strings.TrimSpace(attr.text()) + "]",
			Path: w.path, Line: line, Col: col,
			StartByte: item.start(), EndByte: item.end(),
		}
	}
	return attrNone, nil
}

func isTestAttr(attr syntaxNode) bool {
	if !attr.valid() {
		return false
	}
	toks := attr.tokens()
	return len(toks) == 1 && toks[0] == "test"
}

func isCfgTestAttr(attr syntaxNode) bool {
	if !attr.valid() {
		return false
	}
	toks := attr.tokens()
	return len(toks) == 4 && toks[0] == "cfg" && toks[1] == "(" && toks[2] == "test" && toks[3] == ")"
}

func mentionsTest(attr syntaxNode) bool {
	for _, leaf := range attr.leaves() {
		if leaf.kind() == "identifier" && leaf.text() == "test" {
			return true
		}
	}
	return false
}

func pathAttr(attr syntaxNode) (string, bool) {
	if !attr.valid() {
		return "", false
	}
	kids := attr.children()
	if len(kids) != 3 || kids[0].text() != "path" || kids[1].text() != "=" || kids[2].kind() != "string_literal" {
		return "", false
	}
	var b strings.Builder
	for _, c := range kids[2].children() {
		switch c.kind() {
		case "string_content":
			b.WriteString(c.text())
		case "escape_sequence":
			b.WriteString(c.text()[1:])
		}
	}
	return b.String(), true
}

func (n syntaxNode) attribute() syntaxNode {
	for _, c := range n.children() {
		if c.kind() == "attribute" {
			return c
		}
	}
	return syntaxNode{}
}
