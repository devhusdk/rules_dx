package svelte

import (
	"github.com/ralvik/rules_dx/gazelle/common"
)

func ParseImports(content []byte) []string {
	var set common.SpecSet
	for _, script := range common.ExtractScripts(content) {
		common.ScanEmbedded(script, set.Add)
	}
	return set.Roots()
}
