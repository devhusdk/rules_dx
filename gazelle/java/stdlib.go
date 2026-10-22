package java

import "github.com/devhusdk/rules_dx/gazelle/common"

func IsStdLib(spec string) bool {
	return common.IsJDKPlatform(spec)
}
