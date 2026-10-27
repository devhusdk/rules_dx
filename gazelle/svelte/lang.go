package svelte

import (
	"github.com/devhusdk/rules_dx/gazelle/common"

	"github.com/bazelbuild/bazel-gazelle/language"
)

func NewLanguage() language.Language {
	return common.NewWebComponentLang("svelte", "Svelte", []string{".svelte"}, common.ParseScriptImports)
}
