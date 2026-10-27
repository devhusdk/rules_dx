package java

import (
	"github.com/devhusdk/rules_dx/gazelle/common"
	"regexp"
	"sort"
	"strings"
)

var (
	importRe  = regexp.MustCompile(`(?m)^\s*import\s+(static\s+)?([A-Za-z_][\w]*(?:\.[\w]+)*)(\.\*)?\s*;`)
	packageRe = regexp.MustCompile(`(?m)^\s*package\s+([A-Za-z_][\w]*(?:\.[\w]+)*)\s*;`)
	mainRe    = regexp.MustCompile(`static\s+void\s+main\s*\(`)
)

func ParseImports(content []byte) []string {
	stripped := common.MaskJavaStyle(content)
	set := make(map[string]struct{})
	for _, m := range importRe.FindAllSubmatch(stripped, -1) {
		static := len(m[1]) > 0
		onDemand := len(m[3]) > 0
		if id := importIdentity(string(m[2]), static, onDemand); id != "" {
			set[id] = struct{}{}
		}
	}
	out := make([]string, 0, len(set))
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}

func ParsePackage(content []byte) (string, error) {
	stripped := common.MaskJavaStyle(content)
	matches := packageRe.FindAllSubmatch(stripped, -1)
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
	return "java: duplicate package clause " + e.first
}

func errDuplicatePackage(first string) error { return &duplicatePackageError{first: first} }

func DefinesMain(content []byte) bool {
	return mainRe.Match(common.MaskJavaStyle(content))
}

func importIdentity(dotted string, static, onDemand bool) string {
	dotted = strings.TrimSpace(dotted)
	if !static {
		if onDemand && !IsStdLib(dotted) {
			return dotted + ".*"
		}
		return dotted
	}
	if IsStdLib(dotted) {
		return dotted
	}
	if onDemand {
		return dotted
	}
	i := strings.LastIndexByte(dotted, '.')
	if i < 0 {
		return ""
	}
	return dotted[:i]
}

func qualifiedIDs(path string) []string {
	return common.JVMQualifiedIDs(path, ParsePackage, ClassIdentity)
}
