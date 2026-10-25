package kotlin

import "github.com/devhusdk/rules_dx/gazelle/common"

var externalPackages = []string{"kotlin.reflect.full", "kotlin.reflect.jvm", "kotlin.test", "kotlin.script"}

var platformPackages = []string{"kotlin", "org.jetbrains.annotations"}

func IsStdLib(spec string) bool {
	if common.InPackages(spec, externalPackages) {
		return false
	}
	return common.InPackages(spec, platformPackages) || common.IsJDKPlatform(spec)
}
