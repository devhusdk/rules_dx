package cc

import (
	"path"
	"regexp"
	"sort"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common/treesitter"
	"github.com/odvcencio/gotreesitter/grammars/cpp"
)

var mainRe = regexp.MustCompile(`(^|[^A-Za-z0-9_])main\s*\(`)

// SystemInclude is one angle-bracket include and whether it sits under an __ANDROID conditional.
type SystemInclude struct {
	Path    string
	Guarded bool
}

type lexed struct {
	code    []byte
	quoted  []string
	systems []SystemInclude
}

// lexSource masks comments, literals and #include directives and returns the quoted include basenames.
func lexSource(content []byte) ([]byte, []string) {
	l := lex(content)
	return l.code, l.quoted
}

func lex(content []byte) lexed {
	s := strings.ReplaceAll(string(content), "\\\r\n", " ")
	s = strings.ReplaceAll(s, "\\\n", " ")
	src := []byte(s + "\n")
	code := []byte(s)
	mask := func(n *treesitter.Node) {
		end := int(n.EndByte())
		for end > int(n.StartByte()) && end <= len(code) && (code[end-1] == '\n' || code[end-1] == '\r') {
			end--
		}
		for k := int(n.StartByte()); k < end && k < len(code); k++ {
			if code[k] != '\n' {
				code[k] = ' '
			}
		}
	}
	tree := treesitter.Parse(cpp.Language(), src)
	defer tree.Release()
	if tree.HasError || tree.Root == nil {
		return legacyLex(content)
	}
	set := make(map[string]struct{})
	var systems []SystemInclude
	if tree.Root != nil {
		var visit func(n *treesitter.Node, guarded bool)
		visit = func(n *treesitter.Node, guarded bool) {
			switch tree.Type(n) {
			case "comment", "string_literal", "raw_string_literal", "char_literal":
				mask(n)
				return
			case "preproc_include":
				if atLineStart(src, int(n.StartByte())) {
					mask(n)
					collectInclude(tree, n, guarded, set, &systems)
					return
				}
			case "preproc_if", "preproc_ifdef", "preproc_elif", "preproc_elifdef":
				first, _, _ := strings.Cut(tree.Text(n), "\n")
				guarded = guarded || strings.Contains(first, "__ANDROID")
			}
			for _, c := range n.Children() {
				visit(c, guarded)
			}
		}
		visit(tree.Root, false)
	}
	includes := make([]string, 0, len(set))
	for name := range set {
		includes = append(includes, name)
	}
	sort.Strings(includes)
	return lexed{code: code, quoted: includes, systems: systems}
}

func collectInclude(tree *treesitter.Tree, n *treesitter.Node, guarded bool, set map[string]struct{}, systems *[]SystemInclude) {
	for _, c := range n.Children() {
		switch tree.Type(c) {
		case "string_literal":
			text := tree.Text(c)
			if len(text) < 2 || text[0] != '"' || text[len(text)-1] != '"' {
				return
			}
			if raw := strings.TrimSpace(text[1 : len(text)-1]); raw != "" {
				set[path.Clean(raw)] = struct{}{}
			}
			return
		case "system_lib_string":
			text := tree.Text(c)
			if len(text) < 2 || text[0] != '<' || text[len(text)-1] != '>' {
				return
			}
			if raw := strings.TrimSpace(text[1 : len(text)-1]); raw != "" {
				*systems = append(*systems, SystemInclude{Path: path.Clean(raw), Guarded: guarded})
			}
			return
		}
	}
}

// ParseSystemIncludes returns the angle-bracket includes in source order.
func ParseSystemIncludes(content []byte) []SystemInclude {
	return lex(content).systems
}

var entryRe = regexp.MustCompile(`(^|[^A-Za-z0-9_])(ANativeActivity_onCreate|JNI_OnLoad|Java_[A-Za-z0-9_]+)\s*\(([^()]*)\)\s*\{`)

// DefinesAndroidEntry reports whether content defines a native activity or JNI entry point.
func DefinesAndroidEntry(content []byte) bool {
	return entryRe.Match(lex(content).code)
}

func ParseQuotedIncludes(content []byte) []string {
	_, includes := lexSource(content)
	return includes
}

func DefinesMain(content []byte) bool {
	code, _ := lexSource(content)
	return mainRe.Match(code)
}

func atLineStart(b []byte, pos int) bool {
	for j := pos - 1; j >= 0 && b[j] != '\n'; j-- {
		if b[j] != ' ' && b[j] != '\t' && b[j] != '\r' {
			return false
		}
	}
	return true
}
