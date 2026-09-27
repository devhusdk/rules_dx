package javascript

import (
	"github.com/ralvik/rules_dx/gazelle/common"
	"sort"
	"strings"
)

func ParseImports(content []byte) []string {
	set := make(map[string]struct{})
	add := func(spec string) {
		root := common.NormalizeJSSpec(spec)
		if root == "" {
			return
		}
		set[root] = struct{}{}
	}
	common.Scan(content, add)
	var out []string
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}

type ImportRef struct {
	Root     string
	Relative bool
}

func IsRelativeSpec(spec string) bool {
	spec = strings.TrimSpace(spec)
	return strings.HasPrefix(spec, ".") || strings.HasPrefix(spec, "/")
}

func ParseImportRefs(content []byte) []ImportRef {
	rel := make(map[string]bool)
	add := func(spec string) {
		root := common.NormalizeJSSpec(spec)
		if root == "" {
			return
		}
		if IsRelativeSpec(spec) {
			rel[root] = true
		} else if _, ok := rel[root]; !ok {
			rel[root] = false
		}
	}
	common.Scan(content, add)
	out := make([]ImportRef, 0, len(rel))
	for root, relative := range rel {
		out = append(out, ImportRef{Root: root, Relative: relative})
	}
	sort.Slice(out, func(i, j int) bool { return out[i].Root < out[j].Root })
	return out
}

func normalizeSpec(spec string) string {
	return common.NormalizeJSSpec(spec)
}
