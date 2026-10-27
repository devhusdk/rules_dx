package zig

import (
	"fmt"
	"path/filepath"
	"strings"

	"github.com/bazelbuild/bazel-gazelle/language"
	"github.com/devhusdk/rules_dx/gazelle/common"
)

var SupportedExts = []string{".zig"}

var TestSourceSuffixes = []string{"_test"}

const LibraryKind = "zig_library"

const ManifestFile = "build.zig.zon"

func rootHint(args language.GenerateArgs) string {
	content, err := common.ReadSource(filepath.Join(args.Dir, ManifestFile))
	if err != nil {
		return ""
	}
	return manifestName(content)
}

func manifestName(src []byte) string {
	src = common.StripBOM(src)
	depth := 0
	for i := 0; i < len(src); {
		switch c := src[i]; {
		case c == '/' && i+1 < len(src) && src[i+1] == '/':
			i = skipLine(src, i)
		case c == '\\' && i+1 < len(src) && src[i+1] == '\\':
			i = skipLine(src, i)
		case c == '"':
			_, i = readString(src, i)
		case c == '{':
			depth++
			i++
		case c == '}':
			depth--
			i++
		case c == '.' && depth == 1 && hasKey(src, i):
			return readValue(src, i+len(".name"))
		default:
			i++
		}
	}
	return ""
}

func skipLine(src []byte, i int) int {
	for i < len(src) && src[i] != '\n' {
		i++
	}
	return i
}

func readString(src []byte, i int) (string, int) {
	var b []byte
	i++
	for i < len(src) && src[i] != '"' {
		if src[i] == '\\' && i+1 < len(src) {
			i++
		}
		b = append(b, src[i])
		i++
	}
	if i < len(src) {
		i++
	}
	return string(b), i
}

func isIdentByte(c byte) bool {
	return c == '_' || c >= '0' && c <= '9' || c >= 'a' && c <= 'z' || c >= 'A' && c <= 'Z'
}

func hasKey(src []byte, i int) bool {
	if i > 0 && isIdentByte(src[i-1]) {
		return false
	}
	rest := src[i:]
	if len(rest) < len(".name") || string(rest[:len(".name")]) != ".name" {
		return false
	}
	return len(rest) == len(".name") || !isIdentByte(rest[len(".name")])
}

func skipSpace(src []byte, i int) int {
	for i < len(src) && (src[i] == ' ' || src[i] == '\t' || src[i] == '\n' || src[i] == '\r') {
		i++
	}
	return i
}

func readValue(src []byte, i int) string {
	i = skipSpace(src, i)
	if i >= len(src) || src[i] != '=' {
		return ""
	}
	i = skipSpace(src, i+1)
	if i >= len(src) {
		return ""
	}
	if src[i] == '"' {
		v, _ := readString(src, i)
		return v
	}
	if src[i] != '.' {
		return ""
	}
	i++
	if i < len(src) && src[i] == '@' && i+1 < len(src) && src[i+1] == '"' {
		v, _ := readString(src, i+1)
		return v
	}
	start := i
	for i < len(src) && isIdentByte(src[i]) {
		i++
	}
	return string(src[start:i])
}

func IsTestSource(name string) bool {
	return common.IsTestSource(name, SupportedExts, TestSourceSuffixes)
}

func IsBuildScript(name string) bool { return name == "build.zig" }

func DirTargetName(dir string) (string, error) { return common.DirTargetName(dir) }

func RootCandidates(name string) []string {
	return []string{name + ".zig", "root.zig", "lib.zig"}
}

func OrderRootFirst(name string, sources []string) ([]string, error) {
	present := make(map[string]bool, len(sources))
	for _, src := range sources {
		present[src] = true
	}
	root := ""
	for _, cand := range RootCandidates(name) {
		if present[cand] {
			root = cand
			break
		}
	}
	if root == "" && len(sources) == 1 {
		root = sources[0]
	}
	if root == "" {
		return nil, fmt.Errorf("no module root among %s; name one of %s", strings.Join(sources, ", "), strings.Join(RootCandidates(name), ", "))
	}
	out := []string{root}
	for _, src := range sources {
		if src != root {
			out = append(out, src)
		}
	}
	return out, nil
}
