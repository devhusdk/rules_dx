package svelte

import (
	"github.com/ralvik/rules_dx/gazelle/common"
	"sort"
)

func ParseImports(content []byte) []string {
	scripts := common.ExtractScripts(content)
	if len(scripts) == 0 {
		return nil
	}
	set := make(map[string]struct{})
	add := func(spec string) {
		root := common.NormalizeJSSpec(spec)
		if root == "" {
			return
		}
		set[root] = struct{}{}
	}
	for _, script := range scripts {
		common.ScanEmbedded(script, add)
	}
	var out []string
	for name := range set {
		out = append(out, name)
	}
	sort.Strings(out)
	return out
}

func normalizeSpec(spec string) string {
	return common.NormalizeJSSpec(spec)
}

func ExtractScripts(src []byte) [][]byte {
	return common.ExtractScripts(src)
}

func skipQuoted(src []byte, pos int) int {
	return common.SkipEmbeddedQuoted(src, pos)
}

func skipTemplate(src []byte, pos int) int {
	return common.SkipEmbeddedTemplate(src, pos)
}
