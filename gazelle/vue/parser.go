package vue

import (
	"github.com/ralvik/rules_dx/gazelle/common"
	"sort"
)

func ParseImports(content []byte) []string {
	script := common.ExtractScript(content)
	if len(script) == 0 {
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
	common.ScanEmbedded(script, add)
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

func ExtractScript(src []byte) []byte {
	return common.ExtractScript(src)
}
