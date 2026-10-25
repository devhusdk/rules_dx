package scala

import "github.com/devhusdk/rules_dx/gazelle/common"

var externalPackages = []string{"scala.xml", "scala.util.parsing", "scala.collection.parallel", "scala.swing", "scala.async", "scala.tools"}

var platformPackages = []string{"scala"}

func IsStdLib(spec string) bool {
	if common.InPackages(spec, externalPackages) {
		return false
	}
	return common.InPackages(spec, platformPackages) || common.IsJDKPlatform(spec)
}
