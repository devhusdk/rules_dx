package kotlin

import "github.com/devhusdk/rules_dx/gazelle/common"

var platformPackages = []string{"kotlin"}

func IsStdLib(spec string) bool {
	return common.InPackages(spec, platformPackages) || common.IsJDKPlatform(spec)
}
