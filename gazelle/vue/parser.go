package vue

import (
	"github.com/devhusdk/rules_dx/gazelle/common"
)

func ParseImports(content []byte, dir string) []string {
	set := common.SpecSet{Dir: dir}
	for _, script := range common.ExtractScripts(content) {
		common.ScanEmbedded(script, set.Add)
	}
	return set.Roots()
}
