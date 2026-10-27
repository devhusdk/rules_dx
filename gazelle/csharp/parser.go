package csharp

import (
	"bytes"
	"github.com/devhusdk/rules_dx/gazelle/common"
	"regexp"
	"sort"
	"strings"
)

var (
	usingRe     = regexp.MustCompile(`(?m)^\s*(?:global\s+)?using\s+(static\s+)?([A-Za-z_][\w]*\s*=\s*)?([A-Za-z_][\w]*(?:\.[\w]+)*)\s*;\s*$`)
	namespaceRe = regexp.MustCompile(`(?m)^\s*namespace\s+([A-Za-z_][\w]*(?:\.[\w]+)*)\b`)
	mainRe      = regexp.MustCompile(`static\s+[\w<>\[\],\s]*\bMain\s*\(`)
)

func ParseImports(content []byte) []string {
	stripped := common.MaskCSharpStyle(content)
	set := make(map[string]struct{})
	for _, m := range usingRe.FindAllSubmatch(stripped, -1) {
		dotted := strings.TrimSpace(string(m[3]))
		if dotted == "" {
			continue
		}
		if len(m[1]) > 0 || len(m[2]) > 0 {
			dotted = foldTypeTarget(dotted)
		}
		set[dotted] = struct{}{}
	}
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}

func ParsePackage(content []byte) (string, error) {
	stripped := common.MaskCSharpStyle(content)
	matches := namespaceRe.FindAllSubmatch(stripped, -1)
	if len(matches) > 1 {
		return "", errDuplicatePackage(string(matches[0][1]))
	}
	if len(matches) == 0 {
		return "", nil
	}
	return string(matches[0][1]), nil
}

type duplicatePackageError struct{ first string }

func (e *duplicatePackageError) Error() string {
	return "csharp: duplicate namespace declaration " + e.first
}

func errDuplicatePackage(first string) error { return &duplicatePackageError{first: first} }

func DefinesMain(content []byte) bool {
	m := common.MaskCSharpStyle(content)
	return mainRe.Match(m) || hasTopLevelStatement(m)
}

var (
	usingDirectiveRe = regexp.MustCompile(`^(?:global\s+)?using\s+(?:static\s+)?(?:@?[A-Za-z_]\w*\s*=\s*[^;{}()]*|@?[A-Za-z_][\w.@]*(?:<[^;(){}]*>)?)\s*;`)
	externAliasRe    = regexp.MustCompile(`^extern\s+alias\s+@?[A-Za-z_]\w*\s*;`)
	identRe          = regexp.MustCompile(`^@?[A-Za-z_]\w*`)
	declWords        = map[string]bool{
		"namespace": true, "class": true, "struct": true, "interface": true, "enum": true, "record": true, "delegate": true,
		"public": true, "internal": true, "private": true, "protected": true, "static": true, "sealed": true, "abstract": true,
		"partial": true, "unsafe": true, "readonly": true, "ref": true, "file": true, "new": true,
	}
)

// hasTopLevelStatement reports whether the masked source has a statement
// before any namespace or type declaration, which makes the file a
// top-level-statements entry point.
func hasTopLevelStatement(m []byte) bool {
	i := 0
	if bytes.HasPrefix(m, []byte{0xEF, 0xBB, 0xBF}) {
		i = 3
	}
	for i < len(m) {
		switch c := m[i]; {
		case c == ' ' || c == '\t' || c == '\r' || c == '\n' || c == '\f' || c == '\v':
			i++
		case c == '#':
			for i < len(m) && m[i] != '\n' {
				i++
			}
		case c == '[':
			depth := 0
			for i < len(m) {
				if m[i] == '[' {
					depth++
				} else if m[i] == ']' {
					depth--
					if depth == 0 {
						i++
						break
					}
				}
				i++
			}
		default:
			rest := m[i:]
			if loc := externAliasRe.FindIndex(rest); loc != nil {
				i += loc[1]
				continue
			}
			if loc := usingDirectiveRe.FindIndex(rest); loc != nil {
				i += loc[1]
				continue
			}
			word := identRe.Find(rest)
			if word == nil {
				return true
			}
			if string(word) == "global" && bytes.HasPrefix(bytes.TrimLeft(rest[len(word):], " \t\r\n"), []byte("::")) {
				return true
			}
			return !declWords[string(word)]
		}
	}
	return false
}

// foldTypeTarget maps the target of a static or alias using to its namespace.
// Those forms may name a type, so a target that is not a platform namespace
// but whose parent is gets folded into the parent. A plain using always names
// a namespace and keeps exact matching.
func foldTypeTarget(dotted string) string {
	if IsStdLib(dotted) {
		return dotted
	}
	if i := strings.LastIndexByte(dotted, '.'); i >= 0 && IsStdLib(dotted[:i]) {
		return dotted[:i]
	}
	return dotted
}

// namespaceIDs indexes a source under its declared namespace and under the
// namespace-qualified file stem, so a using of the namespace or of the type
// named after the file resolves to its library. A source that cannot be read,
// declares no namespace, or declares several gets no IDs.
func namespaceIDs(path string) []string {
	return common.JVMQualifiedIDs(path, ParsePackage, ClassIdentity)
}

// namespaceCandidates returns the parent namespace of an unmatched using
// target, so a static or alias using of a local type reaches the library
// that declares the type's namespace. It climbs one level only.
func namespaceCandidates(name string) []string {
	i := strings.LastIndexByte(name, '.')
	if i <= 0 {
		return nil
	}
	return []string{name[:i]}
}
