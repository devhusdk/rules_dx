package zig

import (
	"fmt"
	"strings"

	"github.com/devhusdk/rules_dx/gazelle/common"
)

var SupportedExts = []string{".zig"}

var TestSourceSuffixes = []string{"_test"}

const LibraryKind = "zig_library"

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
