package fsharp

import (
	"github.com/devhusdk/rules_dx/gazelle/common"
	"regexp"
	"sort"
	"strings"
)

const GlobalPackage = "global"

var (
	openRe       = regexp.MustCompile(`(?m)^\s*open\s+([A-Za-z_][\w]*(?:\.[\w]+)*)\s*$`)
	attrPrefixRe = regexp.MustCompile(`^\[<.*?>\]\s*`)
	namespaceRe  = regexp.MustCompile(`^namespace\s+(?:rec\s+)?([A-Za-z_]\w*(?:\.\w+)*)\s*$`)
	topModuleRe  = regexp.MustCompile(`^module\s+(?:(?:public|internal|private)\s+)?(?:rec\s+)?([A-Za-z_]\w*(?:\.\w+)*)\s*$`)
	declRe       = regexp.MustCompile(`^(type|and|exception|module|let|open|namespace)\b\s*(?:\[<.*?>\]\s*)?(?:(?:public|internal|private)\s+)?(?:rec\s+)?(?:\[<.*?>\]\s*)?([A-Za-z_]\w*)?(.*)$`)
	headerSkipRe = regexp.MustCompile(`(?m)^\s*(?:open|namespace)\b.*$|^module\s+(?:(?:public|internal|private)\s+)?(?:rec\s+)?[A-Za-z_]\w*(?:\.\w+)*\s*$`)
	mainRe       = regexp.MustCompile(`\[<\s*EntryPoint\s*>\]`)
)

func ParseImports(content []byte) []string {
	stripped := common.MaskFSharpStyle(content)
	set := make(map[string]struct{})
	for _, m := range openRe.FindAllSubmatch(stripped, -1) {
		dotted := string(m[1])
		if id := normalizeImport(dotted); id != "" {
			set[id] = struct{}{}
		}
	}
	return sortedKeys(set)
}

type topDecl struct {
	namespace bool
	name      string
}

func maskedLines(content []byte) []string {
	return strings.Split(string(common.MaskFSharpStyle(common.StripBOM(content))), "\n")
}

func parseTopDecl(lines []string) (topDecl, bool) {
	for _, line := range lines {
		line = strings.TrimRight(line, " \t\r")
		trimmed := strings.TrimSpace(line)
		if trimmed == "" || strings.HasPrefix(trimmed, "#") {
			continue
		}
		rest := attrPrefixRe.ReplaceAllString(line, "")
		if rest == "" {
			continue
		}
		if m := namespaceRe.FindStringSubmatch(rest); m != nil {
			return topDecl{namespace: true, name: m[1]}, true
		}
		if m := topModuleRe.FindStringSubmatch(rest); m != nil {
			return topDecl{name: m[1]}, true
		}
		return topDecl{}, false
	}
	return topDecl{}, false
}

func ParsePackage(content []byte) (string, error) {
	decl, ok := parseTopDecl(maskedLines(content))
	if !ok {
		return GlobalPackage, nil
	}
	if decl.namespace {
		return decl.name, nil
	}
	if i := strings.LastIndex(decl.name, "."); i >= 0 {
		return decl.name[:i], nil
	}
	return GlobalPackage, nil
}

func ParseDeclaredNames(content []byte) []string {
	lines := maskedLines(content)
	set := make(map[string]struct{})
	if decl, ok := parseTopDecl(lines); ok && !decl.namespace {
		name := decl.name
		if i := strings.LastIndex(name, "."); i >= 0 {
			name = name[i+1:]
		}
		set[name] = struct{}{}
		return sortedKeys(set)
	} else if !ok {
		return nil
	}
	inType := false
	for _, line := range lines {
		line = strings.TrimRight(line, " \t\r")
		if line == "" || line[0] == ' ' || line[0] == '\t' {
			continue
		}
		line = attrPrefixRe.ReplaceAllString(line, "")
		m := declRe.FindStringSubmatch(line)
		if m == nil {
			continue
		}
		keyword, name, tail := m[1], m[2], m[3]
		switch keyword {
		case "type", "exception":
			inType = keyword == "type"
			if name != "" {
				set[name] = struct{}{}
			}
		case "and":
			if inType && name != "" {
				set[name] = struct{}{}
			}
		case "module":
			inType = false
			if name != "" && strings.HasPrefix(strings.TrimSpace(tail), "=") {
				set[name] = struct{}{}
			}
		default:
			inType = false
		}
	}
	return sortedKeys(set)
}

func ParseReferencedNames(content []byte, pkg string) []string {
	text := headerSkipRe.ReplaceAllStringFunc(string(common.MaskFSharpStyle(common.StripBOM(content))), func(s string) string {
		return strings.Repeat(" ", len(s))
	})
	var prefix []string
	if pkg != "" && pkg != GlobalPackage {
		prefix = strings.Split(pkg, ".")
	}
	set := make(map[string]struct{})
	for i := 0; i < len(text); {
		c := text[i]
		if isDigit(c) {
			for i < len(text) && isIdentByte(text[i]) {
				i++
			}
			continue
		}
		if !isIdentStart(c) {
			i++
			continue
		}
		start := i
		var chain []string
		for {
			j := i
			for j < len(text) && isIdentByte(text[j]) {
				j++
			}
			chain = append(chain, text[i:j])
			i = j
			if i+1 < len(text) && text[i] == '.' && isIdentStart(text[i+1]) {
				i++
				continue
			}
			break
		}
		if start > 0 && (text[start-1] == '.' || text[start-1] == '\'') {
			continue
		}
		if len(chain) > len(prefix) && hasPrefix(chain, prefix) {
			chain = chain[len(prefix):]
		}
		set[chain[0]] = struct{}{}
	}
	return sortedKeys(set)
}

func hasPrefix(chain, prefix []string) bool {
	if len(prefix) == 0 {
		return false
	}
	for i, p := range prefix {
		if chain[i] != p {
			return false
		}
	}
	return true
}

func isDigit(c byte) bool { return c >= '0' && c <= '9' }

func isIdentStart(c byte) bool {
	return c == '_' || c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z'
}

func isIdentByte(c byte) bool { return isIdentStart(c) || isDigit(c) || c == '\'' }

func sortedKeys(set map[string]struct{}) []string {
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}

func DefinesMain(content []byte) bool {
	return mainRe.Match(common.MaskFSharpStyle(content))
}

func normalizeImport(dotted string) string {
	return common.NormalizeDotted(dotted, IsStdLib)
}
