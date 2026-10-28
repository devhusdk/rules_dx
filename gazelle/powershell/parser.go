package powershell

import (
	"bytes"
	"regexp"
	"sort"
	"strings"
	"sync"

	"github.com/devhusdk/rules_dx/gazelle/common/treesitter"
	ts "github.com/odvcencio/gotreesitter"
	"github.com/odvcencio/gotreesitter/grammars/powershell"
)

var (
	moduleFileRE  = regexp.MustCompile(`(?i)([A-Za-z0-9_][A-Za-z0-9_.\-]*)\.ps[dm]1\b`)
	moduleIdentRE = regexp.MustCompile(`^[A-Za-z0-9_][A-Za-z0-9_.\-]*$`)
	requiresRE    = regexp.MustCompile(`(?i)^#requires\b`)
)

var grammar = sync.OnceValue(func() *ts.Language { return powershell.Language() })

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

// token is one command argument, independent of the grammar: kind is 'b' bare
// word, 's' single-quoted, 'd' double-quoted, 'p' (...), 'a' @(...) with
// items, or ',' comma.
type token struct {
	kind  byte
	text  string
	items []token
}

// ParseImports returns the module names a PowerShell source imports through
// Import-Module, using module and #Requires -Modules, plus RequiredModules when
// manifest is set (a .psd1). Literal names and paths to .psm1/.psd1 files
// count; variables and other expressions are skipped. Comments, strings and
// here-strings never yield imports.
//
// The grammar cannot parse unquoted comma lists (Import-Module A,B) and then
// loses the rest of the file; when the tree has a real error the legacy
// scanner results are added too.
func ParseImports(content []byte, manifest bool) []string {
	set := make(map[string]bool)
	add := func(names []string) {
		for _, name := range names {
			set[name] = true
		}
	}
	if treeImports(content, manifest, add) {
		add(legacyImports(content, manifest))
	}
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}

// treeImports reports the imports found in the syntax tree through add, and
// whether the tree held a real parse error.
func treeImports(content []byte, manifest bool, add func([]string)) bool {
	tree := treesitter.Parse(grammar(), content)
	defer tree.Release()
	if tree.Root == nil {
		return true
	}
	bad := hasRealError(tree)
	tree.Walk(tree.Root, func(n *treesitter.Node) bool {
		switch tree.Type(n) {
		case "command":
			collectCommand(tree, n, add)
		case "comment":
			if collectRequires(tree, content, n, add) {
				bad = true
			}
		case "hash_entry":
			if manifest && strings.EqualFold(entryKey(tree, n), "requiredmodules") {
				add(valueList(entryValue(tree, n), 0))
			}
		}
		return true
	})
	return bad
}

// hasRealError reports an ERROR or MISSING node. The grammar adds a
// whitespace-only ERROR for empty and comment-only files, which does not count.
func hasRealError(tree *treesitter.Tree) bool {
	if !tree.HasError {
		return false
	}
	bad := false
	tree.Walk(tree.Root, func(n *treesitter.Node) bool {
		if bad {
			return false
		}
		if n.IsMissing() || (n.IsError() && strings.TrimSpace(tree.Text(n)) != "") {
			bad = true
		}
		return !bad
	})
	return bad
}

func children(tree *treesitter.Tree, n *treesitter.Node, typ string) []*treesitter.Node {
	var out []*treesitter.Node
	for _, c := range n.Children() {
		if tree.Type(c) == typ {
			out = append(out, c)
		}
	}
	return out
}

func entryKey(tree *treesitter.Tree, n *treesitter.Node) string {
	for _, k := range children(tree, n, "key_expression") {
		return strings.TrimSpace(tree.Text(k))
	}
	return ""
}

func entryValue(tree *treesitter.Tree, n *treesitter.Node) []token {
	var out []token
	for _, v := range children(tree, n, "pipeline") {
		out = append(out, nodeTokens(tree, v)...)
	}
	return out
}

func collectCommand(tree *treesitter.Tree, n *treesitter.Node, add func([]string)) {
	names := children(tree, n, "command_name")
	elems := children(tree, n, "command_elements")
	if len(names) == 0 || len(elems) == 0 {
		return
	}
	args := commandTokens(tree, elems[0])
	switch strings.ToLower(strings.TrimSpace(tree.Text(names[0]))) {
	case "import-module":
		add(importModuleNames(args))
	case "using":
		if len(args) >= 2 && args[0].kind == 'b' && strings.EqualFold(args[0].text, "module") {
			add(moduleNames(args[1]))
		}
	}
}

// collectRequires handles a #Requires comment, which counts only as the first
// text on its line. It reports whether the comment's own parse had an error.
func collectRequires(tree *treesitter.Tree, src []byte, n *treesitter.Node, add func([]string)) bool {
	text := tree.Text(n)
	trimmed := strings.TrimLeft(text, " \t\r\n")
	if !requiresRE.MatchString(trimmed) {
		return false
	}
	start := min(int(n.StartByte())+len(text)-len(trimmed), len(src))
	lineStart := bytes.LastIndexByte(src[:start], '\n') + 1
	if len(bytes.TrimSpace(src[lineStart:start])) != 0 {
		return false
	}
	line, _, _ := strings.Cut(trimmed, "\n")
	line = strings.TrimRight(line, "\r")
	if !strings.Contains(strings.ToLower(line), "-modules") {
		return false
	}
	sub := treesitter.Parse(grammar(), []byte(line[1:]))
	defer sub.Release()
	if sub.Root == nil {
		return true
	}
	done := false
	sub.Walk(sub.Root, func(c *treesitter.Node) bool {
		if done || sub.Type(c) != "command" {
			return !done
		}
		elems := children(sub, c, "command_elements")
		if len(elems) == 0 {
			return true
		}
		args := commandTokens(sub, elems[0])
		for i, a := range args {
			if isFlag(a) && strings.EqualFold(a.text, "-modules") {
				add(valueList(args, i+1))
				done = true
				break
			}
		}
		return !done
	})
	return hasRealError(sub)
}

// commandTokens turns the children of a command_elements node into tokens.
// An inline parameter value (-Name:X) is folded into the flag token.
func commandTokens(tree *treesitter.Tree, elems *treesitter.Node) []token {
	var out []token
	merge := false
	last := func() *token { return &out[len(out)-1] }
	for _, c := range elems.Children() {
		if tree.Type(c) == "command_argument_sep" {
			text := tree.Text(c)
			switch {
			case strings.Contains(text, ":") && len(out) > 0 && isFlag(*last()):
				last().text = strings.TrimSuffix(last().text, ":") + ":"
				merge = true
			case text != "":
				merge = false
			}
			continue
		}
		toks := nodeTokens(tree, c)
		if merge && len(toks) > 0 && strings.IndexByte("bsd", toks[0].kind) >= 0 {
			last().text += toks[0].text
			toks = toks[1:]
		}
		out = append(out, toks...)
		merge = len(out) > 0 && isFlag(*last()) && strings.HasSuffix(last().text, ":")
	}
	return out
}

// wrappers are expression layers that pass one operand through unchanged.
var wrappers = map[string]bool{
	"unary_expression": true, "pipeline": true, "pipeline_chain": true,
	"logical_expression": true, "bitwise_expression": true,
	"comparison_expression": true, "additive_expression": true,
	"multiplicative_expression": true, "format_expression": true,
	"range_expression": true,
}

// nodeTokens flattens one syntax node into argument tokens.
func nodeTokens(tree *treesitter.Tree, n *treesitter.Node) []token {
	typ := tree.Type(n)
	raw := func() []token { return []token{{kind: 'b', text: strings.TrimSpace(tree.Text(n))}} }
	switch {
	case typ == ",":
		return []token{{kind: ','}}
	case typ == "comment" || typ == "command_argument_sep":
		return nil
	case typ == "generic_token":
		return raw()
	case typ == "command_parameter":
		text := strings.TrimSpace(tree.Text(n))
		for _, c := range n.Children() {
			if tree.Type(c) == ":" && !strings.HasSuffix(text, ":") {
				text += ":"
			}
		}
		return []token{{kind: 'b', text: text}}
	case typ == "string_literal":
		for _, c := range n.Children() {
			if strings.Contains(tree.Type(c), "here") {
				return nil
			}
		}
		text := tree.Text(n)
		kind := byte('d')
		if strings.HasPrefix(text, "'") {
			kind = 's'
		}
		if len(text) >= 2 {
			text = text[1 : len(text)-1]
		}
		return []token{{kind: kind, text: text}}
	case typ == "array_literal_expression" || typ == "statement_list":
		var out []token
		for _, c := range n.Children() {
			out = append(out, nodeTokens(tree, c)...)
		}
		return out
	case typ == "expression_with_unary_operator":
		kids := n.Children()
		if len(kids) > 0 && tree.Type(kids[0]) == "," {
			out := []token{{kind: ','}}
			for _, c := range kids[1:] {
				out = append(out, nodeTokens(tree, c)...)
			}
			return out
		}
		return raw()
	case wrappers[typ]:
		kids := n.Children()
		if len(kids) == 1 {
			return nodeTokens(tree, kids[0])
		}
		return raw()
	case typ == "parenthesized_expression" || typ == "sub_expression":
		return []token{{kind: 'p', text: uncommented(tree, n)}}
	case typ == "array_expression":
		var items []token
		for _, c := range n.Children() {
			items = append(items, nodeTokens(tree, c)...)
		}
		return []token{{kind: 'a', items: items}}
	case typ == "hash_literal_expression":
		name := ""
		for _, body := range children(tree, n, "hash_literal_body") {
			for _, e := range children(tree, body, "hash_entry") {
				if !strings.EqualFold(entryKey(tree, e), "modulename") {
					continue
				}
				if v := entryValue(tree, e); len(v) > 0 && strings.IndexByte("bsd", v[0].kind) >= 0 {
					name = v[0].text
				}
			}
		}
		return []token{{kind: 'b', text: name}}
	}
	return raw()
}

// uncommented returns n's source text with comments blanked out.
func uncommented(tree *treesitter.Tree, n *treesitter.Node) string {
	buf := []byte(tree.Text(n))
	base := int(n.StartByte())
	tree.Walk(n, func(c *treesitter.Node) bool {
		if tree.Type(c) != "comment" {
			return true
		}
		for k := int(c.StartByte()) - base; k < int(c.EndByte())-base; k++ {
			if k >= 0 && k < len(buf) {
				buf[k] = ' '
			}
		}
		return false
	})
	return string(buf)
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
	case 'a':
		var names []string
		for _, inner := range t.items {
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
