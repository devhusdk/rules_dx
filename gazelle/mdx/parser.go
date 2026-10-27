package mdx

import (
	"github.com/devhusdk/rules_dx/gazelle/common"
)

func ParseImports(content []byte, dir string) []string {
	set := common.SpecSet{Dir: dir}
	for _, region := range extractESMRegions(content) {
		common.ScanEmbedded(region, set.Add)
	}
	return set.Roots()
}
