package scala

import "github.com/devhusdk/rules_dx/gazelle/common"

var platformPackages = []string{"scala"}

func IsStdLib(spec string) bool {
	return common.InPackages(spec, platformPackages) || common.IsJDKPlatform(spec)
}
