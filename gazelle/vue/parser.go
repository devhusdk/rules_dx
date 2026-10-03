package vue

import (
	"github.com/ralvik/rules_dx/gazelle/common"
)

func ParseImports(content []byte) []string {
	var set common.SpecSet
	common.ScanEmbedded(common.ExtractScript(content), set.Add)
	return set.Roots()
}
