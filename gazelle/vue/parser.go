package vue

import (
	"github.com/devhusdk/rules_dx/gazelle/common"
)

func ParseImports(content []byte, dir string) []string {
	set := common.SpecSet{Dir: dir}
	common.ScanEmbedded(common.ExtractScript(content), set.Add)
	return set.Roots()
}
